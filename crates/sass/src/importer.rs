//! Importadores: quem transforma a URL de um `@use`/`@forward`/`@import` na
//! URL canônica de uma folha e a carrega — o `Importer` do dart-sass
//! (`lib/src/importer.dart`).
//!
//! O grass original só procurava no disco a partir do caminho do arquivo
//! corrente e dos `load_paths`. Com um [`Importer`] nas [`crate::Options`],
//! a busca inteira passa por ele, como no `ImportCache.canonicalize` do
//! dart-sass: as URLs canônicas são textos (`package:pkg/a.scss`,
//! `asset:pkg/web/b.scss`), que o visitante guarda como `PathBuf` onde o
//! grass guardava caminhos.
use std::fmt::Debug;

use crate::InputSyntax;

/// Uma folha carregada por um [`Importer`].
#[derive(Debug, Clone)]
pub struct ImporterResult {
    /// O texto da folha.
    pub contents: String,
    /// A sintaxe (`Syntax.forPath` do caminho canônico, no `sass_builder`).
    pub syntax: InputSyntax,
}

/// Resolve e carrega folhas, como o `Importer` do dart-sass.
pub trait Importer: Debug {
    /// A URL canônica de `url`, escrita em `@use`/`@forward`/`@import` na
    /// folha de URL canônica `base`, ou `None` se não houver folha. `Err`
    /// é um erro de compilação (por exemplo, ambiguidade entre `_a.scss` e
    /// `a.scss`).
    fn canonicalize(
        &self,
        url: &str,
        base: &str,
        for_import: bool,
    ) -> Result<Option<String>, String>;

    /// Carrega a folha de URL canônica `canonical`.
    fn load(&self, canonical: &str) -> Result<ImporterResult, String>;
}
