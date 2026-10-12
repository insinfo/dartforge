//! Diagnostics shared by the compiler, the analyzer and the language server.
//!
//! A [`Diagnostic`] may carry a [`Codigo`] from the official `package:analyzer`,
//! with severity and arguments. The message of a diagnostic with a code is
//! **rendered from the official English template** ([`InfoCodigo::mensagem`]), with
//! the same rule as the analyzer's `formatList` (`src/generated/java_core.dart`):
//! each `{n}` becomes argument `n`. The template table ([`codigos`]) is generated
//! from `analyzer-6.11.0` (the analyzer of SDK 3.6) by
//! `cargo run -p dartforge-parity --example gerar_codigos`, and is not edited
//! by hand.

mod codigos_g;

/// Analyzer codes grouped by their originating class (`codigos::compile_time_error::X`,
/// `codigos::warning::X`, `codigos::parser::X`...), generated.
pub mod codigos {
    pub use crate::codigos_g::modulos::*;
}

/// Half-open range of UTF-8 bytes in the source file.
///
/// Editor adapters must convert these offsets to the protocol's encoding,
/// for example UTF-16 code units in LSP.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Span {
    /// First byte included.
    pub start: usize,
    /// First byte after the range.
    pub end: usize,
}

/// Severity of a diagnostic, in the terms of the analyzer's `ErrorSeverity`.
///
/// The variant order is the reporting order of `dart analyze`: errors first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Severidade {
    /// `ERROR`: compile-time or syntax error.
    #[default]
    Error,
    /// `WARNING`: warning (former "hint" promoted, `StaticWarningCode`, `WarningCode`).
    Warning,
    /// `INFO`: hints, lints and TODOs.
    Info,
}

impl Severidade {
    /// The name in `ErrorSeverity.name` (`"ERROR"`, `"WARNING"`, `"INFO"`).
    pub const fn nome(self) -> &'static str {
        match self {
            Severidade::Error => "ERROR",
            Severidade::Warning => "WARNING",
            Severidade::Info => "INFO",
        }
    }

    /// Parses a name produced by [`Severidade::nome`].
    pub fn por_nome(name: &str) -> Option<Self> {
        match name {
            "ERROR" => Some(Severidade::Error),
            "WARNING" => Some(Severidade::Warning),
            "INFO" => Some(Severidade::Info),
            _ => None,
        }
    }
}

/// Type of a code, in the terms of the analyzer's `ErrorType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TipoErro {
    /// `TODO`.
    Todo,
    /// `HINT`.
    Hint,
    /// `COMPILE_TIME_ERROR`.
    CompileTimeError,
    /// `CHECKED_MODE_COMPILE_TIME_ERROR`.
    CheckedModeCompileTimeError,
    /// `STATIC_WARNING` (also the type of the `WarningCode`s).
    StaticWarning,
    /// `SYNTACTIC_ERROR` (parser and scanner).
    SyntacticError,
    /// `LINT`.
    Lint,
}

impl TipoErro {
    /// The name in `ErrorType.name`.
    pub const fn nome(self) -> &'static str {
        match self {
            TipoErro::Todo => "TODO",
            TipoErro::Hint => "HINT",
            TipoErro::CompileTimeError => "COMPILE_TIME_ERROR",
            TipoErro::CheckedModeCompileTimeError => "CHECKED_MODE_COMPILE_TIME_ERROR",
            TipoErro::StaticWarning => "STATIC_WARNING",
            TipoErro::SyntacticError => "SYNTACTIC_ERROR",
            TipoErro::Lint => "LINT",
        }
    }

    /// Parses a name produced by [`TipoErro::nome`].
    pub fn por_nome(name: &str) -> Option<Self> {
        [
            TipoErro::Todo,
            TipoErro::Hint,
            TipoErro::CompileTimeError,
            TipoErro::CheckedModeCompileTimeError,
            TipoErro::StaticWarning,
            TipoErro::SyntacticError,
            TipoErro::Lint,
        ]
        .into_iter()
        .find(|t| t.nome() == name)
    }

    /// The default severity of the type (`ErrorType.severity`).
    pub const fn severidade(self) -> Severidade {
        match self {
            TipoErro::Todo | TipoErro::Hint | TipoErro::Lint => Severidade::Info,
            TipoErro::StaticWarning => Severidade::Warning,
            TipoErro::CompileTimeError | TipoErro::CheckedModeCompileTimeError | TipoErro::SyntacticError => {
                Severidade::Error
            }
        }
    }

    /// Compile-time and syntax errors: `compile-js` rejects the program.
    pub const fn e_erro_de_compilacao(self) -> bool {
        matches!(
            self,
            TipoErro::CompileTimeError | TipoErro::CheckedModeCompileTimeError | TipoErro::SyntacticError
        )
    }
}

