use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use crate::{builtin::Builtin, importer::Importer, Fs, Logger, StdFs, StdLogger};

/// Configuration for Sass compilation
///
/// The simplest usage is `grass::Options::default()`; however, a builder pattern
/// is also exposed to offer more control.
#[derive(Debug)]
pub struct Options<'a> {
    pub(crate) fs: &'a dyn Fs,
    pub(crate) logger: &'a dyn Logger,
    pub(crate) style: OutputStyle,
    pub(crate) load_paths: Vec<PathBuf>,
    pub(crate) allows_charset: bool,
    pub(crate) unicode_error_messages: bool,
    pub(crate) quiet: bool,
    pub(crate) input_syntax: Option<InputSyntax>,
    pub(crate) custom_fns: HashMap<String, Builtin>,
    /// Quando presente, toda busca de folha passa por ele (ver
    /// [`crate::importer`]); `fs` e `load_paths` deixam de ser usados.
    pub(crate) importer: Option<&'a dyn Importer>,
    /// A versão do dart-sass imitada (ver [`VersaoDartSass`]).
    pub(crate) versao: VersaoDartSass,
}

impl Default for Options<'_> {
    #[inline]
    fn default() -> Self {
        Self {
            fs: &StdFs,
            logger: &StdLogger,
            style: OutputStyle::Expanded,
            load_paths: Vec::new(),
            allows_charset: true,
            unicode_error_messages: true,
            quiet: false,
            input_syntax: None,
            custom_fns: HashMap::new(),
            importer: None,
            versao: VersaoDartSass::V1_102_0,
        }
    }
}

impl<'a> Options<'a> {
    /// This option allows you to control the file system that Sass will see.
    ///
    /// By default, it uses [`StdFs`], which is backed by [`std::fs`],
    /// allowing direct, unfettered access to the local file system.
    #[must_use]
    #[inline]
    pub fn fs(mut self, fs: &'a dyn Fs) -> Self {
        self.fs = fs;
        self
    }

    /// Define o [`Importer`] que resolve e carrega as folhas.
    #[must_use]
    #[inline]
    pub fn importer(mut self, importer: &'a dyn Importer) -> Self {
        self.importer = Some(importer);
        self
    }

    /// This option allows you to define how log events should be handled
    #[must_use]
    #[inline]
    pub fn logger(mut self, logger: &'a dyn Logger) -> Self {
        self.logger = logger;
        self
    }

    /// `grass` currently offers 2 different output styles
    ///
    ///  - [`OutputStyle::Expanded`] writes each selector and declaration on its own line.
    ///  - [`OutputStyle::Compressed`] removes as many extra characters as possible
    ///    and writes the entire stylesheet on a single line.
    ///
    /// By default, output is expanded.
    #[must_use]
    #[inline]
    pub const fn style(mut self, style: OutputStyle) -> Self {
        self.style = style;
        self
    }

    /// This flag tells Sass not to emit any warnings when compiling. By default,
    /// Sass emits warnings when deprecated features are used or when the `@warn`
    /// rule is encountered. It also silences the `@debug` rule.
    ///
    /// Setting this option to `true` will stop all logs from reaching the [`crate::Logger`].
    ///
    /// By default, this value is `false` and warnings are emitted.
    #[must_use]
    #[inline]
    pub const fn quiet(mut self, quiet: bool) -> Self {
        self.quiet = quiet;
        self
    }

