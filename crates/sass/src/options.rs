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
    /// When present, every stylesheet lookup goes through it (see
    /// [`crate::importer`]); `fs` and `load_paths` are no longer used.
    pub(crate) importer: Option<&'a dyn Importer>,
    /// The dart-sass version being imitated (see [`VersaoDartSass`]).
    pub(crate) version: VersaoDartSass,
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
            version: VersaoDartSass::V1_102_0,
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

    /// Sets the [`Importer`] that resolves and loads the stylesheets.
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

    /// Chooses the dart-sass version whose output the compilation imitates
    /// (default: [`VersaoDartSass::V1_102_0`]).
    ///
    /// ```
    /// use dartforge_sass::{Options, VersaoDartSass};
    /// let source = ".a {\n  .b { x: y; }\n  color: red;\n}\n";
    /// let v166 = Options::default().version(VersaoDartSass::V1_66_0);
    /// assert_eq!(
    ///     dartforge_sass::from_string(source, &v166).unwrap(),
    ///     ".a {\n  color: red;\n}\n.a .b {\n  x: y;\n}\n"
    /// );
    /// assert_eq!(
    ///     dartforge_sass::from_string(source, &Options::default()).unwrap(),
    ///     ".a .b {\n  x: y;\n}\n.a {\n  color: red;\n}\n"
    /// );
    /// ```
    #[must_use]
    #[inline]
    pub const fn version(mut self, version: VersaoDartSass) -> Self {
        self.version = version;
        self
    }

    /// Does it imitate dart-sass before 1.77.7 (the 1.66 mode)?
    pub(crate) fn v166(&self) -> bool {
        matches!(self.version, VersaoDartSass::V1_66_0)
    }

    /// Does it imitate dart-sass 1.101.0–1.101.3?
    pub(crate) fn v1101(&self) -> bool {
        matches!(self.version, VersaoDartSass::V1_101)
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
        // dart-sass `Syntax.forPath`: `switch (p.extension(path))`, which is
        // case-sensitive (`a.SASS` is SCSS).
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

/// The dart-sass version whose output the compiler imitates.
///
/// The port follows dart-sass 1.102.0. [`VersaoDartSass::V1_66_0`] turns on
/// the compatibility mode with dart-sass 1.66.0 (the one in the
/// `pubspec.lock` of packages that do not resolve with the new `sass`, such
/// as ngcomponents 3.0.0-dev.1):
///
/// * declarations, comments, `@import` and at-rules without a body that come
///   **after** a nested rule go into the parent selector's block, before the
///   nested rule (without the `_copyParentAfterSibling` that dart-sass
///   1.92.0 introduced — the "mixed-decls" change);
/// * colors with the old model (integer RGB channels, `rgba(r, g, b, a)`,
///   `hsl()` only for what came out of the `hsl()` function), U+007F
///   unescaped, empty custom property and numbers with complex units as an
///   error, `//` preserved in the value of unknown at-rules;
/// * what the mode cannot guarantee (CSS Color 4 color spaces, out-of-range
///   channels, CSS Values 4 calculation functions that 1.66 did not
///   know...) is refused with an error that says so.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum VersaoDartSass {
    /// dart-sass 1.102.0 (the port).
    #[default]
    V1_102_0,
    /// dart-sass 1.66.0 (compatibility mode).
    V1_66_0,
    /// dart-sass 1.101.0 to 1.101.3 (the same CSS output): before 1.101.4, a
    /// legacy color in `compressed` chooses between `rgb` and `hsl` by the
    /// length of the channels alone, `rgb()` writes a non-integer channel as
    /// a number (without `%`), the integer is the approximate one outside
    /// `inspect`, the CSS `if()` takes the value as its `toString` (inspect);
    /// and, before 1.102.0, the rec2020 gamma is the ITU-R BT.2020 one.
    V1_101,
}

impl VersaoDartSass {
    /// The version for the text of a `sass` package version (from
    /// `pubspec.lock`), or `None` when no mode imitates it byte for byte.
    ///
    /// ```
    /// use dartforge_sass::VersaoDartSass;
    /// assert_eq!(VersaoDartSass::do_lock("1.66.0"), Some(VersaoDartSass::V1_66_0));
    /// assert_eq!(VersaoDartSass::do_lock("1.102.0"), Some(VersaoDartSass::V1_102_0));
    /// assert_eq!(VersaoDartSass::do_lock("1.80.0"), None);
    /// ```
    pub fn do_lock(version: &str) -> Option<Self> {
        match version {
            "1.102.0" => Some(Self::V1_102_0),
            "1.66.0" => Some(Self::V1_66_0),
            "1.101.0" | "1.101.1" | "1.101.2" | "1.101.3" => Some(Self::V1_101),
            _ => None,
        }
    }
}

thread_local! {
    /// The version of the compilation running on this thread:
    /// `Value::to_css_string` and `Value::inspect` serialize without the
    /// compilation's [`Options`].
    static CURRENT_VERSION: std::cell::Cell<VersaoDartSass> =
        const { std::cell::Cell::new(VersaoDartSass::V1_102_0) };
}

/// Restores the previous version when it goes out of scope (see
/// [`set_version`]).
pub(crate) struct VersionGuard(VersaoDartSass);

impl Drop for VersionGuard {
    fn drop(&mut self) {
        CURRENT_VERSION.with(|v| v.set(self.0));
    }
}

/// Sets `version` as the version of the compilation running on this thread
/// until the returned guard goes out of scope.
pub(crate) fn set_version(version: VersaoDartSass) -> VersionGuard {
    VersionGuard(CURRENT_VERSION.with(|v| v.replace(version)))
}

/// The version of the compilation running on this thread.
pub(crate) fn current_version() -> VersaoDartSass {
    CURRENT_VERSION.with(std::cell::Cell::get)
}
