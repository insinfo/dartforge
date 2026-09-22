//! A ponte do gerador para o banco semântico.
//!
//! O compilador oficial pergunta ao `package:analyzer` onde um tipo é
//! declarado; nós perguntamos ao nosso `Program`. É a mesma pergunta e a
//! resposta tem de ser a mesma: para `OidcService` o import gerado é o da
//! biblioteca que **declara** o tipo, não a que o reexporta — por isso sai
//! `package:ngrouter/src/router/router.dart` e não
//! `package:ngrouter/ngrouter.dart`.
//!
//! O caminho do import é montado pela regra do `getImportModulePath` do
//! `ngcompiler` (`output/path_util.dart`): mesmo pacote e mesma pasta de
//! primeiro nível viram caminho relativo; o resto vira `package:`.
use dartforge_elements::model::{Element, LibraryId, Program};
use dartforge_intern::Interner;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// O que o emissor precisa perguntar sobre nomes. É um traço para que o
/// teste possa responder sem carregar um programa inteiro.
pub trait Resolucao {
    /// URI da biblioteca que declara `nome` no escopo de `arquivo`.
    fn uri_do_tipo(&self, arquivo: &Path, nome: &str) -> Option<String>;
}

pub struct Resolvedor<'a> {
    program: &'a Program,
    interner: &'a Interner,
    /// Caminho de cada unidade principal para a sua biblioteca.
    por_caminho: HashMap<PathBuf, LibraryId>,
}

impl<'a> Resolvedor<'a> {
    pub fn novo(program: &'a Program, interner: &'a Interner) -> Self {
        let mut por_caminho = HashMap::new();
        for unidade in &program.units {
            let Some(caminho) = &unidade.path else { continue };
            por_caminho
                .entry(dartforge_elements::gerado::chave(caminho))
                .or_insert(unidade.library);
        }
        Self { program, interner, por_caminho }
    }

    /// Cada biblioteca carregada: URI, árvore, unidade, fonte e caminho.
    ///
    /// É daqui que sai o índice de componentes de **todos** os pacotes — sem
    /// isto, um `<li-select>` do limitless_ui seria só uma tag desconhecida.
    pub fn bibliotecas(
        &self,
    ) -> impl Iterator<
        Item = (
            &'a str,
            &'a dartforge_frontend::ast::Ast,
            &'a dartforge_frontend::ast::CompilationUnit,
            &'a str,
            Option<&'a Path>,
        ),
    > + '_ {
        self.program.units.iter().filter_map(move |u| {
            let lib = self.program.library(u.library);
            if lib.is_sdk {
                return None;
            }
            Some((lib.uri.as_str(), &u.ast, &u.unit, u.source.as_str(), u.path.as_deref()))
        })
    }

    /// O interner da carga, para ler os nomes das árvores acima.
    pub fn interner(&self) -> &'a Interner {
        self.interner
    }

    /// Biblioteca do arquivo, se ele foi carregado.
    pub fn biblioteca(&self, arquivo: &Path) -> Option<LibraryId> {
        self.por_caminho.get(&dartforge_elements::gerado::chave(arquivo)).copied()
    }

    fn procurar(&self, arquivo: &Path, nome: &str) -> Option<&'a str> {
        let lib = self.biblioteca(arquivo)?;
        let biblioteca = self.program.library(lib);
        let (prefixo, simples) = match nome.split_once('.') {
            Some((p, t)) => (Some(p), t),
            None => (None, nome),
        };
        let sym = self.interner.lookup(simples)?;
        let espaco = match prefixo {
            None => &biblioteca.scope,
            Some(p) => {
                let psym = self.interner.lookup(p)?;
                biblioteca.prefixes.get(&psym)?
            }
        };
        let ligacao = espaco.get(&sym)?;
        if ligacao.ambiguous {
            return None;
        }
        match ligacao.getter? {
            Element::Class(id) => {
                let dona = self.program.class(id).library;
                Some(self.program.library(dona).uri.as_str())
            }
            Element::Typedef(id) => {
                let dona = self.program.typedef(id).library;
                Some(self.program.library(dona).uri.as_str())
            }
            _ => None,
        }
    }
}

