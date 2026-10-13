// Runtime nativo: o `dart:ffi` (`runtime/lib/ffi.cc` e
// `ffi_dynamic_library.cc` da VM, e o que o compilador dela gera para os
// métodos reconhecidos).
//
// * `DynamicLibrary`: `dlopen`/`dlsym` (Unix) e `LoadLibraryW`/
//   `GetProcAddress` (Windows); a biblioteca do processo procura em tudo o
//   que está carregado.
// * Os tipos nativos e os trampolins das assinaturas vêm do compilador
//   (`lower/ffi.rs`), registrados na preparação de cada isolado; os ids de
//   classe e os endereços são do programa, então as tabelas são globais.
// * `asFunction` cria uma closure com o trampolim da assinatura nativa (a
//   chave sai da RTI da assinatura) e o endereço no ambiente.
// * As cargas e gravações (`_loadInt32`…) servem a uma base `Pointer`
//   (memória nativa: sem verificação de limites, como em C) e a uma base
//   `TypedData` do heap (a de uma `Struct` criada no Dart: com verificação).
//   O acesso nativo é sempre desalinhado-seguro (`read_unaligned`).

/// O `handle` da biblioteca do processo (`DynamicLibrary.process()`): nenhum
/// handle do sistema o usa.
const PROCESS_HANDLE: i64 = i64::MIN + 1;

/// Classe RTI → letra do tipo C (`TipoC::letter` do compilador).
fn native_types() -> &'static std::sync::RwLock<crate::hash::HashMap<i64, char>> {
    static T: std::sync::OnceLock<std::sync::RwLock<crate::hash::HashMap<i64, char>>> = std::sync::OnceLock::new();
    T.get_or_init(Default::default)
}

/// Chave da assinatura → entrada do trampolim.
fn trampolines() -> &'static std::sync::RwLock<crate::hash::HashMap<String, usize>> {
    static T: std::sync::OnceLock<std::sync::RwLock<crate::hash::HashMap<String, usize>>> = std::sync::OnceLock::new();
    T.get_or_init(Default::default)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_registrar_tipo(class: i64, letter: i64) {
    let Some(c) = u32::try_from(letter).ok().and_then(char::from_u32) else { return };
    let mut t = native_types().write().unwrap_or_else(|e| e.into_inner());
    t.entry(class).or_insert(c);
}

/// # Safety
/// `key` aponta para `n` bytes ASCII; `input` é a entrada uniforme do
/// trampolim.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_ffi_registrar_trampolim(key: *const u8, n: i64, input: usize) {
    // SAFETY: garantido por quem chama (constante do módulo).
    let b = unsafe { std::slice::from_raw_parts(key, n as usize) };
    let key = String::from_utf8_lossy(b).into_owned();
    let mut t = trampolines().write().unwrap_or_else(|e| e.into_inner());
    t.entry(key).or_insert(input);
}

/// O tamanho de um tipo C pela letra.
fn letter_size(l: char) -> Option<i64> {
    Some(match l {
        'a' | 'h' | 'b' => 1,
        's' | 't' => 2,
        'i' | 'j' | 'f' => 4,
        'l' | 'm' | 'd' | 'p' => 8,
        _ => return None,
    })
}

/// O id RTI do tipo de um objeto `Type`.
fn object_type_type(h: i64) -> Option<i64> {
    HEAP.with(|heap| match heap.borrow().object(h) {
        Some(o) if o.class_id == TYPE_CLASS => o.first().map(|f| f.0),
        _ => None,
    })
}

/// A letra do tipo C de um tipo RTI (uma classe de tipo nativo).
fn type_letter(t: i64) -> Result<char, String> {
    let class = RTI.with(|u| match u.borrow().tipo(t) {
        Type::Interface(c, _) => Some(*c),
        _ => None,
    });
    let letter = class.and_then(|c| native_types().read().unwrap_or_else(|e| e.into_inner()).get(&c).copied());
    letter.ok_or_else(|| {
        let text = RTI.with(|u| u.borrow().text(t));
        format!("`{text}` is not a native type supported by the DartForge native backend")
    })
}

/// A parte de um tipo na chave: a letra do tipo C, ou `S<rti>.` de uma
/// struct/union por valor (registrada pelo programa).
fn key_part(t: i64, s: &mut String) -> Result<(), String> {
    if let Some(rti) = type_rti_class(t).filter(|&r| compound_from_rti(r).is_some()) {
        s.push_str(&format!("S{rti}."));
        return Ok(());
    }
    s.push(type_letter(t)?);
    Ok(())
}