/// An entry of the generated table: one analyzer `ErrorCode`.
#[derive(Debug, PartialEq, Eq)]
pub struct InfoCodigo {
    /// The code as `dart analyze` reports it: `ErrorCode.name` in lower case.
    /// Several codes may share the name (with a different `uniqueName`).
    pub nome: &'static str,
    /// `ErrorCode.uniqueName`, for example `CompileTimeErrorCode.RETURN_OF_INVALID_TYPE_FROM_FUNCTION`.
    pub unico: &'static str,
    /// Message template, with `{0}`, `{1}`...
    pub mensagem: &'static str,
    /// Correction template, if any.
    pub correcao: Option<&'static str>,
    /// `ErrorCode.type`.
    pub tipo: TipoErro,
    /// `ErrorCode.errorSeverity`.
    pub severidade: Severidade,
    /// `hasPublishedDocs`: the `dart analyze` JSON carries the `documentation` field.
    pub documentado: bool,
}

impl InfoCodigo {
    /// `ErrorCode.url`: the address of the published documentation.
    pub fn url(&self) -> Option<String> {
        self.documentado.then(|| format!("https://dart.dev/diagnostics/{}", self.nome))
    }
}

/// An analyzer code: index into the generated table (2 bytes per diagnostic).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Codigo(u16);

impl Codigo {
    /// The table entry.
    pub fn info(self) -> &'static InfoCodigo {
        &codigos_g::TABELA[self.0 as usize]
    }

    /// All codes of the table, in generated order.
    pub fn todos() -> impl Iterator<Item = Codigo> {
        (0..codigos_g::TABELA.len() as u16).map(Codigo)
    }

    /// Looks up by `uniqueName` (`"CompileTimeErrorCode.UNDEFINED_FUNCTION"`).
    pub fn por_unico(unique: &str) -> Option<Codigo> {
        codigos_g::POR_UNICO
            .binary_search_by(|(u, _)| (*u).cmp(unique))
            .ok()
            .map(|i| Codigo(codigos_g::POR_UNICO[i].1))
    }

    /// Looks up by the reported code (`"undefined_function"`). Among codes that
    /// share the name, returns the one whose `uniqueName` is the name itself.
    pub fn por_nome(name: &str) -> Option<Codigo> {
        let mut found = None;
        for c in Codigo::todos() {
            let info = c.info();
            if info.nome == name {
                let own = info.unico.rsplit('.').next().is_some_and(|u| u.eq_ignore_ascii_case(name));
                if own {
                    return Some(c);
                }
                found.get_or_insert(c);
            }
        }
        found
    }
}

/// Which analyzer is the reference for the code names and texts of a file
/// (docs/ANALYZER-ESPECIFICACAO.md, T2): 3.6.2, or 3.13.4 for files whose
/// language version is above 3.6 and for those using syntax that 3.6.2
/// does not know.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, serde::Serialize, serde::Deserialize)]
pub enum Referencia {
    /// Analyzer 6.11.0, from SDK 3.6.2.
    #[default]
    V3_6,
    /// The analyzer of SDK 3.13.4.
    V3_13,
}

/// How a code of the 6.11 catalog is reported when the reference is 3.13.4.
#[derive(Debug, PartialEq, Eq)]
pub struct Variante313 {
    /// The 6.11 catalog entry.
    pub de: Codigo,
    /// The entry 3.13.4 reports with (may be the same one); `None` when
    /// 3.13.4 does not report it.
    pub para: Option<Codigo>,
    /// The indices, into the diagnostic's arguments, of the target template's
    /// arguments, in the order of its `{n}`.
    pub args: &'static [u8],
    /// How many arguments the diagnostic must carry for this row to apply.
    pub exige: u8,
}

impl Codigo {
    /// The rows of the variant table for this code, in the order they are
    /// tried.
    pub fn variantes_3_13(self) -> &'static [Variante313] {
        let t = &codigos_g::VARIANTES_3_13[..];
        let start = t.partition_point(|v| v.de < self);
        let end = t.partition_point(|v| v.de <= self);
        &t[start..end]
    }

    /// The variant that applies to a diagnostic with `arg_count` arguments:
    /// the first one that fits. An emitter that does not pass the arguments
    /// only 3.13 uses gets no variant, and the diagnostic is reported as in 3.6.
    pub fn variante_3_13(self, arg_count: usize) -> Option<&'static Variante313> {
        self.variantes_3_13().iter().find(|v| arg_count >= v.exige as usize)
    }
}

