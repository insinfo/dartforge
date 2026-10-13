//! Generated Dart sources, kept in memory.
//!
//! An ngdart project imports `foo.template.dart`, which does not exist on
//! disk: `build_runner` writes it to `.dart_tool/build/generated`. Here those
//! sources live in memory, indexed by the **natural** path — the path next
//! to the file that produced them, `lib/src/x/foo.template.dart` — which is
//! exactly what `resolve_package_uri` returns for the imported URI. That way
//! nothing changes in resolution or in the mapping back to `package:`: only
//! reading consults this table before the disk.
//!
//! A generation is immutable and replaced whole at once. That is what
//! guarantees the browser never sees an intermediate state: either the
//! generation closed without errors and the JS module comes out consistent
//! with it, or the previous one stays in effect.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A Dart source produced by a generator.
#[derive(Debug, Clone)]
pub struct GeneratedSource {
    pub conteudo: Arc<str>,
    /// Files that produced it (`.dart`, `.html`, `.css`): through them the
    /// `serve` watcher knows what to invalidate.
    pub entradas: Arc<[PathBuf]>,
    pub gerador: &'static str,
}

/// Immutable set of generated sources.
#[derive(Debug, Default)]
pub struct Geracao {
    pub id: u64,
    sources: HashMap<PathBuf, GeneratedSource>,
}

/// Normalizes the path to serve as a key: lexically (without touching the
/// disk, since the file does not exist) and without Windows' verbatim
/// prefix.
pub fn chave(caminho: &Path) -> PathBuf {
    crate::config::sem_verbatim(crate::load::normalize(caminho))
}

impl Geracao {
    pub fn obter(&self, caminho: &Path) -> Option<&GeneratedSource> {
        self.sources.get(&chave(caminho))
    }

    pub fn contem(&self, caminho: &Path) -> bool {
        self.sources.contains_key(&chave(caminho))
    }

    pub fn vazia(&self) -> bool {
        self.sources.is_empty()
    }

    pub fn len(&self) -> usize {
        self.sources.len()
    }

    pub fn caminhos(&self) -> impl Iterator<Item = &PathBuf> {
        self.sources.keys()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&PathBuf, &GeneratedSource)> {
        self.sources.iter()
    }
}

/// Accumulates a generation. It only becomes a `Geracao` if it closes
/// without errors.
#[derive(Default)]
pub struct Construtor {
    sources: HashMap<PathBuf, GeneratedSource>,
    errors: Vec<String>,
}

impl Construtor {
    pub fn nova() -> Self {
        Self::default()
    }

    pub fn por(
        &mut self,
        caminho: PathBuf,
        conteudo: impl Into<Arc<str>>,
        gerador: &'static str,
        entradas: Vec<PathBuf>,
    ) {
        self.sources.insert(
            chave(&caminho),
            GeneratedSource { conteudo: conteudo.into(), entradas: entradas.into(), gerador },
        );
    }

    pub fn contem(&self, caminho: &Path) -> bool {
        self.sources.contains_key(&chave(caminho))
    }

    pub fn erro(&mut self, message: String) {
        self.errors.push(message);
    }

    /// Closes the generation. With any error nothing is published — the caller
    /// keeps the previous generation.
    pub fn concluir(self, id: u64) -> Result<Arc<Geracao>, Vec<String>> {
        if !self.errors.is_empty() {
            return Err(self.errors);
        }
        Ok(Arc::new(Geracao { id, sources: self.sources }))
    }
}

/// Generation built from what `build_runner` already wrote to
/// `.dart_tool/build/generated`, mapping each file to its natural path next
/// to the source.
///
/// Used to verify the generator's separate plumbing: with the same input,
/// compiling while reading from here must give byte for byte the same JS as
/// compiling while reading from disk. When the Angular generator in Rust
/// arrives, only who produces the strings changes.
pub fn do_build_runner(
    cfg: &crate::config::PackageConfig,
    suffixes: &[&str],
    packages_: Option<&std::collections::HashSet<String>>,
) -> Arc<Geracao> {
    let mut c = Construtor::nova();
    let Some(origin) = cfg.origin.as_ref().and_then(|o| o.parent()) else {
        return c.concluir(0).unwrap_or_default_arc();
    };
    let raiz = origin.join("build").join("generated");
    for (name_, pkg) in &cfg.packages {
        if packages_.is_some_and(|p| !p.contains(name_)) {
            continue;
        }
        let dir = raiz.join(name_).join("lib");
        if !dir.is_dir() {
            continue;
        }
        let mut stack = vec![dir.clone()];
        while let Some(d) = stack.pop() {
            let Ok(entradas) = std::fs::read_dir(&d) else { continue };
            for e in entradas.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                let name_ = p.to_string_lossy();
                if !suffixes.iter().any(|s| name_.ends_with(s)) {
                    continue;
                }
                let Ok(rel) = p.strip_prefix(&dir) else { continue };
                let rel = rel.to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/");
                let Ok(destination) = pkg.package_uri.join(&rel).and_then(|u| {
                    u.to_file_path().map_err(|_| url::ParseError::RelativeUrlWithoutBase)
                }) else {
                    continue;
                };
                match std::fs::read_to_string(&p) {
                    Ok(text) => c.por(destination, text, "build_runner", vec![p]),
                    Err(e) => c.erro(format!("could not read {}: {e}", p.display())),
                }
            }
        }
    }
    c.concluir(0).unwrap_or_default_arc()
}

trait OrEmpty {
    fn unwrap_or_default_arc(self) -> Arc<Geracao>;
}

impl OrEmpty for Result<Arc<Geracao>, Vec<String>> {
    fn unwrap_or_default_arc(self) -> Arc<Geracao> {
        self.unwrap_or_else(|e| {
            for m in e {
                eprintln!("warning: {m}");
            }
            Arc::new(Geracao::default())
        })
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn key_is_lexical() {
        let g = {
            let mut c = Construtor::nova();
            c.por(PathBuf::from("/p/lib/./a/../foo.template.dart"), "// x", "teste", vec![]);
            c.concluir(1).unwrap()
        };
        assert!(g.contem(Path::new("/p/lib/foo.template.dart")));
        assert_eq!(g.obter(Path::new("/p/lib/foo.template.dart")).unwrap().conteudo.as_ref(), "// x");
        assert!(!g.contem(Path::new("/p/lib/other.dart")));
    }

    #[test]
    fn error_aborts_generation() {
        let mut c = Construtor::nova();
        c.por(PathBuf::from("/p/lib/foo.template.dart"), "// x", "teste", vec![]);
        c.erro("broken template".into());
        assert!(c.concluir(2).is_err());
    }
}