    /// All Sass implementations allow users to provide load paths: paths on the
    /// filesystem that Sass will look in when locating modules. For example, if
    /// you pass `node_modules/susy/sass` as a load path, you can use
    /// `@import "susy"` to load `node_modules/susy/sass/susy.scss`.
    ///
    /// Imports will always be resolved relative to the current file first, though.
    /// Load paths will only be used if no relative file exists that matches the
    /// module's URL. This ensures that you can't accidentally mess up your relative
    /// imports when you add a new library.
    ///
    /// This method will append a single path to the list.
    #[must_use]
    #[inline]
    pub fn load_path<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.load_paths.push(path.as_ref().to_owned());
        self
    }

    /// Append multiple loads paths
    ///
    /// Note that this method does *not* remove existing load paths
    ///
    /// See [`Options::load_path`](Options::load_path) for more information about
    /// load paths
    #[must_use]
    #[inline]
    pub fn load_paths<P: AsRef<Path>>(mut self, paths: &[P]) -> Self {
        for path in paths {
            self.load_paths.push(path.as_ref().to_owned());
        }

        self
    }

    /// This flag tells Sass whether to emit a `@charset`
    /// declaration or a UTF-8 byte-order mark.
    ///
    /// By default, Sass will insert either a `@charset`
    /// declaration (in expanded output mode) or a byte-order
    /// mark (in compressed output mode) if the stylesheet
    /// contains any non-ASCII characters.
    #[must_use]
    #[inline]
    pub const fn allows_charset(mut self, allows_charset: bool) -> Self {
        self.allows_charset = allows_charset;
        self
    }

    /// This flag tells Sass only to emit ASCII characters as
    /// part of error messages.
    ///
    /// By default Sass will emit non-ASCII characters for
    /// these messages.
    ///
    /// This flag does not affect the CSS output.
    #[must_use]
    #[inline]
    pub const fn unicode_error_messages(mut self, unicode_error_messages: bool) -> Self {
        self.unicode_error_messages = unicode_error_messages;
        self
    }

    /// This option forces Sass to parse input using the given syntax.
    ///
    /// By default, Sass will attempt to read the file extension to determine
    /// the syntax. If this is not possible, it will default to [`InputSyntax::Scss`].
    ///
    /// This flag only affects the first file loaded. Files that are loaded using
    /// `@import`, `@use`, or `@forward` will always have their syntax inferred.
    #[must_use]
    #[inline]
    pub const fn input_syntax(mut self, syntax: InputSyntax) -> Self {
        self.input_syntax = Some(syntax);
        self
    }

    /// Add a custom function accessible from within Sass
    ///
    /// See the [`Builtin`] documentation for additional information
    #[must_use]
    #[inline]
    #[cfg(any(feature = "custom-builtin-fns", doc))]

    pub fn add_custom_fn<S: Into<String>>(mut self, name: S, func: Builtin) -> Self {
        self.custom_fns.insert(name.into(), func);
        self
    }

    /// Escolhe a versão do dart-sass cuja saída a compilação imita (padrão:
    /// [`VersaoDartSass::V1_102_0`]).
    ///
    /// ```
    /// use dartforge_sass::{Options, VersaoDartSass};
    /// let fonte = ".a {\n  .b { x: y; }\n  color: red;\n}\n";
    /// let v166 = Options::default().versao(VersaoDartSass::V1_66_0);
    /// assert_eq!(
    ///     dartforge_sass::from_string(fonte, &v166).unwrap(),
    ///     ".a {\n  color: red;\n}\n.a .b {\n  x: y;\n}\n"
    /// );
    /// assert_eq!(
    ///     dartforge_sass::from_string(fonte, &Options::default()).unwrap(),
    ///     ".a .b {\n  x: y;\n}\n.a {\n  color: red;\n}\n"
    /// );
    /// ```
    #[must_use]
    #[inline]
    pub const fn versao(mut self, versao: VersaoDartSass) -> Self {
        self.versao = versao;
        self
    }

    /// Imita o dart-sass anterior ao 1.77.7 (o modo 1.66)?
    pub(crate) fn v166(&self) -> bool {
        matches!(self.versao, VersaoDartSass::V1_66_0)
    }

    pub(crate) fn is_compressed(&self) -> bool {
        matches!(self.style, OutputStyle::Compressed)
    }
}

/// Useful when parsing Sass from sources other than the file system
///
/// See [`Options::input_syntax`] for additional information
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InputSyntax {
    /// The CSS-superset SCSS syntax.
    Scss,

    /// The whitespace-sensitive indented syntax.
    Sass,

    /// The plain CSS syntax, which disallows special Sass features.
    Css,
}

