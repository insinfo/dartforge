// Runtime nativo: o que um programa vê na VM JIT e o AOT da VM não dá, e de
// que os builders do ecossistema dependem (B01). Duas partes:
//
// * o package config (`Isolate.resolvePackageUri`, `Isolate.packageConfig`):
//   na VM vem do `--packages`; aqui, de `DARTFORGE_PACKAGE_CONFIG` (o
//   hospedeiro do executor de builders o define). O `build_resolvers` acha
//   o diretório dos pacotes por ele. A leitura e a resolução ficam no patch
//   do `dart:isolate` (`native_sdk/isolate/isolate_patch.dart`), como no
//   `builtin.dart` da VM; o runtime só dá o caminho e o texto do arquivo.
//
// * o subconjunto de `dart:mirrors`:
//
// O `TypeChecker.fromRuntime(T)` do `source_gen` (json_serializable, freezed,
// built_value…) pede `reflectClass(T)` e lê só o nome da classe e a URI da
// biblioteca que a declara (`(mirror.owner as LibraryMirror).uri`,
// `MirrorSystem.getName(mirror.simpleName)`). O resto da API de reflexão não
// existe no nativo (como no AOT da VM) e lança `UnsupportedError`
// (`native_sdk/mirrors/mirrors_patch.dart`).
//
// O compilador registra, na tabela da RTI (`lower/rti.rs`), uma linha
// `B<id> <uri> <nome>` por classe: a biblioteca que a declara e o nome
// declarado (não o visível: `_Smi` continua `_Smi`).

thread_local! {
    /// Classe RTI → (URI da biblioteca, nome declarado).
    static CLASS_LIBRARIES: RefCell<HashMap<i64, (String, String)>> = RefCell::new(HashMap::default());
}

/// Registra a biblioteca e o nome declarado da classe RTI `class` (linha
/// `B` da tabela da RTI).
fn register_class_library(class: i64, uri: &str, name: &str) {
    CLASS_LIBRARIES.with(|m| {
        m.borrow_mut().insert(class, (uri.to_string(), name.to_string()));
    });
}

/// A classe RTI de um objeto `Type` de classe (`Type::Interface`, também
/// anulável), ou `None` para os outros tipos (função, record, `dynamic`…).
fn type_object_class(tipo: i64) -> Option<i64> {
    let t = HEAP.with(|h| match h.borrow().object(tipo) {
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

/// `DartForge_mirrors_uri`: a URI da biblioteca da classe do objeto `Type`, ou
/// `null` quando o tipo não é de classe (ou a classe não foi registrada).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_mirrors_uri(tipo: i64) -> i64 {
    match type_object_class(tipo).and_then(|c| CLASS_LIBRARIES.with(|m| m.borrow().get(&c).cloned())) {
        Some((uri, _)) => HEAP.with(|h| h.borrow_mut().allocate_str(&uri)),
        None => 0,
    }
}

/// `DartForge_mirrors_nome`: o nome declarado da classe do objeto `Type`,
/// ou `null` (ver [`dartforge_nativo_DartForge_mirrors_uri`]).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_mirrors_nome(tipo: i64) -> i64 {
    match type_object_class(tipo).and_then(|c| CLASS_LIBRARIES.with(|m| m.borrow().get(&c).cloned())) {
        Some((_, name)) => HEAP.with(|h| h.borrow_mut().allocate_str(&name)),
        None => 0,
    }
}

/// `DartForge_package_config`: o caminho do `package_config.json` do
/// processo (`DARTFORGE_PACKAGE_CONFIG`), ou `null`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_package_config() -> i64 {
    match std::env::var("DARTFORGE_PACKAGE_CONFIG") {
        Ok(c) if !c.is_empty() => HEAP.with(|h| h.borrow_mut().allocate_str(&c)),
        _ => 0,
    }
}

/// `DartForge_ler_texto`: o conteúdo UTF-8 do arquivo `file_path`, ou `null`
/// quando ele não pode ser lido.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ler_texto(file_path: i64) -> i64 {
    let file_path = HEAP.with(|h| h.borrow().text(file_path).map(|t| t.para_string()).unwrap_or_default());
    match std::fs::read_to_string(&file_path) {
        Ok(t) => HEAP.with(|h| h.borrow_mut().allocate_str(&t)),
        Err(_) => 0,
    }
}
