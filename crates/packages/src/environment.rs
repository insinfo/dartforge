//! Perfis de seleção de URIs condicionais do SDK Dart 3.6.2.
//! Uma flag descreve o SDK de referência; não promete uma biblioteca implementada pelo DartForge.
use std::collections::BTreeMap;

/// Plataforma de referência; Native corresponde a AOT, não ao VM JIT com mirrors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompilationTarget {
    JavaScript,
    Native,
    Wasm,
}

/// Ambiente explícito, determinístico e independente das variáveis do processo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilationEnvironment {
    target: CompilationTarget,
}

impl Default for CompilationEnvironment {
    /// Preserva JavaScript como alvo padrão do resolvedor de pacotes.
    fn default() -> Self {
        Self::javascript()
    }
}

impl CompilationEnvironment {
    /// Seleciona as bibliotecas anunciadas pelo dart2js 3.6.2.
    pub fn javascript() -> Self {
        Self {
            target: CompilationTarget::JavaScript,
        }
    }
    /// Seleciona as bibliotecas anunciadas pelo AOT 3.6.2; mirrors permanece ausente.
    pub fn native() -> Self {
        Self {
            target: CompilationTarget::Native,
        }
    }
    /// Seleciona URIs para dart2wasm 3.6.2, sem habilitar um backend Wasm no DartForge.
    pub fn wasm() -> Self {
        Self {
            target: CompilationTarget::Wasm,
        }
    }
    /// Retorna o perfil utilizado na resolução.
    pub fn target(&self) -> CompilationTarget {
        self.target
    }
    /// Consulta uma flag reservada de biblioteca suportada; ausência é None.
    pub fn get(&self, name: &str) -> Option<&str> {
        if let Some(library) = name.strip_prefix("dart.library.") {
            return self.supports(library).then_some("true");
        }
        None
    }
    /// Avalia uma condição de URI conforme o frontend 3.6.2: bare equivale a == 'true'.
    /// Nomes comuns valem string vazia nesta operação, mesmo quando definidos por -D.
    /// Bibliotecas ausentes usam None e portanto nunca correspondem, nem a string vazia.
    /// ```
    /// let env = dartforge_packages::CompilationEnvironment::wasm();
    /// assert!(env.condition("dart.library.js_util", None));
    /// assert!(env.condition("custom.absent", Some("")));
    /// assert!(!env.condition("dart.library.unknown", Some("")));
    /// ```
    pub fn condition(&self, name: &str, expected: Option<&str>) -> bool {
        let actual = if name.starts_with("dart.library.") {
            self.get(name)
        } else {
            Some("")
        };
        actual == Some(expected.unwrap_or("true"))
    }
    /// Expõe o inventário público pesquisado; inclui a exceção privada _dart2js_only do alvo JavaScript.
    /// False representa ausência, não a string 'false' na avaliação das condições.
    pub fn library_flags(&self) -> BTreeMap<&'static str, bool> {
        LIBRARIES
            .iter()
            .map(|&name| (name, self.supports(name)))
            .collect()
    }
    /// Aplica a matriz obtida de libraries.json e validada por execução dos três backends SDK.
    fn supports(&self, name: &str) -> bool {
        if matches!(
            name,
            "async" | "collection" | "convert" | "core" | "developer" | "math" | "typed_data"
        ) {
            return true;
        }
        match self.target {
            CompilationTarget::JavaScript => matches!(
                name,
                "_dart2js_only"
                    | "html"
                    | "html_common"
                    | "indexed_db"
                    | "js"
                    | "js_interop"
                    | "js_interop_unsafe"
                    | "js_util"
                    | "svg"
                    | "web_audio"
                    | "web_gl"
            ),
            CompilationTarget::Native => matches!(
                name,
                "cli" | "concurrent" | "ffi" | "io" | "isolate" | "nativewrappers" | "vmservice_io"
            ),
            CompilationTarget::Wasm => matches!(
                name,
                "isolate" | "js_interop" | "js_interop_unsafe" | "js_util" | "nativewrappers"
            ),
        }
    }
}

/// União pesquisada de nomes públicos dos perfis SDK, mais chaves comuns indisponíveis.
const LIBRARIES: &[&str] = &[
    "_dart2js_only",
    "_ddc_only",
    "async",
    "cli",
    "collection",
    "concurrent",
    "convert",
    "core",
    "developer",
    "ffi",
    "html",
    "html_common",
    "indexed_db",
    "io",
    "isolate",
    "js",
    "js_interop",
    "js_interop_unsafe",
    "js_util",
    "math",
    "mirrors",
    "nativewrappers",
    "svg",
    "typed_data",
    "ui",
    "vmservice_io",
    "wasm",
    "web_audio",
    "web_gl",
];

#[cfg(test)]
mod tests {
    use super::*;
    /// Reproduz diferenças observadas executando dart2js, AOT e dart2wasm 3.6.2.
    #[test]
    fn profiles_follow_the_pinned_sdk() {
        let js = CompilationEnvironment::javascript();
        let native = CompilationEnvironment::native();
        let wasm = CompilationEnvironment::wasm();
        for env in [&js, &native, &wasm] {
            assert_eq!(env.get("dart.library.core"), Some("true"));
            assert_eq!(env.get("dart.library._internal"), None);
            assert_eq!(env.get("dart.library.unknown"), None);
        }
        assert!(js.condition("dart.library._dart2js_only", None));
        assert!(!native.condition("dart.library._dart2js_only", None));
        assert!(js.condition("dart.library.html", None));
        assert!(!native.condition("dart.library.html", None));
        assert!(!wasm.condition("dart.library.html", None));
        assert!(native.condition("dart.library.io", None));
        assert!(!js.condition("dart.library.io", None));
        assert!(!wasm.condition("dart.library.io", None));
        assert!(wasm.condition("dart.library.js_util", None));
        assert!(wasm.condition("dart.library.isolate", None));
        assert!(!native.condition("dart.library.mirrors", None));
        assert_ne!(js, native);
        assert_eq!(CompilationEnvironment::default(), js);
    }
    /// Distingue a string vazia comum do sentinel reservado de bibliotecas ausentes.
    #[test]
    fn uri_conditions_do_not_consume_custom_defines() {
        let env = CompilationEnvironment::javascript();
        assert!(env.condition("custom.absent", Some("")));
        assert!(!env.condition("custom.absent", None));
        assert!(!env.condition("dart.library.io", Some("")));
        assert!(!env.condition("dart.library.io", Some("false")));
        assert!(!env.condition("dart.library.core", Some("TRUE")));
    }
}
