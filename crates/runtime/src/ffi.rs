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
const HANDLE_DO_PROCESSO: i64 = i64::MIN + 1;

/// Classe RTI → letra do tipo C (`TipoC::letra` do compilador).
fn tipos_nativos() -> &'static std::sync::RwLock<crate::hash::HashMap<i64, char>> {
    static T: std::sync::OnceLock<std::sync::RwLock<crate::hash::HashMap<i64, char>>> = std::sync::OnceLock::new();
    T.get_or_init(Default::default)
}

/// Chave da assinatura → entrada do trampolim.
fn trampolins() -> &'static std::sync::RwLock<crate::hash::HashMap<String, usize>> {
    static T: std::sync::OnceLock<std::sync::RwLock<crate::hash::HashMap<String, usize>>> = std::sync::OnceLock::new();
    T.get_or_init(Default::default)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_registrar_tipo(classe: i64, letra: i64) {
    let Some(c) = u32::try_from(letra).ok().and_then(char::from_u32) else { return };
    let mut t = tipos_nativos().write().unwrap_or_else(|e| e.into_inner());
    t.entry(classe).or_insert(c);
}

/// # Safety
/// `chave` aponta para `n` bytes ASCII; `entrada` é a entrada uniforme do
/// trampolim.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_ffi_registrar_trampolim(chave: *const u8, n: i64, entrada: usize) {
    // SAFETY: garantido por quem chama (constante do módulo).
    let b = unsafe { std::slice::from_raw_parts(chave, n as usize) };
    let chave = String::from_utf8_lossy(b).into_owned();
    let mut t = trampolins().write().unwrap_or_else(|e| e.into_inner());
    t.entry(chave).or_insert(entrada);
}

/// O tamanho de um tipo C pela letra.
fn tamanho_da_letra(l: char) -> Option<i64> {
    Some(match l {
        'a' | 'h' | 'b' => 1,
        's' | 't' => 2,
        'i' | 'j' | 'f' => 4,
        'l' | 'm' | 'd' | 'p' => 8,
        _ => return None,
    })
}

/// O id RTI do tipo de um objeto `Type`.
fn tipo_do_objeto_type(h: i64) -> Option<i64> {
    HEAP.with(|heap| match heap.borrow().try_get(h) {
        Some(Value::Object { class_id: CLASSE_TIPO, fields }) => fields.first().map(|f| f.0),
        _ => None,
    })
}

/// A letra do tipo C de um tipo RTI (uma classe de tipo nativo).
fn letra_do_tipo(t: i64) -> Result<char, String> {
    let classe = RTI.with(|u| match u.borrow().tipo(t) {
        Tipo::Interface(c, _) => Some(*c),
        _ => None,
    });
    let letra = classe.and_then(|c| tipos_nativos().read().unwrap_or_else(|e| e.into_inner()).get(&c).copied());
    letra.ok_or_else(|| {
        let texto = RTI.with(|u| u.borrow().texto(t));
        format!("`{texto}` is not a native type supported by the DartForge native backend")
    })
}

/// A parte de um tipo na chave: a letra do tipo C, ou `S<rti>.` de uma
/// struct/union por valor (registrada pelo programa).
fn parte_da_chave(t: i64, s: &mut String) -> Result<(), String> {
    if let Some(rti) = classe_rti_do_tipo(t).filter(|&r| composto_de_rti(r).is_some()) {
        s.push_str(&format!("S{rti}."));
        return Ok(());
    }
    s.push(letra_do_tipo(t)?);
    Ok(())
}

/// A chave (`lower/ffi.rs::chave_da_assinatura`) da assinatura nativa `ns`.
fn chave_da_assinatura(ns: i64) -> Result<String, String> {
    let (ret, pos) = RTI.with(|u| match u.borrow().tipo(ns) {
        Tipo::Funcao { genericos: 0, ret, pos, n_obrig, nomeados } if nomeados.is_empty() && *n_obrig == pos.len() => Ok((*ret, pos.clone())),
        _ => Err(format!("`{}` is not a native function signature", u.borrow().texto(ns))),
    })?;
    let mut s = String::with_capacity(pos.len() + 2);
    parte_da_chave(ret, &mut s)?;
    s.push('_');
    for p in pos {
        // `VarArgs<(T1, T2…)>`/`VarArgs<T>`: a marca e os tipos variádicos.
        let variadicos = RTI.with(|u| {
            let u = u.borrow();
            let Tipo::Interface(c, args) = u.tipo(p) else { return None };
            if tipos_nativos().read().unwrap_or_else(|e| e.into_inner()).get(c) != Some(&'*') {
                return None;
            }
            let a = *args.first()?;
            Some(match u.tipo(a) {
                Tipo::Registro { pos, nomeados } if nomeados.is_empty() => pos.clone(),
                _ => vec![a],
            })
        });
        match variadicos {
            Some(vs) => {
                s.push('*');
                for v in vs {
                    parte_da_chave(v, &mut s)?;
                }
            }
            None => parte_da_chave(p, &mut s)?,
        }
    }
    Ok(s)
}

