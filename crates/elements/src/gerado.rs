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
pub struct FonteGerada {
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
    fontes: HashMap<PathBuf, FonteGerada>,
}

/// Normaliza o caminho para servir de chave: lexical (sem tocar no disco, que
/// o arquivo não existe) e sem o prefixo verbatim do Windows.
pub fn chave(caminho: &Path) -> PathBuf {
    crate::config::sem_verbatim(crate::load::normalizar(caminho))
}

impl Geracao {
    pub fn obter(&self, caminho: &Path) -> Option<&FonteGerada> {
        self.fontes.get(&chave(caminho))
    }

    pub fn contem(&self, caminho: &Path) -> bool {
        self.fontes.contains_key(&chave(caminho))
    }

    pub fn vazia(&self) -> bool {
        self.fontes.is_empty()
    }

    pub fn len(&self) -> usize {
        self.fontes.len()
    }

    pub fn caminhos(&self) -> impl Iterator<Item = &PathBuf> {
        self.fontes.keys()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&PathBuf, &FonteGerada)> {
        self.fontes.iter()
    }
}

/// Acumula uma geração. Só vira `Geracao` se fechar sem erro.
#[derive(Default)]
pub struct Construtor {
    fontes: HashMap<PathBuf, FonteGerada>,
    erros: Vec<String>,
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
        self.fontes.insert(
            chave(&caminho),
            FonteGerada { conteudo: conteudo.into(), entradas: entradas.into(), gerador },
        );
    }

    pub fn contem(&self, caminho: &Path) -> bool {
        self.fontes.contains_key(&chave(caminho))
    }

    pub fn erro(&mut self, mensagem: String) {
        self.erros.push(mensagem);
    }

    /// Fecha a geração. Com qualquer erro nada é publicado — quem chamou segue
    /// com a geração anterior.
    pub fn concluir(self, id: u64) -> Result<Arc<Geracao>, Vec<String>> {
        if !self.erros.is_empty() {
            return Err(self.erros);
        }
        Ok(Arc::new(Geracao { id, fontes: self.fontes }))
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
    sufixo: &str,
    pacotes: Option<&std::collections::HashSet<String>>,
) -> Arc<Geracao> {
    let mut c = Construtor::nova();
    let Some(origem) = cfg.origin.as_ref().and_then(|o| o.parent()) else {
        return c.concluir(0).unwrap_or_default_arc();
    };
    let raiz = origem.join("build").join("generated");
    for (nome, pkg) in &cfg.packages {
        if pacotes.is_some_and(|p| !p.contains(nome)) {
            continue;
        }
        let dir = raiz.join(nome).join("lib");
        if !dir.is_dir() {
            continue;
        }
        let mut pilha = vec![dir.clone()];
        while let Some(d) = pilha.pop() {
            let Ok(entradas) = std::fs::read_dir(&d) else { continue };
            for e in entradas.flatten() {
                let p = e.path();
                if p.is_dir() {
                    pilha.push(p);
                    continue;
                }
                if !p.to_string_lossy().ends_with(sufixo) {
                    continue;
                }
                let Ok(rel) = p.strip_prefix(&dir) else { continue };
                let rel = rel.to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/");
                let Ok(destino) = pkg.package_uri.join(&rel).and_then(|u| {
                    u.to_file_path().map_err(|_| url::ParseError::RelativeUrlWithoutBase)
                }) else {
                    continue;
                };
                match std::fs::read_to_string(&p) {
                    Ok(texto) => c.por(destino, texto, "build_runner", vec![p]),
                    Err(e) => c.erro(format!("não foi possível ler {}: {e}", p.display())),
                }
            }
        }
    }
    c.concluir(0).unwrap_or_default_arc()
}

trait OuVazia {
    fn unwrap_or_default_arc(self) -> Arc<Geracao>;
}

impl OuVazia for Result<Arc<Geracao>, Vec<String>> {
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
    fn chave_e_lexical() {
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
    fn erro_aborta_a_geracao() {
        let mut c = Construtor::nova();
        c.por(PathBuf::from("/p/lib/foo.template.dart"), "// x", "teste", vec![]);
        c.erro("template quebrado".into());
        assert!(c.concluir(2).is_err());
    }
}
