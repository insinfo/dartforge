// Runtime nativo: o subconjunto de `dart:mirrors` que os builders usam (B01).
//
// O `TypeChecker.fromRuntime(T)` do `source_gen` (json_serializable, freezed,
// built_value…) pede `reflectClass(T)` e lê só o nome da classe e a URI da
// biblioteca que a declara (`(mirror.owner as LibraryMirror).uri`,
// `MirrorSystem.getName(mirror.simpleName)`). O resto da API de reflexão não
// existe no nativo (como no AOT da VM) e lança `UnsupportedError`
// (`sdk_nativo/mirrors/mirrors_patch.dart`).
//
// O compilador registra, na tabela da RTI (`lower/rti.rs`), uma linha
// `B<id> <uri> <nome>` por classe: a biblioteca que a declara e o nome
// declarado (não o visível: `_Smi` continua `_Smi`).

thread_local! {
    /// Classe RTI → (URI da biblioteca, nome declarado).
    static BIBLIOTECAS_DAS_CLASSES: RefCell<HashMap<i64, (String, String)>> = RefCell::new(HashMap::default());
}

/// Registra a biblioteca e o nome declarado da classe RTI `classe` (linha
/// `B` da tabela da RTI).
fn registrar_biblioteca_da_classe(classe: i64, uri: &str, nome: &str) {
    BIBLIOTECAS_DAS_CLASSES.with(|m| {
        m.borrow_mut().insert(classe, (uri.to_string(), nome.to_string()));
    });
}

/// A classe RTI de um objeto `Type` de classe (`Tipo::Interface`, também
/// anulável), ou `None` para os outros tipos (função, record, `dynamic`…).
fn classe_do_objeto_tipo(tipo: i64) -> Option<i64> {
    let t = HEAP.with(|h| match h.borrow().try_get(tipo) {
        Some(Value::Object { class_id, fields }) if *class_id == CLASSE_TIPO => fields.first().map(|f| f.0),
        _ => None,
    })?;
    RTI.with(|u| {
        let u = u.borrow();
        let mut t = t;
        loop {
            match u.tipos.get(t as usize)? {
                Tipo::Interface(c, _) => return Some(*c),
                Tipo::Anulavel(x) => t = *x,
                _ => return None,
            }
        }
    })
}

/// `DartForge_mirrors_uri`: a URI da biblioteca da classe do objeto `Type`, ou
/// `null` quando o tipo não é de classe (ou a classe não foi registrada).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_mirrors_uri(tipo: i64) -> i64 {
    match classe_do_objeto_tipo(tipo).and_then(|c| BIBLIOTECAS_DAS_CLASSES.with(|m| m.borrow().get(&c).cloned())) {
        Some((uri, _)) => alocar_str(&uri),
        None => 0,
    }
}

/// `DartForge_mirrors_nome`: o nome declarado da classe do objeto `Type`,
/// ou `null` (ver [`dartforge_nativo_DartForge_mirrors_uri`]).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_mirrors_nome(tipo: i64) -> i64 {
    match classe_do_objeto_tipo(tipo).and_then(|c| BIBLIOTECAS_DAS_CLASSES.with(|m| m.borrow().get(&c).cloned())) {
        Some((_, nome)) => alocar_str(&nome),
        None => 0,
    }
}