impl InputSyntax {
    pub(crate) fn for_path(path: &Path) -> Self {
        // dart-sass `Syntax.forPath`: `switch (p.extension(path))`, que
        // distingue maiúsculas (`a.SASS` é SCSS).
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("css") => Self::Css,
            Some("sass") => Self::Sass,
            _ => Self::Scss,
        }
    }
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OutputStyle {
    /// This mode writes each selector and declaration on its own line.
    ///
    /// This is the default output.
    Expanded,

    /// Ideal for release builds, this mode removes as many extra characters as
    /// possible and writes the entire stylesheet on a single line.
    Compressed,
}

/// A versão do dart-sass cuja saída o compilador imita.
///
/// O port segue o dart-sass 1.102.0. [`VersaoDartSass::V1_66_0`] liga o modo
/// de compatibilidade com o dart-sass 1.66.0 (o do `pubspec.lock` de pacotes
/// que não resolvem com o `sass` novo, como o ngcomponents 3.0.0-dev.1):
///
/// * declarações, comentários, `@import` e at-rules sem corpo que vêm
///   **depois** de uma regra aninhada vão para o bloco do seletor pai, antes
///   da regra aninhada (sem o `_copyParentAfterSibling` que o dart-sass
///   1.92.0 introduziu — a mudança "mixed-decls");
/// * cores com o modelo antigo (canais RGB inteiros, `rgba(r, g, b, a)`,
///   `hsl()` só para o que saiu da função `hsl()`), U+007F sem escape,
///   propriedade customizada vazia e números com unidades complexas como
///   erro, `//` preservado no valor de at-rules desconhecidas;
/// * o que o modo não consegue garantir (espaços de cor do CSS Color 4,
///   canais fora do intervalo, funções de cálculo do CSS Values 4 que o
///   1.66 não conhecia...) é recusado com erro que o diz.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum VersaoDartSass {
    /// dart-sass 1.102.0 (o port).
    #[default]
    V1_102_0,
    /// dart-sass 1.66.0 (modo de compatibilidade).
    V1_66_0,
}

impl VersaoDartSass {
    /// A versão para o texto de uma versão do pacote `sass` (do
    /// `pubspec.lock`), ou `None` quando não há modo que a imite byte a byte.
    ///
    /// ```
    /// use dartforge_sass::VersaoDartSass;
    /// assert_eq!(VersaoDartSass::do_lock("1.66.0"), Some(VersaoDartSass::V1_66_0));
    /// assert_eq!(VersaoDartSass::do_lock("1.102.0"), Some(VersaoDartSass::V1_102_0));
    /// assert_eq!(VersaoDartSass::do_lock("1.80.0"), None);
    /// ```
    pub fn do_lock(versao: &str) -> Option<Self> {
        match versao {
            "1.102.0" => Some(Self::V1_102_0),
            "1.66.0" => Some(Self::V1_66_0),
            _ => None,
        }
    }
}

thread_local! {
    /// A versão da compilação em curso nesta thread: `Value::to_css_string`
    /// e `Value::inspect` serializam sem as [`Options`] da compilação.
    static VERSAO_CORRENTE: std::cell::Cell<VersaoDartSass> =
        const { std::cell::Cell::new(VersaoDartSass::V1_102_0) };
}

/// Restaura a versão anterior ao sair de escopo (ver [`fixar_versao`]).
pub(crate) struct GuardaVersao(VersaoDartSass);

impl Drop for GuardaVersao {
    fn drop(&mut self) {
        VERSAO_CORRENTE.with(|v| v.set(self.0));
    }
}

/// Fixa `versao` como a da compilação em curso nesta thread até a guarda
/// devolvida sair de escopo.
pub(crate) fn fixar_versao(versao: VersaoDartSass) -> GuardaVersao {
    GuardaVersao(VERSAO_CORRENTE.with(|v| v.replace(versao)))
}

/// A versão da compilação em curso nesta thread.
pub(crate) fn versao_corrente() -> VersaoDartSass {
    VERSAO_CORRENTE.with(std::cell::Cell::get)
}
