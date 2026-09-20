//! Valida fronteiras reais entre bibliotecas para os modificadores Dart 3.6.2.
use dartforge_compiler::{Optimization, compile_path, compile_path_llvm};
use std::path::PathBuf;

/// Isola duas bibliotecas sem depender de arquivos de referência externos.
struct Libraries(PathBuf);
impl Libraries {
    /// Cria biblioteca de origem e consumidora em diretório exclusivo.
    fn new(definition: &str, consumer: &str) -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "dartforge-modifiers-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("api.dart"), definition).unwrap();
        std::fs::write(
            root.join("main.dart"),
            format!("import 'api.dart'; {consumer}"),
        )
        .unwrap();
        Self(root)
    }
}
impl Drop for Libraries {
    /// Remove exclusivamente a pasta criada para este fixture.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Extends e implements respeitam o modificador mesmo com corpos vazios.
#[test]
fn cross_library_forbidden_edges_are_diagnosed_by_both_backends() {
    for (definition, consumer, expected) in [
        (
            "interface class Api {}",
            "class C extends Api {} void main() {}",
            "interface",
        ),
        (
            "base class Api {}",
            "base class C implements Api {} void main() {}",
            "base",
        ),
        (
            "final class Api {}",
            "final class C extends Api {} void main() {}",
            "final",
        ),
        (
            "final class Api {}",
            "final class C implements Api {} void main() {}",
            "final",
        ),
        (
            "sealed class Api {}",
            "class C extends Api {} void main() {}",
            "sealed",
        ),
        (
            "sealed class Api {}",
            "class C implements Api {} void main() {}",
            "sealed",
        ),
        (
            "base class Api {}",
            "class C extends Api {} void main() {}",
            "base",
        ),
    ] {
        let files = Libraries::new(definition, consumer);
        let path = files.0.join("main.dart");
        for result in [
            compile_path(&path, Optimization::None),
            compile_path_llvm(&path),
        ] {
            let error = result.expect_err(consumer);
            assert!(error.message.to_lowercase().contains(expected), "{error:?}");
            assert!(error.path.ends_with("main.dart"), "{error:?}");
        }
    }
}

/// Bibliotecas podem consumir interface e base pelas arestas permitidas.
#[test]
fn cross_library_permitted_edges_compile_for_js_and_llvm() {
    for (definition, consumer) in [
        (
            "interface class Api { int value() => 1; }",
            "class C implements Api { int value() => 2; } void main() { print(C().value()); }",
        ),
        (
            "base class Api { int value() => 1; }",
            "base class C extends Api {} void main() { print(C().value()); }",
        ),
        (
            "final class Api { int value() => 1; }",
            "void main() { print(Api().value()); }",
        ),
    ] {
        let files = Libraries::new(definition, consumer);
        let path = files.0.join("main.dart");
        compile_path(&path, Optimization::None).unwrap();
        compile_path_llvm(&path).unwrap();
    }
}
