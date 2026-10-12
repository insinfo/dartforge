//! Fontes Dart geradas, mantidas em memória.
//!
//! Um projeto ngdart importa `foo.template.dart`, que não existe no disco: o
//! `build_runner` o escreve em `.dart_tool/build/generated`. Aqui essas fontes
//! passam a viver em memória, indexadas pelo caminho **natural** — o caminho ao
//! lado do arquivo que as originou, `lib/src/x/foo.template.dart` — que é
//! exatamente o que `resolve_package_uri` devolve para a URI importada. Assim
//! nada muda na resolução nem no mapeamento de volta para `package:`: só a
//! leitura consulta esta tabela antes do disco.
//!
//! Uma geração é imutável e trocada inteira de uma vez. Isso é o que garante
//! que o navegador nunca veja estado intermediário: ou a geração fechou sem
//! erro e o módulo JS sai coerente com ela, ou a anterior continua valendo.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Uma fonte Dart produzida por um gerador.
#[derive(Debug, Clone)]
pub struct GeneratedSource {
    pub conteudo: Arc<str>,
    /// Arquivos que a produziram (`.dart`, `.html`, `.css`): é por eles que o
    /// observador do `serve` sabe o que invalidar.
    pub entradas: Arc<[PathBuf]>,
    pub gerador: &'static str,
}

/// Conjunto imutável de fontes geradas.
#[derive(Debug, Default)]
pub struct Geracao {
    pub id: u64,
    sources: HashMap<PathBuf, GeneratedSource>,
}

/// Normaliza o caminho para servir de chave: lexical (sem tocar no disco, que
/// o arquivo não existe) e sem o prefixo verbatim do Windows.
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

/// Acumula uma geração. Só vira `Geracao` se fechar sem erro.
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

    /// Fecha a geração. Com qualquer erro nada é publicado — quem chamou segue
    /// com a geração anterior.
    pub fn concluir(self, id: u64) -> Result<Arc<Geracao>, Vec<String>> {
        if !self.errors.is_empty() {
            return Err(self.errors);
        }
        Ok(Arc::new(Geracao { id, sources: self.sources }))
    }
}

/// Geração montada a partir do que o `build_runner` já escreveu em
/// `.dart_tool/build/generated`, mapeando cada arquivo para o seu caminho
/// natural ao lado da fonte.
///
/// Serve para verificar o encanamento separado do gerador: com a mesma
/// entrada, compilar lendo daqui tem de dar byte a byte o mesmo JS de
/// compilar lendo do disco. Quando o gerador Angular em Rust entrar, só muda
/// quem produz as strings.
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
    for (nome, pkg) in &cfg.packages {
        if packages_.is_some_and(|p| !p.contains(nome)) {
            continue;
        }
        let dir = raiz.join(nome).join("lib");
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
                let nome = p.to_string_lossy();
                if !suffixes.iter().any(|s| nome.ends_with(s)) {
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
                    Err(e) => c.erro(format!("não foi possível ler {}: {e}", p.display())),
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
                eprintln!("aviso: {m}");
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
        assert!(!g.contem(Path::new("/p/lib/outro.dart")));
    }

    #[test]
    fn error_aborts_generation() {
        let mut c = Construtor::nova();
        c.por(PathBuf::from("/p/lib/foo.template.dart"), "// x", "teste", vec![]);
        c.erro("template quebrado".into());
        assert!(c.concluir(2).is_err());
    }
}