impl serde::Serialize for Codigo {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.info().unico)
    }
}

impl<'de> serde::Deserialize<'de> for Codigo {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Codigo::por_unico(&s).ok_or_else(|| serde::de::Error::custom(format!("unknown code: {s}")))
    }
}

impl serde::Serialize for Severidade {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.nome())
    }
}

impl<'de> serde::Deserialize<'de> for Severidade {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Severidade::por_nome(&s).ok_or_else(|| serde::de::Error::custom(format!("unknown severity: {s}")))
    }
}

/// Renders a template like the analyzer's `formatList`: each `{n}` becomes
/// `args[n]`. Without arguments the template is returned untouched (the
/// analyzer does not substitute anything either). An index without an
/// argument is left as is.
pub fn formatar(molde: &str, args: &[Box<str>]) -> String {
    if args.is_empty() {
        return molde.to_string();
    }
    let mut out = String::with_capacity(molde.len() + 16);
    let mut rest = molde;
    while let Some(i) = rest.find('{') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let digits = after.bytes().take_while(u8::is_ascii_digit).count();
        if digits > 0 && after.as_bytes().get(digits) == Some(&b'}') {
            let n: usize = after[..digits].parse().unwrap_or(usize::MAX);
            match args.get(n) {
                Some(a) => out.push_str(a),
                None => out.push_str(&rest[i..i + digits + 2]),
            }
            rest = &after[digits + 1..];
        } else {
            out.push('{');
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

/// A context message of a diagnostic (the analyzer's `DiagnosticMessage`:
/// "The first definition of this name", the declaration of a type with the
/// same name…), which the output shows below it
/// (docs/ANALYZER-ESPECIFICACAO-INFRA.md §3.3, §5.4, §5.5).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Contexto {
    /// The file of the message, when it is not the diagnostic's own
    /// (absolute path).
    #[serde(default)]
    pub arquivo: Option<Box<str>>,
    pub span: Span,
    pub mensagem: Box<str>,
}

/// An error together with its location in the source code.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Diagnostic {
    /// Human-readable explanation of the problem. With [`Diagnostic::code`],
    /// it is the official template rendered with [`Diagnostic::args`].
    pub message: String,
    /// Region of the code associated with the problem.
    pub span: Span,
    /// Analyzer code, when the diagnostic has parity with one.
    #[serde(default)]
    pub code: Option<Codigo>,
    /// Effective severity (the code's, unless reconfigured by
    /// `analysis_options.yaml`). Without a code, `Error`.
    #[serde(default)]
    pub severity: Severidade,
    /// Template arguments, in the order of the `{n}`.
    #[serde(default)]
    pub args: Box<[Box<str>]>,
    /// The context messages (`contextMessages`); almost always empty.
    #[serde(default)]
    pub contexto: Vec<Contexto>,
}

impl Diagnostic {
    /// Creates a diagnostic without a code, keeping the given message and range.
    ///
    /// # Examples
    ///
    /// ```
    /// use dartforge_diagnostics::{Diagnostic, Span};
    /// let error = Diagnostic::new("Name not found", Span { start: 2, end: 5 });
    /// assert_eq!(error.span.start, 2);
    /// assert!(error.code.is_none());
    /// ```
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
            code: None,
            severity: Severidade::Error,
            args: Box::default(),
            contexto: Vec::new(),
        }
    }

    /// Adds a context message in the same file as the diagnostic.
    pub fn com_contexto(mut self, span: Span, message: impl Into<Box<str>>) -> Self {
        self.contexto.push(Contexto { arquivo: None, span, mensagem: message.into() });
        self
    }

    /// Creates a diagnostic with a code: the message comes from the official template.
    ///
    /// ```
    /// use dartforge_diagnostics::{codigos, Diagnostic, Span};
    /// let d = Diagnostic::com_codigo(codigos::compile_time_error::UNDEFINED_FUNCTION, Span { start: 0, end: 1 }, ["h"]);
    /// assert_eq!(d.message, "The function 'h' isn't defined.");
    /// assert_eq!(d.code.unwrap().info().nome, "undefined_function");
    /// ```
    pub fn com_codigo<I, A>(code: Codigo, span: Span, args: I) -> Self
    where
        I: IntoIterator<Item = A>,
        A: Into<Box<str>>,
    {
        let args: Box<[Box<str>]> = args.into_iter().map(Into::into).collect();
        let info = code.info();
        Self {
            message: formatar(info.mensagem, &args),
            span,
            code: Some(code),
            severity: info.severidade,
            args,
            contexto: Vec::new(),
        }
    }

    /// The diagnostic as the analyzer of reference `r` reports it: for 3.6,
    /// itself; for 3.13.4, with the code, text and arguments of the variant
    /// ([`Codigo::variante_3_13`]), or `None` when 3.13.4 does not report it.
    /// The severity follows the target code's, unless it was already
    /// reconfigured.
    pub fn na_referencia(mut self, r: Referencia) -> Option<Diagnostic> {
        if r == Referencia::V3_6 {
            return Some(self);
        }
        let Some(from) = self.code else { return Some(self) };
        let Some(v) = from.variante_3_13(self.args.len()) else { return Some(self) };
        let to = v.para?;
        let args: Box<[Box<str>]> = v.args.iter().map(|&i| self.args[i as usize].clone()).collect();
        let info = to.info();
        if self.severity == from.info().severidade {
            self.severity = info.severidade;
        }
        self.message = formatar(info.mensagem, &args);
        self.code = Some(to);
        self.args = args;
        Some(self)
    }

    /// The correction rendered from the official template, if the code has one.
    pub fn correcao(&self) -> Option<String> {
        let template = self.code?.info().correcao?;
        Some(formatar(template, &self.args))
    }
}