/// A chave (`lower/ffi.rs::signature_key`) da assinatura nativa `ns`.
fn signature_key(ns: i64) -> Result<String, String> {
    let (ret, pos) = RTI.with(|u| match u.borrow().tipo(ns) {
        Type::Function { generics: 0, ret, pos, n_required, named } if named.is_empty() && *n_required == pos.len() => Ok((*ret, pos.clone())),
        _ => Err(format!("`{}` is not a native function signature", u.borrow().text(ns))),
    })?;
    let mut s = String::with_capacity(pos.len() + 2);
    key_part(ret, &mut s)?;
    s.push('_');
    for p in pos {
        // `VarArgs<(T1, T2…)>`/`VarArgs<T>`: a marca e os tipos variádicos.
        let variadic = RTI.with(|u| {
            let u = u.borrow();
            let Type::Interface(c, args) = u.tipo(p) else { return None };
            if native_types().read().unwrap_or_else(|e| e.into_inner()).get(c) != Some(&'*') {
                return None;
            }
            let a = *args.first()?;
            Some(match u.tipo(a) {
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

/// `sizeOf<T>()`: o tamanho em bytes do tipo nativo `T` na ABI do alvo.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_tamanho(tipo: i64) -> i64 {
    let Some(t) = object_type_type(tipo) else {
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

/// `asFunction`: a closure que chama a função nativa em `address` pela
/// assinatura `ns`, com o tipo Dart `df`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_funcao(address: i64, ns: i64, df: i64) -> i64 {
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
        // O contexto guarda os dois escalares (sem referência); a closure o
        // guarda como `Ref`. O owner temporário é contado durante a alocação
        // e soltado depois que a closure passa a guardar a aresta forte.
        let env = h.new_context(&[(address, false), (ns, false)]);
        let frame = h.push_owning_frame(1);
        h.set_root(frame, 0, env);
        let clo = h.new_closure(input as i64, (env, true), 0, 0);
        h.set_metadata(clo, df + 1);
        h.pop_frame(frame);
        clo
    })
}

/// O ambiente `[endereço, fn_signature]` da closure de um trampolim.
fn closure_env(clo: i64) -> (i64, i64) {
    HEAP.with(|h| {
        let h = h.borrow();
        let Some(c) = h.closure(clo) else {
            panic!("bug do compilador: trampolim FFI sem closure");
        };
        let env = c.contexto.0;
        (h.capture(env, 0).0, h.capture(env, 1).0)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_endereco_da_closure(clo: i64) -> i64 {
    closure_env(clo).0
}

/// O endereço de um `Pointer` (o campo `_endereco`).
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
pub extern "C" fn dartforge_ffi_endereco_do_ponteiro(pointer: i64) -> i64 {
    match address_of(pointer) {
        Some(e) => e,
        None => {
            throw_argument_error("a Pointer was expected in a native call");
            0
        }
    }
}

/// O `Pointer<X>` que uma chamada nativa devolve (`X` da assinatura).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_ponteiro_de_retorno(address: i64, clo: i64) -> i64 {
    let (_, ns) = closure_env(clo);
    new_pointer(address, RTI.with(|u| match u.borrow().tipo(ns) {
        Type::Function { ret, .. } => Some(*ret),
        _ => None,
    }))
}

/// O `Pointer<X>` que uma chamada nativa devolve, sob ARC, com um owner do
/// código.
///
/// `clo` é emprestada pelo chamador. O ajudante Dart `_dartforgePonteiro`
/// roda dentro de um escopo nativo: o token do retorno da porta Owned fica
/// nele e é copiado para o owner do chamador antes de fechar. Uma exceção
/// do ajudante fica pendente e o resultado é null, sem owner. Pode alocar,
/// coletar e executar Dart.
///
/// # Panics
/// Ajudante ausente, registro Owned incompatível ou escopo reentrante não
/// fechado; aborta no limite C.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_ffi_ponteiro_de_retorno_owned_v1(address: i64, clo: i64) -> i64 {
    owner_in_native_scope(|| dartforge_ffi_ponteiro_de_retorno(address, clo))
}

/// Um `Pointer` Dart com o endereço e, se conhecido, o tipo `Pointer<X>`.
fn new_pointer(address: i64, tipo: Option<i64>) -> i64 {
    let Some(f) = ajudante("_dartforgePonteiro") else {
        panic!("bug do compilador: dart:ffi sem `_dartforgePonteiro` registrado");
    };
    // SAFETY: registrada pela sobreposição de `dart:ffi` com a assinatura
    // (`int`) → `Pointer`.
    let f = { let dart_target: usize = f; move |a0: i64| -> i64 { dart_r1(dart_target, a0) } };
    let p = f(address);
    if let Some(t) = tipo {
        HEAP.with(|h| h.borrow_mut().set_metadata(p, t + 1));
    }
    p
}

// ---------------------------------------------------------------------------
// A memória: uma base `Pointer` ou `TypedData`.

/// Chama `f` com o endereço dos `n` bytes em `base + offset`. Numa
/// base `TypedData`, fora dos limites lança `RangeError`.
fn with_memory<R>(base: i64, offset: i64, n: usize, f: impl FnOnce(*mut u8) -> R) -> Option<R> {
    let list = HEAP.with(|h| resolver(&h.borrow(), base));
    if let Some(t) = list {
        let total = t.bytes;
        let begin = usize::try_from(offset).ok().filter(|&d| d.checked_add(n).is_some_and(|end| end <= total));
        let Some(begin) = begin else {
            throw_range_error(offset, total as i64);
            return None;
        };
        // Os bytes não se movem (§2.14): o endereço vale enquanto a base vive,
        // e `f` não aloca no heap.
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

// As cargas e gravações são funções explícitas (não geradas por macro):
// `build.rs` lê os símbolos exportados do texto dos fragmentos.
fn load_native<T: Copy + Into<i128>>(base: i64, offset: i64) -> i64 {
    // SAFETY: a memória nativa é a que o programa indicou (como em C); a do
    // heap foi conferida contra os limites.
    with_memory(base, offset, std::mem::size_of::<T>(), |p| unsafe { std::ptr::read_unaligned(p.cast::<T>()) }.into() as i64).unwrap_or(0)
}

fn write_native<T: Copy>(base: i64, offset: i64, value: T) {
    // SAFETY: como em `load_native`.
    with_memory(base, offset, std::mem::size_of::<T>(), |p| unsafe { std::ptr::write_unaligned(p.cast::<T>(), value) });
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_carregar_i8(base: i64, offset: i64) -> i64 {
    load_native::<i8>(base, offset)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_gravar_i8(base: i64, offset: i64, value: i64) {
    write_native::<i8>(base, offset, value as i8);
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_carregar_u8(base: i64, offset: i64) -> i64 {
    load_native::<u8>(base, offset)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_gravar_u8(base: i64, offset: i64, value: i64) {
    write_native::<u8>(base, offset, value as u8);
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_carregar_i16(base: i64, offset: i64) -> i64 {
    load_native::<i16>(base, offset)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_gravar_i16(base: i64, offset: i64, value: i64) {
    write_native::<i16>(base, offset, value as i16);
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_carregar_u16(base: i64, offset: i64) -> i64 {
    load_native::<u16>(base, offset)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_gravar_u16(base: i64, offset: i64, value: i64) {
    write_native::<u16>(base, offset, value as u16);
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_carregar_i32(base: i64, offset: i64) -> i64 {
    load_native::<i32>(base, offset)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_gravar_i32(base: i64, offset: i64, value: i64) {
    write_native::<i32>(base, offset, value as i32);
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_carregar_u32(base: i64, offset: i64) -> i64 {
    load_native::<u32>(base, offset)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_gravar_u32(base: i64, offset: i64, value: i64) {
    write_native::<u32>(base, offset, value as u32);
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_carregar_i64(base: i64, offset: i64) -> i64 {
    load_native::<i64>(base, offset)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_gravar_i64(base: i64, offset: i64, value: i64) {
    write_native::<i64>(base, offset, value as i64);
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_carregar_u64(base: i64, offset: i64) -> i64 {
    load_native::<u64>(base, offset)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_gravar_u64(base: i64, offset: i64, value: i64) {
    write_native::<u64>(base, offset, value as u64);
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_carregar_f32(base: i64, offset: i64) -> f64 {
    // SAFETY: como em `carga_e_gravacao!`.
    with_memory(base, offset, 4, |p| f64::from(unsafe { std::ptr::read_unaligned(p.cast::<f32>()) })).unwrap_or(0.0)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_gravar_f32(base: i64, offset: i64, value: f64) {
    // SAFETY: como em `carga_e_gravacao!`.
    with_memory(base, offset, 4, |p| unsafe { std::ptr::write_unaligned(p.cast::<f32>(), value as f32) });
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_carregar_f64(base: i64, offset: i64) -> f64 {
    // SAFETY: como em `carga_e_gravacao!`.
    with_memory(base, offset, 8, |p| unsafe { std::ptr::read_unaligned(p.cast::<f64>()) }).unwrap_or(0.0)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_gravar_f64(base: i64, offset: i64, value: f64) {
    // SAFETY: como em `carga_e_gravacao!`.
    with_memory(base, offset, 8, |p| unsafe { std::ptr::write_unaligned(p.cast::<f64>(), value) });
}

/// A carga de um inteiro específico da ABI (`Long`, `Size`…): o tipo C que o
/// compilador resolveu para o alvo.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_carregar_abi(tipo: i64, base: i64, offset: i64) -> i64 {
    match object_type_type(tipo).map(type_letter) {
        Some(Ok('a')) => dartforge_nativo_DartForge_ffi_carregar_i8(base, offset),
        Some(Ok('h')) => dartforge_nativo_DartForge_ffi_carregar_u8(base, offset),
        Some(Ok('s')) => dartforge_nativo_DartForge_ffi_carregar_i16(base, offset),
        Some(Ok('t')) => dartforge_nativo_DartForge_ffi_carregar_u16(base, offset),
        Some(Ok('i')) => dartforge_nativo_DartForge_ffi_carregar_i32(base, offset),
        Some(Ok('j')) => dartforge_nativo_DartForge_ffi_carregar_u32(base, offset),
        Some(Ok('l')) => dartforge_nativo_DartForge_ffi_carregar_i64(base, offset),
        Some(Ok('m')) => dartforge_nativo_DartForge_ffi_carregar_u64(base, offset),
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
pub extern "C" fn dartforge_nativo_DartForge_ffi_gravar_abi(tipo: i64, base: i64, offset: i64, value: i64) {
    match object_type_type(tipo).map(type_letter) {
        Some(Ok('a' | 'h')) => dartforge_nativo_DartForge_ffi_gravar_u8(base, offset, value),
        Some(Ok('s' | 't')) => dartforge_nativo_DartForge_ffi_gravar_u16(base, offset, value),
        Some(Ok('i' | 'j')) => dartforge_nativo_DartForge_ffi_gravar_u32(base, offset, value),
        Some(Ok('l' | 'm')) => dartforge_nativo_DartForge_ffi_gravar_u64(base, offset, value),
        Some(Err(m)) => throw_unsupported(&m),
        _ => throw_unsupported("AbiSpecificInteger without an integer mapping for this ABI"),
    }
}

/// `_memCopy(target, targetOffset, source, sourceOffset, length)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_copiar(dest: i64, dest_offset: i64, origin: i64, source_offset: i64, n: i64) {
    let Ok(n) = usize::try_from(n) else { return };
    let mut buf = vec![0u8; n];
    // SAFETY: como em `carga_e_gravacao!`; o buffer intermediário separa as
    // duas bases (que podem ser a mesma lista do heap).
    if with_memory(origin, source_offset, n, |p| unsafe { std::ptr::copy_nonoverlapping(p, buf.as_mut_ptr(), n) }).is_none() {
        return;
    }
    with_memory(dest, dest_offset, n, |p| unsafe { std::ptr::copy_nonoverlapping(buf.as_ptr(), p, n) });
}

/// `_abi()`: o índice do alvo em `Abi.values`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_abi() -> i64 {
    // A ordem de `Abi.values` (`abi.dart`): androidArm … windowsX64.
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
    /// `RTLD_DEFAULT`: `NULL` no Linux, `-2` no macOS.
    #[cfg(target_os = "linux")]
    pub const RTLD_DEFAULT: *mut std::ffi::c_void = std::ptr::null_mut();
    #[cfg(not(target_os = "linux"))]
    pub const RTLD_DEFAULT: *mut std::ffi::c_void = -2isize as *mut std::ffi::c_void;

    /// A última mensagem do `dl*`.
    pub fn error() -> String {
        // SAFETY: `dlerror` devolve nulo ou um texto válido até a próxima
        // chamada.
        let e = unsafe { dlerror() };
        if e.is_null() {
            return String::new();
        }
        // SAFETY: texto terminado em NUL.
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

    /// Os módulos carregados no processo (`DynamicLibrary.process()`).
    pub fn modulos() -> Vec<usize> {
        let mut n = 256u32;
        loop {
            let mut v = vec![0usize; n as usize];
            let mut needs = 0u32;
            // SAFETY: o buffer tem `n` handles.
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

/// `Ffi_dl_open(path)`: o handle, ou `ArgumentError` com o erro do sistema.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Ffi_dl_open(file_path: i64) -> i64 {
    let text = HEAP.with(|h| h.borrow().text(file_path).map(|t| t.para_string()).unwrap_or_default());
    #[cfg(unix)]
    {
        let Ok(c) = std::ffi::CString::new(text.clone()) else {
            return throw_ffi_argument_error(format!("Failed to load dynamic library '{text}': invalid path"));
        };
        // SAFETY: abre a biblioteca com um caminho C válido.
        let h = unsafe { dl::dlopen(c.as_ptr(), dl::RTLD_LAZY) };
        if h.is_null() {
            return throw_ffi_argument_error(format!("Failed to load dynamic library '{text}': {}", dl::error()));
        }
        h as i64
    }
    #[cfg(windows)]
    {
        let w: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: nome UTF-16 terminado em NUL.
        let h = unsafe { dl::LoadLibraryW(w.as_ptr()) };
        if h == 0 {
            let code = unsafe { dl::GetLastError() };
            return throw_ffi_argument_error(format!("Failed to load dynamic library '{text}': error code {code}"));
        }
        h as i64
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Ffi_dl_processLibrary() -> i64 {
    PROCESS_HANDLE
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Ffi_dl_executableLibrary() -> i64 {
    #[cfg(unix)]
    {
        // SAFETY: `dlopen(NULL)` é o executável.
        unsafe { dl::dlopen(std::ptr::null(), dl::RTLD_LAZY) as i64 }
    }
    #[cfg(windows)]
    {
        // SAFETY: o módulo do executável.
        unsafe { dl::GetModuleHandleW(std::ptr::null()) as i64 }
    }
}

/// O endereço de `name` em `handle` (0 se não há).
fn search_for(handle: i64, name: &std::ffi::CStr) -> usize {
    #[cfg(unix)]
    {
        let h = if handle == PROCESS_HANDLE { dl::RTLD_DEFAULT } else { handle as *mut std::ffi::c_void };
        // SAFETY: handle de `dlopen` (ou `RTLD_DEFAULT`) e nome C válido.
        unsafe { dl::dlsym(h, name.as_ptr()) as usize }
    }
    #[cfg(windows)]
    {
        if handle == PROCESS_HANDLE {
            return dl::modulos()
                .into_iter()
                // SAFETY: módulo carregado e nome C válido.
                .map(|m| unsafe { dl::GetProcAddress(m, name.as_ptr()) })
                .find(|&p| p != 0)
                .unwrap_or(0);
        }
        // SAFETY: handle de `LoadLibraryW` e nome C válido.
        unsafe { dl::GetProcAddress(handle as usize, name.as_ptr()) }
    }
}

/// `Ffi_dl_lookup(handle, symbolName)`: o endereço, ou `ArgumentError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Ffi_dl_lookup(handle: i64, name: i64) -> i64 {
    let text = HEAP.with(|h| h.borrow().text(name).map(|t| t.para_string()).unwrap_or_default());
    let Ok(c) = std::ffi::CString::new(text.clone()) else {
        return throw_ffi_argument_error(format!("Failed to lookup symbol '{text}'"));
    };
    let p = search_for(handle, &c);
    if p == 0 {
        #[cfg(unix)]
        let detail = dl::error();
        #[cfg(windows)]
        let detail = format!("error code {}", unsafe { dl::GetLastError() });
        return throw_ffi_argument_error(format!("Failed to lookup symbol '{text}': {detail}"));
    }
    p as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Ffi_dl_providesSymbol(handle: i64, name: i64) -> u8 {
    let text = HEAP.with(|h| h.borrow().text(name).map(|t| t.para_string()).unwrap_or_default());
    std::ffi::CString::new(text).map_or(0, |c| u8::from(search_for(handle, &c) != 0))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Ffi_dl_close(handle: i64) {
    if handle == PROCESS_HANDLE {
        throw_argument_error("Cannot close the process library.");
        return;
    }
    #[cfg(unix)]
    // SAFETY: handle de `dlopen`.
    unsafe {
        dl::dlclose(handle as *mut std::ffi::c_void);
    }
    #[cfg(windows)]
    // SAFETY: handle de `LoadLibraryW`.
    unsafe {
        dl::FreeLibrary(handle as usize);
    }
}

// ---------------------------------------------------------------------------
// Structs e unions (`lower/ffi.rs`, «Structs e unions»).

/// Uma struct/union do programa: a classe no heap, onde ficam os campos de
/// `_Compound` no objeto e a medida na ABI do alvo.
#[derive(Clone, Copy)]
struct FfiCompound {
    class: i64,
    fields: i64,
    base_index: i64,
    offset_index: i64,
    size: i64,
}

/// Classe RTI → composto (os ids de classe são do programa, iguais em todo
/// isolado).
fn ffi_compounds() -> &'static std::sync::RwLock<crate::hash::HashMap<i64, FfiCompound>> {
    static C: std::sync::OnceLock<std::sync::RwLock<crate::hash::HashMap<i64, FfiCompound>>> = std::sync::OnceLock::new();
    C.get_or_init(Default::default)
}

/// Registra uma struct/union do programa (a preparação do isolado).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_registrar_composto(
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

/// A classe RTI de um tipo RTI `Interface`.
fn type_rti_class(t: i64) -> Option<i64> {
    RTI.with(|u| match u.borrow().tipo(t) {
        Type::Interface(c, _) => Some(*c),
        _ => None,
    })
}

/// Uma instância da struct/union `rti` sobre a memória `base` +
/// `offset` (o `S#fromTypedDataBase` do transformador da VM).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_composto(rti: i64, base: i64, offset: i64) -> i64 {
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

/// `Pointer<S>.ref`/`[i]` (a sobreposição): a instância de `S` sobre
/// `base` + `offset`, pelo objeto `Type` de `S`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_composto_de_tipo(tipo: i64, base: i64, offset: i64) -> i64 {
    let Some(rti) = object_type_type(tipo).and_then(type_rti_class) else {
        throw_argument_error("struct type argument expected");
        return 0;
    };
    dartforge_ffi_composto(rti, base, offset)
}

/// Um `Pointer` (tipo `Pointer<Never>`) com o endereço (campo `Pointer` de
/// uma struct).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_ponteiro_novo(address: i64) -> i64 {
    new_pointer(address, None)
}

/// O endereço do símbolo nativo `name` (UTF-8), para um
/// `external` com `@Native` (com cache: o `Ffi_GetFfiNativeResolver` da VM
/// resolve uma vez por função). Símbolo ausente: `ArgumentError`, como a VM.
/// A ABI reservada DartForge vem deste runtime, inclusive no SDK RTLD_LOCAL;
/// símbolos estrangeiros continuam sendo procurados no processo.
///
/// # Safety
/// `name` aponta para `n` bytes legíveis.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_ffi_simbolo_nativo(name: *const u8, n: i64) -> i64 {
    // SAFETY: garantido por quem chama (uma constante do módulo).
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

/// A seleção local precede o lookup estrangeiro para não cruzar heaps/TLS.
fn resolve_native_address(name: &str, process: impl FnOnce(&std::ffi::CStr) -> usize) -> usize {
    endereco_nativo_do_runtime(name).unwrap_or_else(|| {
        std::ffi::CString::new(name).map_or(0, |c| process(&c))
    })
}

#[cfg(test)]
mod tests_resolve_native {
    use super::*;

    #[test]
    fn reserved_abi_does_not_query_same_named_process_runtime() {
        let looked_up = std::cell::Cell::new(false);
        let address = resolve_native_address("dartforge_arc_verificar_abi", |_| {
            looked_up.set(true);
            0xdead
        });
        assert!(!looked_up.get());
        let lookup: extern "C" fn(i64) -> i8 = dartforge_arc_verificar_abi;
        assert_eq!(address, lookup as *const () as usize);
        let previous = HEAP.with(|h| h.replace(Heap::new(true)));
        assert_eq!(lookup(1), 0);
        HEAP.with(|h| h.borrow_mut().enable_arc());
        assert_eq!(lookup(1), 1);
        HEAP.with(|h| h.replace(previous));
    }

    #[test]
    fn foreign_symbol_keeps_lookup_and_invalid_name_does_not_call_it() {
        assert_eq!(resolve_native_address("simbolo_da_biblioteca_do_usuario", |c| {
            assert_eq!(c.to_bytes(), b"simbolo_da_biblioteca_do_usuario");
            0xbeef
        }), 0xbeef);
        assert_eq!(resolve_native_address("ausente", |_| 0), 0);
        assert_eq!(resolve_native_address("nome\0invalido", |_| panic!("nome inválido chegou ao lookup")), 0);
    }
}

/// `Pointer<X>.asTypedList(n)`: a lista tipada interna `class_id` (elementos
/// `tipo`) sobre a memória nativa do ponteiro, sem cópia — escrever numa é
/// escrever na outra.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_typed_externo(class_id: i64, tipo: i64, pointer: i64, n: i64) -> i64 {
    let Some(address) = address_of(pointer) else {
        throw_argument_error("a Pointer was expected");
        return 0;
    };
    let tipo = tipo as u8;
    if n < 0 {
        throw_argument_error("length must be non-negative");
        return 0;
    }
    if address == 0 && n > 0 {
        throw_argument_error("asTypedList on nullptr");
        return 0;
    }
    debug_assert_eq!(class_id, i64::from(crate::layout::cid::tipada(tipo)), "cid da lista tipada fora do contrato");
    // A lista interna com `EXTERNAL` (§2.5): o comprimento e o endereço nativo.
    HEAP.with(|h| h.borrow_mut().external_typed(tipo, address as usize as *mut u8, n as usize))
}

/// [`dartforge_typed_externo`] que registra a tabela de métodos da classe na
/// primeira alocação (como `dartforge_typed_novo_t`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_typed_externo_t(class_id: i64, tipo: i64, pointer: i64, n: i64, f: extern "C" fn() -> *const i64) -> i64 {
    dartforge_registrar_tabela(class_id, f);
    dartforge_typed_externo(class_id, tipo, pointer, n)
}

/// O endereço dos bytes de uma struct/union (um argumento por valor): a
/// base (`Pointer` ou `TypedData`) mais o deslocamento, conferido contra o
/// tamanho numa base `TypedData`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_endereco_do_composto(obj: i64) -> i64 {
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

/// Uma struct/union nova, sobre um `Uint8List` do tamanho dela (o valor que
/// uma chamada nativa devolve por valor, como na VM).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_composto_novo(rti: i64) -> i64 {
    let Some(c) = compound_from_rti(rti) else {
        throw_unsupported("struct or union not registered in the DartForge native backend");
        return 0;
    };
    let bytes = HEAP.with(|h| h.borrow_mut().new_typed(TYPE_UINT8, c.size as usize));
    with_roots(&[bytes], || dartforge_ffi_composto(rti, bytes, 0))
}

/// Uma struct/union nova (sobre `Uint8List`) com os bytes em `address` — o
/// argumento por valor de um callback, copiado como na VM.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_composto_copia(rti: i64, address: i64) -> i64 {
    let new = dartforge_ffi_composto_novo(rti);
    if new == 0 {
        return 0;
    }
    let n = compound_from_rti(rti).map_or(0, |c| c.size as usize);
    let dest = with_roots(&[new], || dartforge_ffi_endereco_do_composto(new));
    if dest != 0 && address != 0 {
        // SAFETY: `address` tem os `n` bytes da struct (a entrada C ou uma
        // mensagem); `dest`, os da struct nova.
        unsafe { std::ptr::copy_nonoverlapping(address as usize as *const u8, dest as usize as *mut u8, n) };
    }
    new
}

/// Copia os `n` bytes da struct devolvida por um callback (em `origin`; 0 se
/// a closure lançou) para o retorno da entrada C.
///
/// # Safety
/// `dest` tem `n` bytes graváveis; `origin` é 0 ou tem `n` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_ffi_copiar_composto(dest: *mut u8, origin: i64, n: i64) {
    if origin != 0 && n > 0 {
        // SAFETY: garantido por quem chama.
        unsafe { std::ptr::copy_nonoverlapping(origin as usize as *const u8, dest, n as usize) };
    }
}

// ---------------------------------------------------------------------------
// `Handle`: objetos Dart na fronteira nativa (os handles locais da VM).
//
// Cada objeto passado vira uma célula (`Box<i64>` com o handle do objeto,
// raiz do coletor pelo endereço dela) e o C recebe o endereço da célula. As
// células pertencem ao escopo aberto em volta da chamada nativa e são soltas
// quando ele fecha; um `Handle` que volta do C é conferido contra as células
// vivas deste isolado (um ponteiro qualquer é `ArgumentError`).

thread_local! {
    static HANDLE_CELLS: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_handles_abrir() -> i64 {
    HANDLE_CELLS.with(|c| c.borrow().len() as i64)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_handles_fechar(scope: i64) {
    let released_list: Vec<usize> = HANDLE_CELLS.with(|c| {
        let mut c = c.borrow_mut();
        let de = (scope.max(0) as usize).min(c.len());
        c.split_off(de)
    });
    for cell in released_list {
        HEAP.with(|h| h.borrow_mut().release_global_root(cell as i64));
        // SAFETY: a célula foi criada por `dartforge_ffi_handle_novo` e sai
        // do registro uma vez.
        unsafe { drop(Box::from_raw(cell as *mut i64)) };
    }
}

/// O `Handle` de `obj`: o endereço de uma célula nova do escopo corrente.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_handle_novo(obj: i64) -> i64 {
    let cell = Box::into_raw(Box::new(obj)) as usize;
    HEAP.with(|h| h.borrow_mut().set_global_root(cell as i64, obj));
    HANDLE_CELLS.with(|c| c.borrow_mut().push(cell));
    cell as i64
}

/// O objeto de um `Handle` devolvido pelo C.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_objeto_do_handle(handle: i64) -> i64 {
    match live_object_of_handle(handle) {
        Some(obj) => obj,
        None => {
            throw_invalid_handle();
            0
        }
    }
}

/// O objeto de `handle` se a célula está viva neste isolado; sem efeitos.
fn live_object_of_handle(handle: i64) -> Option<i64> {
    let alive = HANDLE_CELLS.with(|c| c.borrow().iter().rev().any(|&x| x as i64 == handle));
    // SAFETY: a célula está viva (registrada neste isolado).
    alive.then(|| unsafe { *(handle as usize as *const i64) })
}

fn throw_invalid_handle() {
    throw_argument_error("the native code returned a Handle that is not alive in this isolate");
}

/// O objeto do `Handle` devolvido por uma chamada nativa, fechando em
/// seguida o escopo `scope` dos handles dela — também quando o `Handle` é
/// inválido, caso em que a `ArgumentError` fica pendente e o resultado é
/// null. Assim a saída excepcional não deixa células (e raízes) para trás.
///
/// Sob tracing, nada coleta entre ler a célula e devolver o objeto.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_handles_fechar_com_objeto(scope: i64, handle: i64) -> i64 {
    let obj = live_object_of_handle(handle);
    dartforge_ffi_handles_fechar(scope);
    obj.unwrap_or_else(|| {
        throw_invalid_handle();
        0
    })
}

/// [`dartforge_ffi_handles_fechar_com_objeto`] sob ARC: o objeto ganha um
/// owner do código antes de as células do escopo soltarem as suas raízes.
/// Handle inválido: `ArgumentError` pendente, null e nenhum owner.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_ffi_handles_fechar_com_objeto_owned_v1(scope: i64, handle: i64) -> i64 {
    // O owner antes de fechar os handles; o erro roda num escopo nativo.
    in_native_scope(|| {
        let obj = live_object_of_handle(handle);
        if let Some(o) = obj {
            dartforge_arc_retain(o);
        }
        dartforge_ffi_handles_fechar(scope);
        obj.unwrap_or_else(|| {
            throw_invalid_handle();
            0
        })
    })
}

/// [`dartforge_ffi_objeto_do_handle`] sob ARC, com um owner do código (o
/// argumento `Handle` de um callback, cuja célula é do escopo de fora).
/// Handle inválido: `ArgumentError` pendente, null e nenhum owner.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_ffi_objeto_do_handle_owned_v1(handle: i64) -> i64 {
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
