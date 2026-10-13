//! Importers: what turns the URL of a `@use`/`@forward`/`@import` into the
//! canonical URL of a stylesheet and loads it — dart-sass's `Importer`
//! (`lib/src/importer.dart`).
//!
//! The original grass only searched the disk from the path of the current
//! file and from the `load_paths`. With an [`Importer`] in the
//! [`crate::Options`], the whole lookup goes through it, as in dart-sass's
//! `ImportCache.canonicalize`: the canonical URLs are strings
//! (`package:pkg/a.scss`, `asset:pkg/web/b.scss`), which the visitor stores
//! as `PathBuf` where grass stored paths.
use std::fmt::Debug;

use crate::InputSyntax;

/// A stylesheet loaded by an [`Importer`].
#[derive(Debug, Clone)]
pub struct ImporterResult {
    /// The text of the stylesheet.
    pub contents: String,
    /// The syntax (`Syntax.forPath` of the canonical path, in
    /// `sass_builder`).
    pub syntax: InputSyntax,
}

/// Resolves and loads stylesheets, like dart-sass's `Importer`.
pub trait Importer: Debug {
    /// The canonical URL of `url`, written in `@use`/`@forward`/`@import` in
    /// the stylesheet with canonical URL `base`, or `None` if there is no
    /// stylesheet. `Err` is a compilation error (for example, ambiguity
    /// between `_a.scss` and `a.scss`).
    fn canonicalize(
        &self,
        url: &str,
        base: &str,
        for_import: bool,
    ) -> Result<Option<String>, String>;

    /// Loads the stylesheet with canonical URL `canonical`.
    fn load(&self, canonical: &str) -> Result<ImporterResult, String>;
}