/// URI `asset:<pacote>/<pasta>/<resto>` de uma URI de biblioteca.
///
/// O emissor oficial trabalha nesse espaço; `package:x/y` é
/// `asset:x/lib/y`.
pub fn asset_de_uri(uri: &str, pacote_do_projeto: &str, raiz: &Path) -> Option<String> {
    if let Some(resto) = uri.strip_prefix("package:") {
        let (pkg, caminho) = resto.split_once('/')?;
        return Some(format!("asset:{pkg}/lib/{caminho}"));
    }
    // `file:///…/web/main.dart` do próprio projeto.
    let caminho = uri.strip_prefix("file:///").map(|c| c.replace('/', "\\"))?;
    let rel = Path::new(&caminho).strip_prefix(raiz).ok()?;
    let rel = rel.to_string_lossy().replace('\\', "/");
    Some(format!("asset:{pacote_do_projeto}/{rel}"))
}

/// `getImportModulePath` do `ngcompiler`: como um arquivo gerado em
/// `modulo` escreve o import de `importado`, ambos em URIs `asset:`.
pub fn caminho_do_import(modulo: &str, importado: &str) -> Option<String> {
    let m = Asset::analisar(modulo)?;
    let i = Asset::analisar(importado)?;
    if modulo == importado {
        return i.caminho.rsplit('/').next().map(str::to_string);
    }
    if m.pasta == i.pasta && m.pacote == i.pacote {
        return Some(relativo(&m.caminho, &i.caminho));
    }
    if i.pasta == "lib" {
        return Some(format!("package:{}/{}", i.pacote, i.caminho));
    }
    None
}

struct Asset<'a> {
    pacote: &'a str,
    /// `lib`, `web`, `test`.
    pasta: &'a str,
    /// O que vem depois da pasta.
    caminho: &'a str,
}

impl<'a> Asset<'a> {
    fn analisar(uri: &'a str) -> Option<Self> {
        let resto = uri.strip_prefix("asset:")?;
        let (pacote, resto) = resto.split_once('/')?;
        let (pasta, caminho) = resto.split_once('/')?;
        Some(Asset { pacote, pasta, caminho })
    }
}

/// Caminho relativo de `modulo` para `importado`, contando segmentos.
fn relativo(modulo: &str, importado: &str) -> String {
    let m: Vec<&str> = modulo.split('/').collect();
    let i: Vec<&str> = importado.split('/').collect();
    let mut prefixo = 0;
    while prefixo < m.len().min(i.len()) && m[prefixo] == i[prefixo] {
        prefixo += 1;
    }
    let subir = m.len().saturating_sub(1).saturating_sub(prefixo);
    let mut partes: Vec<&str> = vec![".."; subir];
    partes.extend_from_slice(&i[prefixo..]);
    partes.join("/")
}

/// Aceita `T` e `p.T`: o prefixo é procurado no namespace do próprio
/// prefixo, como manda a resolução do Dart.
impl Resolucao for Resolvedor<'_> {
    fn uri_do_tipo(&self, arquivo: &Path, nome: &str) -> Option<String> {
        self.procurar(arquivo, nome).map(str::to_string)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// O caso do `CallbackComponent`: o oficial escreveu
    /// `'../../../../shared/services/oidc_service.dart'` e
    /// `'package:ngrouter/src/router/router.dart'`.
    #[test]
    fn caminhos_como_no_oficial() {
        let modulo = "asset:new_sali_frontend/lib/src/modules/auth/pages/callback/callback_component.dart";
        assert_eq!(
            caminho_do_import(
                modulo,
                "asset:new_sali_frontend/lib/src/shared/services/oidc_service.dart"
            )
            .unwrap(),
            "../../../../shared/services/oidc_service.dart"
        );
        assert_eq!(
            caminho_do_import(modulo, "asset:ngrouter/lib/src/router/router.dart").unwrap(),
            "package:ngrouter/src/router/router.dart"
        );
        assert_eq!(caminho_do_import(modulo, modulo).unwrap(), "callback_component.dart");
    }

    #[test]
    fn asset_de_package() {
        assert_eq!(
            asset_de_uri("package:ngrouter/src/router/router.dart", "x", Path::new("/p")).unwrap(),
            "asset:ngrouter/lib/src/router/router.dart"
        );
    }
}