impl std::fmt::Display for Diagnostic {
    /// Formats the message with its byte range.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(c) = self.code {
            write!(f, "{}: ", c.info().nome)?;
        }
        write!(f, "{} at bytes {}..{}", self.message, self.span.start, self.span.end)
    }
}
impl std::error::Error for Diagnostic {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_like_format_list() {
        let a: Vec<Box<str>> = vec!["String".into(), "int".into(), "".into()];
        assert_eq!(
            formatar("The argument type '{0}' can't be assigned to the parameter type '{1}'. {2}", &a),
            "The argument type 'String' can't be assigned to the parameter type 'int'. "
        );
        assert_eq!(formatar("no {0} args", &[]), "no {0} args");
        assert_eq!(formatar("{x} {9}", &a), "{x} {9}");
    }

    /// The analyzer 6.11 table plus the generator's 3.13.4 supplement
    /// (`SUPLEMENTO_3_13`): 4 `CompileTimeErrorCode` and 6 `ParserErrorCode`
    /// for primary constructors (3 of them for `const` and for the header in
    /// mixins and extensions), 4 `CompileTimeErrorCode` for dot shorthands
    /// (including the one for type arguments on the constructor), the 10 codes
    /// only 3.13.4 has (5, 3 and 2 `WarningCode`) and the 14 new shapes of
    /// 6.11 codes (12, 1 and 1).
    #[test]
    fn table_has_the_counts_of_analyzer_6_11_and_the_supplement() {
        let count = |prefix: &str| Codigo::todos().filter(|c| c.info().unico.starts_with(prefix)).count();
        assert_eq!(count("CompileTimeErrorCode."), 542 + 4 + 4 + 5 + 12);
        assert_eq!(count("StaticWarningCode."), 7);
        assert_eq!(count("WarningCode."), 144 + 2 + 1);
        assert_eq!(count("ParserErrorCode."), 265 + 6 + 3 + 1);
        for c in Codigo::todos() {
            assert_eq!(Codigo::por_unico(c.info().unico), Some(c));
        }
    }

    /// The variant table is sorted, and each row fits the templates.
    #[test]
    fn variants_3_13_sorted_and_consistent() {
        let t = &codigos_g::VARIANTES_3_13;
        assert!(t.windows(2).all(|w| w[0].de <= w[1].de));
        for v in t.iter() {
            assert!(v.args.iter().all(|&i| i < v.exige));
            assert_eq!(v.de.variantes_3_13().first().map(|p| p.de), Some(v.de));
        }
        assert!(codigos::compile_time_error::UNDEFINED_FUNCTION.variantes_3_13().is_empty());
    }