fn lancar_unsupported(msg: &str) {
    let m = alocar_str(msg);
    let e = com_raizes(&[m], || dartforge_unsupported_error_new(m));
    com_raizes(&[e], || dartforge_exception_throw(e, 3));
}

/// `sizeOf<T>()`: o tamanho em bytes do tipo nativo `T` na ABI do alvo.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_tamanho(tipo: i64) -> i64 {
    let Some(t) = tipo_do_objeto_type(tipo) else {
        lancar_erro_de_argumento("sizeOf: type argument expected");
        return 0;
    };
    if let Some(c) = classe_rti_do_tipo(t).and_then(composto_de_rti) {
        return c.tamanho;
    }
    match letra_do_tipo(t).map(tamanho_da_letra) {
        Ok(Some(n)) => n,
        Ok(None) => {
            lancar_unsupported("sizeOf<Void>() is not defined");
            0
        }
        Err(m) => {
            lancar_unsupported(&m);
            0
        }
    }
}

/// `asFunction`: a closure que chama a função nativa em `endereco` pela
/// assinatura `ns`, com o tipo Dart `df`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_funcao(endereco: i64, ns: i64, df: i64) -> i64 {
    let (Some(ns), Some(df)) = (tipo_do_objeto_type(ns), tipo_do_objeto_type(df)) else {
        lancar_erro_de_argumento("asFunction: type arguments expected");
        return 0;
    };
    let chave = match chave_da_assinatura(ns) {
        Ok(c) => c,
        Err(m) => {
            lancar_unsupported(&m);
            return 0;
        }
    };
    let Some(entrada) = trampolins().read().unwrap_or_else(|e| e.into_inner()).get(&chave).copied() else {
        let texto = RTI.with(|u| u.borrow().texto(ns));
        lancar_unsupported(&format!("the native signature `{texto}` was not compiled into this program (it must be written as a constant type argument)"));
        return 0;
    };
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        let env = h.create_environment(vec![TaggedValue::scalar(endereco), TaggedValue::scalar(ns)]);
        let frame = h.push_frame_with_slots(1);
        h.set_root(frame, 0, env);
        let clo = h.create_closure(entrada as i64, env);
        h.set_metadado(clo, df + 1);
        h.pop_frame(frame);
        clo
    })
}

