// Native runtime: what a program sees in the JIT VM that the VM's AOT does not
// provide, and that the ecosystem's builders depend on (B01). Two parts:
//
// * the package config (`Isolate.resolvePackageUri`, `Isolate.packageConfig`):
//   in the VM it comes from `--packages`; here, from `DARTFORGE_PACKAGE_CONFIG`
//   (the host of the builder executor sets it). `build_resolvers` finds the
//   packages directory through it. Reading and resolution live in the
//   `dart:isolate` patch (`native_sdk/isolate/isolate_patch.dart`), as in the
//   VM's `builtin.dart`; the runtime only provides the path and the file text.
//
// * the `dart:mirrors` subset:
//
// `source_gen`'s `TypeChecker.fromRuntime(T)` (json_serializable, freezed,
// built_value…) asks for `reflectClass(T)` and reads only the class name and the
// URI of the library that declares it (`(mirror.owner as LibraryMirror).uri`,
// `MirrorSystem.getName(mirror.simpleName)`). The rest of the reflection API does
// not exist in native code (as in the VM's AOT) and throws `UnsupportedError`
// (`native_sdk/mirrors/mirrors_patch.dart`).
//
// The compiler registers, in the RTI table (`lower/rti.rs`), one line
// `B<id> <uri> <name>` per class: the library that declares it and the declared
// name (not the visible one: `_Smi` stays `_Smi`).

thread_local! {
    /// RTI class → (library URI, declared name).
    static CLASS_LIBRARIES: RefCell<HashMap<i64, (String, String)>> = RefCell::new(HashMap::default());
}

/// Registers the library and the declared name of RTI class `class` (line `B` of
/// the RTI table).
fn register_class_library(class: i64, uri: &str, name: &str) {
    CLASS_LIBRARIES.with(|m| {
        m.borrow_mut().insert(class, (uri.to_string(), name.to_string()));
    });
}

/// The RTI class of a class `Type` object (`Type::Interface`, nullable too), or
/// `None` for the other types (function, record, `dynamic`…).
fn type_object_class(ty: i64) -> Option<i64> {
    let t = HEAP.with(|h| match h.borrow().object(ty) {
        Some(o) if o.class_id == TYPE_CLASS => o.first().map(|f| f.0),
        _ => None,
    })?;
    RTI.with(|u| {
        let u = u.borrow();
        let mut t = t;
        loop {
            match u.get_value(t)? {
                Type::Interface(c, _) => return Some(*c),
                Type::Nullable(x) => t = *x,
                _ => return None,
            }
        }
    })
}

/// `DartForge_mirrors_uri`: the URI of the library of the `Type` object's class,
/// or `null` when the type is not a class type (or the class was not registered).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_mirrors_uri(ty: i64) -> i64 {
    match type_object_class(ty).and_then(|c| CLASS_LIBRARIES.with(|m| m.borrow().get(&c).cloned())) {
        Some((uri, _)) => HEAP.with(|h| h.borrow_mut().allocate_str(&uri)),
        None => 0,
    }
}

/// `DartForge_mirrors_name`: the declared name of the `Type` object's class, or
/// `null` (see [`dartforge_native_DartForge_mirrors_uri`]).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_mirrors_name(ty: i64) -> i64 {
    match type_object_class(ty).and_then(|c| CLASS_LIBRARIES.with(|m| m.borrow().get(&c).cloned())) {
        Some((_, name)) => HEAP.with(|h| h.borrow_mut().allocate_str(&name)),
        None => 0,
    }
}

fn program_package_config() -> &'static std::sync::Mutex<Option<Option<String>>> {
    static CONFIG: std::sync::Mutex<Option<Option<String>>> = std::sync::Mutex::new(None);
    &CONFIG
}

/// Selects the program's config without mutating the host process environment.
/// Explicit absence suppresses a stale inherited environment selection.
pub fn set_program_package_config(path: Option<String>) {
    *program_package_config().lock().unwrap_or_else(|error| error.into_inner()) = Some(path);
}

/// Sets the SDK library runtime's selected package config before main.
/// A zero-length payload explicitly selects no package configuration.
///
/// # Safety
/// `data` points to `size` readable UTF-8 bytes, or `size` is zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_set_program_package_config(data: *const u8, size: usize) {
    let path = if size == 0 {
        None
    } else {
        // SAFETY: the host keeps these bytes readable throughout this call.
        Some(String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(data, size) }).into_owned())
    };
    set_program_package_config(path);
}

/// `DartForge_package_config`: the explicitly selected program configuration,
/// otherwise the raw-IR/builder host's `DARTFORGE_PACKAGE_CONFIG`, or `null`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_package_config() -> i64 {
    let selected = program_package_config().lock().unwrap_or_else(|error| error.into_inner()).clone();
    let path = selected.unwrap_or_else(|| std::env::var("DARTFORGE_PACKAGE_CONFIG").ok());
    match path {
        Some(c) if !c.is_empty() => HEAP.with(|h| h.borrow_mut().allocate_str(&c)),
        _ => 0,
    }
}

/// `DartForge_read_text`: the UTF-8 content of the file `file_path`, or `null`
/// when it cannot be read.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_read_text(file_path: i64) -> i64 {
    let file_path = HEAP.with(|h| h.borrow().text(file_path).map(|t| t.to_dart_string()).unwrap_or_default());
    match std::fs::read_to_string(&file_path) {
        Ok(t) => HEAP.with(|h| h.borrow_mut().allocate_str(&t)),
        Err(_) => 0,
    }
}