    /// Cases c02, c03, c04, c05, c13 and c16 of `corpus/especificacao/t2/v`.
    #[test]
    fn na_referencia_switches_name_text_and_arguments() {
        use codigos::{compile_time_error as c, parser as p, warning as w};
        let s = Span { start: 0, end: 1 };
        let same = Diagnostic::com_codigo(c::ENUM_WITHOUT_CONSTANTS, s, Vec::<String>::new());
        assert_eq!(same.clone().na_referencia(Referencia::V3_6), Some(same.clone()));
        let new = same.na_referencia(Referencia::V3_13).unwrap();
        assert_eq!(new.message, "The enum must have at least one enum constant.");
        assert_eq!(new.correcao().as_deref(), Some("Try declaring an enum constant."));
        assert_eq!(new.code.unwrap().info().nome, "enum_without_constants");

        let field = Diagnostic::com_codigo(p::REPRESENTATION_FIELD_MODIFIER, s, ["var"]);
        assert_eq!(field.message, "Representation fields can't have modifiers.");
        assert_eq!(field.na_referencia(Referencia::V3_13).unwrap().message, "Representation fields can't have the modifier 'var'.");

        let param = Diagnostic::com_codigo(w::UNUSED_ELEMENT_PARAMETER, s, ["p"]);
        assert_eq!(param.code.unwrap().info().nome, "unused_element");
        let param = param.na_referencia(Referencia::V3_13).unwrap();
        assert_eq!(param.code.unwrap().info().nome, "unused_element_parameter");
        assert_eq!(param.message, "A value for optional parameter 'p' isn't ever given.");
        assert_eq!(param.severity, Severidade::Warning);

        let superclass = Diagnostic::com_codigo(c::CONST_CONSTRUCTOR_WITH_NON_CONST_SUPER, s, ["B", "A"]);
        assert!(superclass.message.ends_with("of 'B'."));
        assert!(superclass.na_referencia(Referencia::V3_13).unwrap().message.ends_with("of 'A'."));
        // Without the argument only 3.13 uses, the diagnostic does not change.
        let old = Diagnostic::com_codigo(c::CONST_CONSTRUCTOR_WITH_NON_CONST_SUPER, s, ["B"]);
        assert_eq!(old.clone().na_referencia(Referencia::V3_13), Some(old));

        let two = Diagnostic::com_codigo(
            c::AMBIGUOUS_EXTENSION_MEMBER_ACCESS,
            s,
            ["m", "extension 'E1' and extension 'E2'", "extension E1 on int", "extension E2 on int"],
        );
        assert_eq!(
            two.na_referencia(Referencia::V3_13).unwrap().message,
            "A member named 'm' is defined in 'extension E1 on int' and 'extension E2 on int', and neither is more specific."
        );
        // Three or more: the 3.13.4 text is the 3.6.2 one (case c25).
        let three = Diagnostic::com_codigo(c::AMBIGUOUS_EXTENSION_MEMBER_ACCESS, s, ["m", "extension 'E1', extension 'E2', and extension 'E3'"]);
        assert_eq!(three.clone().na_referencia(Referencia::V3_13), Some(three));

        let private = Diagnostic::com_codigo(c::NON_EXHAUSTIVE_SWITCH_EXPRESSION, s, ["P", "P._b", "P._b", ""]);
        let private = private.na_referencia(Referencia::V3_13).unwrap();
        assert_eq!(private.message, "The enum 'P' isn't exhaustively matched by the switch cases because some of the enum constants are private.");
        assert_eq!(private.correcao().as_deref(), Some("Try adding a wildcard pattern."));

        let outside = Diagnostic::com_codigo(c::FIELD_INITIALIZER_OUTSIDE_CONSTRUCTOR, s, Vec::<String>::new());
        assert_eq!(outside.na_referencia(Referencia::V3_13), None);
        let from_parser = Diagnostic::com_codigo(p::FIELD_INITIALIZER_OUTSIDE_CONSTRUCTOR, s, Vec::<String>::new());
        assert!(from_parser.na_referencia(Referencia::V3_13).is_some());
    }

    #[test]
    fn por_nome_prefers_own_name() {
        let c = Codigo::por_nome("abstract_field_initializer").unwrap();
        assert_eq!(c.info().unico, "CompileTimeErrorCode.ABSTRACT_FIELD_INITIALIZER");
        assert_eq!(c.info().severidade, Severidade::Error);
        let w = Codigo::por_nome("unused_local_variable").unwrap();
        assert_eq!(w.info().tipo, TipoErro::StaticWarning);
        assert_eq!(w.info().severidade, Severidade::Warning);
    }

    #[test]
    fn codigo_serde_by_unique_name() {
        let d = Diagnostic::com_codigo(codigos::compile_time_error::NON_BOOL_CONDITION, Span { start: 1, end: 2 }, Vec::<String>::new());
        let j = serde_json::to_string(&d).unwrap();
        assert!(j.contains("CompileTimeErrorCode.NON_BOOL_CONDITION"), "{j}");
        let back: Diagnostic = serde_json::from_str(&j).unwrap();
        assert_eq!(back, d);
        let old: Diagnostic = serde_json::from_str(r#"{"message":"x","span":{"start":0,"end":1}}"#).unwrap();
        assert_eq!(old, Diagnostic::new("x", Span { start: 0, end: 1 }));
    }
}