/// O ambiente `[endereço, assinatura]` da closure de um trampolim.
fn ambiente_da_closure(clo: i64) -> (i64, i64) {
    HEAP.with(|h| {
        let h = h.borrow();
        let (_, env) = h.closure_parts(clo);
        (h.environment_get(env, 0).bits, h.environment_get(env, 1).bits)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_endereco_da_closure(clo: i64) -> i64 {
    ambiente_da_closure(clo).0
}

/// O endereço de um `Pointer` (o campo `_endereco`).
fn endereco_de(ponteiro: i64) -> Option<i64> {
    HEAP.with(|h| {
        let h = h.borrow();
        match h.try_get(ponteiro) {
            Some(Value::Object { fields, .. }) => {
                let (bits, is_ref) = *fields.first()?;
                if is_ref { h.int_de_ref(bits) } else { Some(bits) }
            }
            _ => None,
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_endereco_do_ponteiro(ponteiro: i64) -> i64 {
    match endereco_de(ponteiro) {
        Some(e) => e,
        None => {
            lancar_erro_de_argumento("a Pointer was expected in a native call");
            0
        }
    }
}

/// O `Pointer<X>` que uma chamada nativa devolve (`X` da assinatura).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_ponteiro_de_retorno(endereco: i64, clo: i64) -> i64 {
    let (_, ns) = ambiente_da_closure(clo);
    novo_ponteiro(endereco, RTI.with(|u| match u.borrow().tipo(ns) {
        Tipo::Funcao { ret, .. } => Some(*ret),
        _ => None,
    }))
}

/// Um `Pointer` Dart com o endereço e, se conhecido, o tipo `Pointer<X>`.
fn novo_ponteiro(endereco: i64, tipo: Option<i64>) -> i64 {
    let Some(f) = ajudante("_dartforgePonteiro") else {
        panic!("bug do compilador: dart:ffi sem `_dartforgePonteiro` registrado");
    };
    // SAFETY: registrada pela sobreposição de `dart:ffi` com a assinatura
    // (`int`) → `Pointer`.
    let f: extern "C" fn(i64) -> i64 = unsafe { std::mem::transmute(f) };
    let p = f(endereco);
    if let Some(t) = tipo {
        HEAP.with(|h| h.borrow_mut().set_metadado(p, t + 1));
    }
    p
}

// ---------------------------------------------------------------------------
// A memória: uma base `Pointer` ou `TypedData`.

/// Chama `f` com o endereço dos `n` bytes em `base + deslocamento`. Numa
/// base `TypedData`, fora dos limites lança `RangeError`.
fn com_memoria<R>(base: i64, deslocamento: i64, n: usize, f: impl FnOnce(*mut u8) -> R) -> Option<R> {
    let lista = HEAP.with(|h| resolver(&h.borrow(), base));
    if let Some((interna, desloc, tipo, comprimento)) = lista {
        let total = comprimento * tamanho_do_elemento(tipo);
        let ini = usize::try_from(deslocamento).ok().filter(|&d| d.checked_add(n).is_some_and(|fim| fim <= total));
        let Some(ini) = ini else {
            lancar_erro_de_intervalo(deslocamento, total as i64);
            return None;
        };
        return Some(HEAP.with(|h| {
            let mut h = h.borrow_mut();
            let bytes = bytes_de_mut(&mut h, interna);
            f(bytes[desloc + ini..].as_mut_ptr())
        }));
    }
    let Some(endereco) = endereco_de(base) else {
        lancar_erro_de_argumento("a Pointer or TypedData was expected");
        return None;
    };
    Some(f(endereco.wrapping_add(deslocamento) as usize as *mut u8))
}

fn lancar_erro_de_intervalo(indice: i64, total: i64) {
    let m = alocar_str(&format!("Invalid value: Not in inclusive range 0..{}: {indice}", total - 1));
    let e = com_raizes(&[m], || dartforge_range_error_new(m));
    com_raizes(&[e], || dartforge_exception_throw(e, 3));
}

// As cargas e gravações são funções explícitas (não geradas por macro):
// `build.rs` lê os símbolos exportados do texto dos fragmentos.
fn carregar_nativo<T: Copy + Into<i128>>(base: i64, deslocamento: i64) -> i64 {
    // SAFETY: a memória nativa é a que o programa indicou (como em C); a do
    // heap foi conferida contra os limites.
    com_memoria(base, deslocamento, std::mem::size_of::<T>(), |p| unsafe { std::ptr::read_unaligned(p.cast::<T>()) }.into() as i64).unwrap_or(0)
}

fn gravar_nativo<T: Copy>(base: i64, deslocamento: i64, valor: T) {
    // SAFETY: como em `carregar_nativo`.
    com_memoria(base, deslocamento, std::mem::size_of::<T>(), |p| unsafe { std::ptr::write_unaligned(p.cast::<T>(), valor) });
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_carregar_i8(base: i64, deslocamento: i64) -> i64 {
    carregar_nativo::<i8>(base, deslocamento)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_gravar_i8(base: i64, deslocamento: i64, valor: i64) {
    gravar_nativo::<i8>(base, deslocamento, valor as i8);
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_carregar_u8(base: i64, deslocamento: i64) -> i64 {
    carregar_nativo::<u8>(base, deslocamento)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_gravar_u8(base: i64, deslocamento: i64, valor: i64) {
    gravar_nativo::<u8>(base, deslocamento, valor as u8);
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_carregar_i16(base: i64, deslocamento: i64) -> i64 {
    carregar_nativo::<i16>(base, deslocamento)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_gravar_i16(base: i64, deslocamento: i64, valor: i64) {
    gravar_nativo::<i16>(base, deslocamento, valor as i16);
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_carregar_u16(base: i64, deslocamento: i64) -> i64 {
    carregar_nativo::<u16>(base, deslocamento)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_gravar_u16(base: i64, deslocamento: i64, valor: i64) {
    gravar_nativo::<u16>(base, deslocamento, valor as u16);
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_carregar_i32(base: i64, deslocamento: i64) -> i64 {
    carregar_nativo::<i32>(base, deslocamento)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_gravar_i32(base: i64, deslocamento: i64, valor: i64) {
    gravar_nativo::<i32>(base, deslocamento, valor as i32);
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_carregar_u32(base: i64, deslocamento: i64) -> i64 {
    carregar_nativo::<u32>(base, deslocamento)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_gravar_u32(base: i64, deslocamento: i64, valor: i64) {
    gravar_nativo::<u32>(base, deslocamento, valor as u32);
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_carregar_i64(base: i64, deslocamento: i64) -> i64 {
    carregar_nativo::<i64>(base, deslocamento)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_gravar_i64(base: i64, deslocamento: i64, valor: i64) {
    gravar_nativo::<i64>(base, deslocamento, valor as i64);
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_carregar_u64(base: i64, deslocamento: i64) -> i64 {
    carregar_nativo::<u64>(base, deslocamento)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_gravar_u64(base: i64, deslocamento: i64, valor: i64) {
    gravar_nativo::<u64>(base, deslocamento, valor as u64);
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_carregar_f32(base: i64, deslocamento: i64) -> f64 {
    // SAFETY: como em `carga_e_gravacao!`.
    com_memoria(base, deslocamento, 4, |p| f64::from(unsafe { std::ptr::read_unaligned(p.cast::<f32>()) })).unwrap_or(0.0)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_gravar_f32(base: i64, deslocamento: i64, valor: f64) {
    // SAFETY: como em `carga_e_gravacao!`.
    com_memoria(base, deslocamento, 4, |p| unsafe { std::ptr::write_unaligned(p.cast::<f32>(), valor as f32) });
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_carregar_f64(base: i64, deslocamento: i64) -> f64 {
    // SAFETY: como em `carga_e_gravacao!`.
    com_memoria(base, deslocamento, 8, |p| unsafe { std::ptr::read_unaligned(p.cast::<f64>()) }).unwrap_or(0.0)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_gravar_f64(base: i64, deslocamento: i64, valor: f64) {
    // SAFETY: como em `carga_e_gravacao!`.
    com_memoria(base, deslocamento, 8, |p| unsafe { std::ptr::write_unaligned(p.cast::<f64>(), valor) });
}

/// A carga de um inteiro específico da ABI (`Long`, `Size`…): o tipo C que o
/// compilador resolveu para o alvo.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_carregar_abi(tipo: i64, base: i64, deslocamento: i64) -> i64 {
    match tipo_do_objeto_type(tipo).map(letra_do_tipo) {
        Some(Ok('a')) => dartforge_nativo_DartForge_ffi_carregar_i8(base, deslocamento),
        Some(Ok('h')) => dartforge_nativo_DartForge_ffi_carregar_u8(base, deslocamento),
        Some(Ok('s')) => dartforge_nativo_DartForge_ffi_carregar_i16(base, deslocamento),
        Some(Ok('t')) => dartforge_nativo_DartForge_ffi_carregar_u16(base, deslocamento),
        Some(Ok('i')) => dartforge_nativo_DartForge_ffi_carregar_i32(base, deslocamento),
        Some(Ok('j')) => dartforge_nativo_DartForge_ffi_carregar_u32(base, deslocamento),
        Some(Ok('l')) => dartforge_nativo_DartForge_ffi_carregar_i64(base, deslocamento),
        Some(Ok('m')) => dartforge_nativo_DartForge_ffi_carregar_u64(base, deslocamento),
        Some(Err(m)) => {
            lancar_unsupported(&m);
            0
        }
        _ => {
            lancar_unsupported("AbiSpecificInteger without an integer mapping for this ABI");
            0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_gravar_abi(tipo: i64, base: i64, deslocamento: i64, valor: i64) {
    match tipo_do_objeto_type(tipo).map(letra_do_tipo) {
        Some(Ok('a' | 'h')) => dartforge_nativo_DartForge_ffi_gravar_u8(base, deslocamento, valor),
        Some(Ok('s' | 't')) => dartforge_nativo_DartForge_ffi_gravar_u16(base, deslocamento, valor),
        Some(Ok('i' | 'j')) => dartforge_nativo_DartForge_ffi_gravar_u32(base, deslocamento, valor),
        Some(Ok('l' | 'm')) => dartforge_nativo_DartForge_ffi_gravar_u64(base, deslocamento, valor),
        Some(Err(m)) => lancar_unsupported(&m),
        _ => lancar_unsupported("AbiSpecificInteger without an integer mapping for this ABI"),
    }
}

/// `_memCopy(target, targetOffset, source, sourceOffset, length)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_copiar(destino: i64, desloc_destino: i64, origem: i64, desloc_origem: i64, n: i64) {
    let Ok(n) = usize::try_from(n) else { return };
    let mut buf = vec![0u8; n];
    // SAFETY: como em `carga_e_gravacao!`; o buffer intermediário separa as
    // duas bases (que podem ser a mesma lista do heap).
    if com_memoria(origem, desloc_origem, n, |p| unsafe { std::ptr::copy_nonoverlapping(p, buf.as_mut_ptr(), n) }).is_none() {
        return;
    }
    com_memoria(destino, desloc_destino, n, |p| unsafe { std::ptr::copy_nonoverlapping(buf.as_ptr(), p, n) });
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

fn lancar_erro_de_argumento_ffi(msg: String) -> i64 {
    lancar_erro_de_argumento(&msg);
    0
}

#[cfg(unix)]
mod dl {
    unsafe extern "C" {
        pub fn dlopen(caminho: *const std::ffi::c_char, modo: i32) -> *mut std::ffi::c_void;
        pub fn dlsym(h: *mut std::ffi::c_void, nome: *const std::ffi::c_char) -> *mut std::ffi::c_void;
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
    pub fn erro() -> String {
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
        pub fn LoadLibraryW(nome: *const u16) -> usize;
        pub fn GetModuleHandleW(nome: *const u16) -> usize;
        pub fn GetProcAddress(h: usize, nome: *const std::ffi::c_char) -> usize;
        pub fn FreeLibrary(h: usize) -> i32;
        pub fn GetLastError() -> u32;
        pub fn GetCurrentProcess() -> usize;
        pub fn K32EnumProcessModules(p: usize, m: *mut usize, n: u32, precisa: *mut u32) -> i32;
    }

    /// Os módulos carregados no processo (`DynamicLibrary.process()`).
    pub fn modulos() -> Vec<usize> {
        let mut n = 256u32;
        loop {
            let mut v = vec![0usize; n as usize];
            let mut precisa = 0u32;
            // SAFETY: o buffer tem `n` handles.
            let ok = unsafe { K32EnumProcessModules(GetCurrentProcess(), v.as_mut_ptr(), n * std::mem::size_of::<usize>() as u32, &mut precisa) } != 0;
            if !ok {
                return Vec::new();
            }
            let usados = precisa as usize / std::mem::size_of::<usize>();
            if usados <= v.len() {
                v.truncate(usados);
                return v;
            }
            n = usados as u32;
        }
    }
}

/// `Ffi_dl_open(path)`: o handle, ou `ArgumentError` com o erro do sistema.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Ffi_dl_open(caminho: i64) -> i64 {
    let texto = HEAP.with(|h| h.borrow().texto(caminho).para_string());
    #[cfg(unix)]
    {
        let Ok(c) = std::ffi::CString::new(texto.clone()) else {
            return lancar_erro_de_argumento_ffi(format!("Failed to load dynamic library '{texto}': invalid path"));
        };
        // SAFETY: abre a biblioteca com um caminho C válido.
        let h = unsafe { dl::dlopen(c.as_ptr(), dl::RTLD_LAZY) };
        if h.is_null() {
            return lancar_erro_de_argumento_ffi(format!("Failed to load dynamic library '{texto}': {}", dl::erro()));
        }
        h as i64
    }
    #[cfg(windows)]
    {
        let w: Vec<u16> = texto.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: nome UTF-16 terminado em NUL.
        let h = unsafe { dl::LoadLibraryW(w.as_ptr()) };
        if h == 0 {
            let codigo = unsafe { dl::GetLastError() };
            return lancar_erro_de_argumento_ffi(format!("Failed to load dynamic library '{texto}': error code {codigo}"));
        }
        h as i64
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Ffi_dl_processLibrary() -> i64 {
    HANDLE_DO_PROCESSO
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

/// O endereço de `nome` em `handle` (0 se não há).
fn procurar(handle: i64, nome: &std::ffi::CStr) -> usize {
    #[cfg(unix)]
    {
        let h = if handle == HANDLE_DO_PROCESSO { dl::RTLD_DEFAULT } else { handle as *mut std::ffi::c_void };
        // SAFETY: handle de `dlopen` (ou `RTLD_DEFAULT`) e nome C válido.
        unsafe { dl::dlsym(h, nome.as_ptr()) as usize }
    }
    #[cfg(windows)]
    {
        if handle == HANDLE_DO_PROCESSO {
            return dl::modulos()
                .into_iter()
                // SAFETY: módulo carregado e nome C válido.
                .map(|m| unsafe { dl::GetProcAddress(m, nome.as_ptr()) })
                .find(|&p| p != 0)
                .unwrap_or(0);
        }
        // SAFETY: handle de `LoadLibraryW` e nome C válido.
        unsafe { dl::GetProcAddress(handle as usize, nome.as_ptr()) }
    }
}

/// `Ffi_dl_lookup(handle, symbolName)`: o endereço, ou `ArgumentError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Ffi_dl_lookup(handle: i64, nome: i64) -> i64 {
    let texto = HEAP.with(|h| h.borrow().texto(nome).para_string());
    let Ok(c) = std::ffi::CString::new(texto.clone()) else {
        return lancar_erro_de_argumento_ffi(format!("Failed to lookup symbol '{texto}'"));
    };
    let p = procurar(handle, &c);
    if p == 0 {
        #[cfg(unix)]
        let detalhe = dl::erro();
        #[cfg(windows)]
        let detalhe = format!("error code {}", unsafe { dl::GetLastError() });
        return lancar_erro_de_argumento_ffi(format!("Failed to lookup symbol '{texto}': {detalhe}"));
    }
    p as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Ffi_dl_providesSymbol(handle: i64, nome: i64) -> u8 {
    let texto = HEAP.with(|h| h.borrow().texto(nome).para_string());
    std::ffi::CString::new(texto).map_or(0, |c| u8::from(procurar(handle, &c) != 0))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Ffi_dl_close(handle: i64) {
    if handle == HANDLE_DO_PROCESSO {
        lancar_erro_de_argumento("Cannot close the process library.");
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
struct CompostoFfi {
    classe: i64,
    campos: i64,
    indice_base: i64,
    indice_deslocamento: i64,
    tamanho: i64,
}

/// Classe RTI → composto (os ids de classe são do programa, iguais em todo
/// isolado).
fn compostos_ffi() -> &'static std::sync::RwLock<crate::hash::HashMap<i64, CompostoFfi>> {
    static C: std::sync::OnceLock<std::sync::RwLock<crate::hash::HashMap<i64, CompostoFfi>>> = std::sync::OnceLock::new();
    C.get_or_init(Default::default)
}

/// Registra uma struct/union do programa (a preparação do isolado).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_registrar_composto(
    rti: i64,
    classe: i64,
    campos: i64,
    indice_base: i64,
    indice_deslocamento: i64,
    tamanho: i64,
    _alinhamento: i64,
) {
    compostos_ffi()
        .write()
        .unwrap_or_else(|e| e.into_inner())
        .insert(rti, CompostoFfi { classe, campos, indice_base, indice_deslocamento, tamanho });
}

fn composto_de_rti(rti: i64) -> Option<CompostoFfi> {
    compostos_ffi().read().unwrap_or_else(|e| e.into_inner()).get(&rti).copied()
}

/// A classe RTI de um tipo RTI `Interface`.
fn classe_rti_do_tipo(t: i64) -> Option<i64> {
    RTI.with(|u| match u.borrow().tipo(t) {
        Tipo::Interface(c, _) => Some(*c),
        _ => None,
    })
}

/// Uma instância da struct/union `rti` sobre a memória `base` +
/// `deslocamento` (o `S#fromTypedDataBase` do transformador da VM).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_composto(rti: i64, base: i64, deslocamento: i64) -> i64 {
    let Some(c) = composto_de_rti(rti) else {
        lancar_unsupported("struct or union not registered in the DartForge native backend");
        return 0;
    };
    let obj = com_raizes(&[base], || dartforge_object_new(c.classe, c.campos));
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        if let Value::Object { fields, .. } = h.get_mut(obj) {
            if let Some(f) = usize::try_from(c.indice_base).ok().and_then(|i| fields.get_mut(i)) {
                *f = (base, true);
            }
            if let Some(f) = usize::try_from(c.indice_deslocamento).ok().and_then(|i| fields.get_mut(i)) {
                *f = (deslocamento, false);
            }
        }
    });
    obj
}

/// `Pointer<S>.ref`/`[i]` (a sobreposição): a instância de `S` sobre
/// `base` + `deslocamento`, pelo objeto `Type` de `S`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_composto_de_tipo(tipo: i64, base: i64, deslocamento: i64) -> i64 {
    let Some(rti) = tipo_do_objeto_type(tipo).and_then(classe_rti_do_tipo) else {
        lancar_erro_de_argumento("struct type argument expected");
        return 0;
    };
    dartforge_ffi_composto(rti, base, deslocamento)
}

/// Um `Pointer` (tipo `Pointer<Never>`) com o endereço (campo `Pointer` de
/// uma struct).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_ponteiro_novo(endereco: i64) -> i64 {
    novo_ponteiro(endereco, None)
}

/// O endereço do símbolo nativo `nome` (UTF-8) no processo, para um
/// `external` com `@Native` (com cache: o `Ffi_GetFfiNativeResolver` da VM
/// resolve uma vez por função). Símbolo ausente: `ArgumentError`, como a VM.
///
/// # Safety
/// `nome` aponta para `n` bytes legíveis.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_ffi_simbolo_nativo(nome: *const u8, n: i64) -> i64 {
    // SAFETY: garantido por quem chama (uma constante do módulo).
    let nome = unsafe { std::slice::from_raw_parts(nome, usize::try_from(n).unwrap_or(0)) };
    let nome = String::from_utf8_lossy(nome).into_owned();
    static CACHE: std::sync::OnceLock<std::sync::Mutex<crate::hash::HashMap<String, i64>>> = std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(&e) = cache.lock().unwrap_or_else(|e| e.into_inner()).get(&nome) {
        return e;
    }
    let e = std::ffi::CString::new(nome.clone()).map_or(0, |c| procurar(HANDLE_DO_PROCESSO, &c) as i64);
    match e {
        e if e != 0 => {
            cache.lock().unwrap_or_else(|e| e.into_inner()).insert(nome, e);
            e
        }
        _ => {
            lancar_erro_de_argumento(&format!("Couldn't resolve native function '{nome}' in the process: symbol not found"));
            0
        }
    }
}

/// `Pointer<X>.asTypedList(n)`: a lista tipada interna `class_id` (elementos
/// `tipo`) sobre a memória nativa do ponteiro, sem cópia — escrever numa é
/// escrever na outra.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_typed_externo(class_id: i64, tipo: i64, ponteiro: i64, n: i64) -> i64 {
    let Some(endereco) = endereco_de(ponteiro) else {
        lancar_erro_de_argumento("a Pointer was expected");
        return 0;
    };
    let tipo = tipo as u8;
    if n < 0 {
        lancar_erro_de_argumento("length must be non-negative");
        return 0;
    }
    if endereco == 0 && n > 0 {
        lancar_erro_de_argumento("asTypedList on nullptr");
        return 0;
    }
    let tamanho = n as usize * tamanho_do_elemento(tipo);
    HEAP.with(|h| {
        h.borrow_mut().allocate(Value::TypedData {
            class_id,
            tipo,
            bytes: crate::heap::Armazenamento::Externo { endereco: endereco as usize, tamanho },
        })
    })
}

/// [`dartforge_typed_externo`] que registra a tabela de métodos da classe na
/// primeira alocação (como `dartforge_typed_novo_t`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_typed_externo_t(class_id: i64, tipo: i64, ponteiro: i64, n: i64, f: extern "C" fn() -> *const i64) -> i64 {
    dartforge_registrar_tabela(class_id, f);
    dartforge_typed_externo(class_id, tipo, ponteiro, n)
}

/// O endereço dos bytes de uma struct/union (um argumento por valor): a
/// base (`Pointer` ou `TypedData`) mais o deslocamento, conferido contra o
/// tamanho numa base `TypedData`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_endereco_do_composto(obj: i64) -> i64 {
    let dados = HEAP.with(|h| match h.borrow().try_get(obj) {
        Some(Value::Object { class_id, fields }) => {
            let c = compostos_ffi().read().unwrap_or_else(|e| e.into_inner()).values().find(|c| c.classe == *class_id).copied()?;
            let campo = |i: i64| usize::try_from(i).ok().and_then(|i| fields.get(i)).copied();
            Some((c, campo(c.indice_base)?, campo(c.indice_deslocamento)?))
        }
        _ => None,
    });
    let Some((c, (base, _), (deslocamento, e_ref))) = dados else {
        lancar_erro_de_argumento("a Struct or Union was expected in a native call");
        return 0;
    };
    let deslocamento = if e_ref { HEAP.with(|h| h.borrow().int_de_ref(deslocamento)).unwrap_or(0) } else { deslocamento };
    com_memoria(base, deslocamento, c.tamanho as usize, |p| p as i64).unwrap_or(0)
}

/// Uma struct/union nova, sobre um `Uint8List` do tamanho dela (o valor que
/// uma chamada nativa devolve por valor, como na VM).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_composto_novo(rti: i64) -> i64 {
    let Some(c) = composto_de_rti(rti) else {
        lancar_unsupported("struct or union not registered in the DartForge native backend");
        return 0;
    };
    let Some(cid) = cid_registrado(CID_UINT8_LIST) else {
        lancar_unsupported("struct by value needs the Dart SDK compiled from source");
        return 0;
    };
    let bytes = HEAP.with(|h| {
        h.borrow_mut().allocate(Value::TypedData { class_id: cid, tipo: TIPO_UINT8, bytes: vec![0u8; c.tamanho as usize].into() })
    });
    com_raizes(&[bytes], || dartforge_ffi_composto(rti, bytes, 0))
}

/// Uma struct/union nova (sobre `Uint8List`) com os bytes em `endereco` — o
/// argumento por valor de um callback, copiado como na VM.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_composto_copia(rti: i64, endereco: i64) -> i64 {
    let novo = dartforge_ffi_composto_novo(rti);
    if novo == 0 {
        return 0;
    }
    let n = composto_de_rti(rti).map_or(0, |c| c.tamanho as usize);
    let destino = com_raizes(&[novo], || dartforge_ffi_endereco_do_composto(novo));
    if destino != 0 && endereco != 0 {
        // SAFETY: `endereco` tem os `n` bytes da struct (a entrada C ou uma
        // mensagem); `destino`, os da struct nova.
        unsafe { std::ptr::copy_nonoverlapping(endereco as usize as *const u8, destino as usize as *mut u8, n) };
    }
    novo
}

/// Copia os `n` bytes da struct devolvida por um callback (em `origem`; 0 se
/// a closure lançou) para o retorno da entrada C.
///
/// # Safety
/// `destino` tem `n` bytes graváveis; `origem` é 0 ou tem `n` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_ffi_copiar_composto(destino: *mut u8, origem: i64, n: i64) {
    if origem != 0 && n > 0 {
        // SAFETY: garantido por quem chama.
        unsafe { std::ptr::copy_nonoverlapping(origem as usize as *const u8, destino, n as usize) };
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
    static CELULAS_DE_HANDLE: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_handles_abrir() -> i64 {
    CELULAS_DE_HANDLE.with(|c| c.borrow().len() as i64)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_handles_fechar(escopo: i64) {
    let soltas: Vec<usize> = CELULAS_DE_HANDLE.with(|c| {
        let mut c = c.borrow_mut();
        let de = (escopo.max(0) as usize).min(c.len());
        c.split_off(de)
    });
    for celula in soltas {
        HEAP.with(|h| h.borrow_mut().soltar_raiz_global(celula as i64));
        // SAFETY: a célula foi criada por `dartforge_ffi_handle_novo` e sai
        // do registro uma vez.
        unsafe { drop(Box::from_raw(celula as *mut i64)) };
    }
}

/// O `Handle` de `obj`: o endereço de uma célula nova do escopo corrente.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_handle_novo(obj: i64) -> i64 {
    let celula = Box::into_raw(Box::new(obj)) as usize;
    HEAP.with(|h| h.borrow_mut().set_global_root(celula as i64, obj));
    CELULAS_DE_HANDLE.with(|c| c.borrow_mut().push(celula));
    celula as i64
}

/// O objeto de um `Handle` devolvido pelo C.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_objeto_do_handle(handle: i64) -> i64 {
    let vivo = CELULAS_DE_HANDLE.with(|c| c.borrow().iter().rev().any(|&x| x as i64 == handle));
    if !vivo {
        lancar_erro_de_argumento("the native code returned a Handle that is not alive in this isolate");
        return 0;
    }
    // SAFETY: a célula está viva (registrada neste isolado).
    unsafe { *(handle as usize as *const i64) }
}
