//! Diagnostics shared by the compiler, the analyzer and the language server.
//!
//! A [`Diagnostic`] may carry a [`Code`] from the official `package:analyzer`,
//! with severity and arguments. The message of a diagnostic with a code is
//! **rendered from the official English template** ([`CodeInfo::message`]), with
//! the same rule as the analyzer's `formatList` (`src/generated/java_core.dart`):
//! each `{n}` becomes argument `n`. The template table ([`codes`]) is generated
//! from `analyzer-6.11.0` (the analyzer of SDK 3.6) by
//! `cargo run -p dartforge-parity --example generate_codes`, and is not edited
//! by hand.

mod codes_g;

/// Analyzer codes grouped by their originating class (`codes::compile_time_error::X`,
/// `codes::warning::X`, `codes::parser::X`...), generated.
pub mod codes {
    pub use crate::codes_g::modules::*;
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
pub enum Severity {
    /// `ERROR`: compile-time or syntax error.
    #[default]
    Error,
    /// `WARNING`: warning (former "hint" promoted, `StaticWarningCode`, `WarningCode`).
    Warning,
    /// `INFO`: hints, lints and TODOs.
    Info,
}

impl Severity {
    /// The name in `ErrorSeverity.name` (`"ERROR"`, `"WARNING"`, `"INFO"`).
    pub const fn name(self) -> &'static str {
        match self {
            Severity::Error => "ERROR",
            Severity::Warning => "WARNING",
            Severity::Info => "INFO",
        }
    }

    /// Parses a name produced by [`Severity::name`].
    pub fn by_name(name: &str) -> Option<Self> {
        match name {
            "ERROR" => Some(Severity::Error),
            "WARNING" => Some(Severity::Warning),
            "INFO" => Some(Severity::Info),
            _ => None,
        }
    }
}

/// Type of a code, in the terms of the analyzer's `ErrorType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ErrorType {
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

impl ErrorType {
    /// The name in `ErrorType.name`.
    pub const fn name(self) -> &'static str {
        match self {
            ErrorType::Todo => "TODO",
            ErrorType::Hint => "HINT",
            ErrorType::CompileTimeError => "COMPILE_TIME_ERROR",
            ErrorType::CheckedModeCompileTimeError => "CHECKED_MODE_COMPILE_TIME_ERROR",
            ErrorType::StaticWarning => "STATIC_WARNING",
            ErrorType::SyntacticError => "SYNTACTIC_ERROR",
            ErrorType::Lint => "LINT",
        }
    }

    /// Parses a name produced by [`ErrorType::name`].
    pub fn by_name(name: &str) -> Option<Self> {
        [
            ErrorType::Todo,
            ErrorType::Hint,
            ErrorType::CompileTimeError,
            ErrorType::CheckedModeCompileTimeError,
            ErrorType::StaticWarning,
            ErrorType::SyntacticError,
            ErrorType::Lint,
        ]
        .into_iter()
        .find(|t| t.name() == name)
    }

    /// The default severity of the type (`ErrorType.severity`).
    pub const fn severity(self) -> Severity {
        match self {
            ErrorType::Todo | ErrorType::Hint | ErrorType::Lint => Severity::Info,
            ErrorType::StaticWarning => Severity::Warning,
            ErrorType::CompileTimeError | ErrorType::CheckedModeCompileTimeError | ErrorType::SyntacticError => {
                Severity::Error
            }
        }
    }

    /// Compile-time and syntax errors: `compile-js` rejects the program.
    pub const fn is_compile_error(self) -> bool {
        matches!(
            self,
            ErrorType::CompileTimeError | ErrorType::CheckedModeCompileTimeError | ErrorType::SyntacticError
        )
    }
}

/// An entry of the generated table: one analyzer `ErrorCode`.
#[derive(Debug, PartialEq, Eq)]
pub struct CodeInfo {
    /// The code as `dart analyze` reports it: `ErrorCode.name` in lower case.
    /// Several codes may share the name (with a different `uniqueName`).
    pub name: &'static str,
    /// `ErrorCode.uniqueName`, for example `CompileTimeErrorCode.RETURN_OF_INVALID_TYPE_FROM_FUNCTION`.
    pub unique: &'static str,
    /// Message template, with `{0}`, `{1}`...
    pub message: &'static str,
    /// Correction template, if any.
    pub correction: Option<&'static str>,
    /// `ErrorCode.type`.
    pub error_type: ErrorType,
    /// `ErrorCode.errorSeverity`.
    pub severity: Severity,
    /// `hasPublishedDocs`: the `dart analyze` JSON carries the `documentation` field.
    pub documented: bool,
}

impl CodeInfo {
    /// `ErrorCode.url`: the address of the published documentation.
    pub fn url(&self) -> Option<String> {
        self.documented.then(|| format!("https://dart.dev/diagnostics/{}", self.name))
    }
}

/// An analyzer code: index into the generated table (2 bytes per diagnostic).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Code(u16);

impl Code {
    /// The table entry.
    pub fn info(self) -> &'static CodeInfo {
        &codes_g::TABLE[self.0 as usize]
    }

    /// All codes of the table, in generated order.
    pub fn all() -> impl Iterator<Item = Code> {
        (0..codes_g::TABLE.len() as u16).map(Code)
    }

    /// Looks up by `uniqueName` (`"CompileTimeErrorCode.UNDEFINED_FUNCTION"`).
    pub fn by_unique(unique: &str) -> Option<Code> {
        codes_g::BY_UNIQUE
            .binary_search_by(|(u, _)| (*u).cmp(unique))
            .ok()
            .map(|i| Code(codes_g::BY_UNIQUE[i].1))
    }

    /// Looks up by the reported code (`"undefined_function"`). Among codes that
    /// share the name, returns the one whose `uniqueName` is the name itself.
    pub fn by_name(name: &str) -> Option<Code> {
        let mut found = None;
        for c in Code::all() {
            let info = c.info();
            if info.name == name {
                let own = info.unique.rsplit('.').next().is_some_and(|u| u.eq_ignore_ascii_case(name));
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
pub enum AnalyzerReference {
    /// Analyzer 6.11.0, from SDK 3.6.2.
    #[default]
    V3_6,
    /// The analyzer of SDK 3.13.4.
    V3_13,
}

/// How a code of the 6.11 catalog is reported when the reference is 3.13.4.
#[derive(Debug, PartialEq, Eq)]
pub struct Variant313 {
    /// The 6.11 catalog entry.
    pub from: Code,
    /// The entry 3.13.4 reports with (may be the same one); `None` when
    /// 3.13.4 does not report it.
    pub to: Option<Code>,
    /// The indices, into the diagnostic's arguments, of the target template's
    /// arguments, in the order of its `{n}`.
    pub args: &'static [u8],
    /// How many arguments the diagnostic must carry for this row to apply.
    pub requires: u8,
}

impl Code {
    /// The rows of the variant table for this code, in the order they are
    /// tried.
    pub fn variants_3_13(self) -> &'static [Variant313] {
        let t = &codes_g::VARIANTS_3_13[..];
        let start = t.partition_point(|v| v.from < self);
        let end = t.partition_point(|v| v.from <= self);
        &t[start..end]
    }

    /// The variant that applies to a diagnostic with `arg_count` arguments:
    /// the first one that fits. An emitter that does not pass the arguments
    /// only 3.13 uses gets no variant, and the diagnostic is reported as in 3.6.
    pub fn variant_3_13(self, arg_count: usize) -> Option<&'static Variant313> {
        self.variants_3_13().iter().find(|v| arg_count >= v.requires as usize)
    }
}

impl serde::Serialize for Code {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.info().unique)
    }
}

impl<'de> serde::Deserialize<'de> for Code {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Code::by_unique(&s).ok_or_else(|| serde::de::Error::custom(format!("unknown code: {s}")))
    }
}

impl serde::Serialize for Severity {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.name())
    }
}

impl<'de> serde::Deserialize<'de> for Severity {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Severity::by_name(&s).ok_or_else(|| serde::de::Error::custom(format!("unknown severity: {s}")))
    }
}

/// Renders a template like the analyzer's `formatList`: each `{n}` becomes
/// `args[n]`. Without arguments the template is returned untouched (the
/// analyzer does not substitute anything either). An index without an
/// argument is left as is.
pub fn format_list(template: &str, args: &[Box<str>]) -> String {
    if args.is_empty() {
        return template.to_string();
    }
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
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
pub struct ContextMessage {
    /// The file of the message, when it is not the diagnostic's own
    /// (absolute path).
    #[serde(default)]
    pub file: Option<Box<str>>,
    pub span: Span,
    pub message: Box<str>,
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
    pub code: Option<Code>,
    /// Effective severity (the code's, unless reconfigured by
    /// `analysis_options.yaml`). Without a code, `Error`.
    #[serde(default)]
    pub severity: Severity,
    /// Template arguments, in the order of the `{n}`.
    #[serde(default)]
    pub args: Box<[Box<str>]>,
    /// The context messages (`contextMessages`); almost always empty.
    #[serde(default)]
    pub context: Vec<ContextMessage>,
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
            severity: Severity::Error,
            args: Box::default(),
            context: Vec::new(),
        }
    }

    /// Adds a context message in the same file as the diagnostic.
    pub fn with_context(mut self, span: Span, message: impl Into<Box<str>>) -> Self {
        self.context.push(ContextMessage { file: None, span, message: message.into() });
        self
    }

    /// Creates a diagnostic with a code: the message comes from the official template.
    ///
    /// ```
    /// use dartforge_diagnostics::{codes, Diagnostic, Span};
    /// let d = Diagnostic::with_code(codes::compile_time_error::UNDEFINED_FUNCTION, Span { start: 0, end: 1 }, ["h"]);
    /// assert_eq!(d.message, "The function 'h' isn't defined.");
    /// assert_eq!(d.code.unwrap().info().name, "undefined_function");
    /// ```
    pub fn with_code<I, A>(code: Code, span: Span, args: I) -> Self
    where
        I: IntoIterator<Item = A>,
        A: Into<Box<str>>,
    {
        let args: Box<[Box<str>]> = args.into_iter().map(Into::into).collect();
        let info = code.info();
        Self {
            message: format_list(info.message, &args),
            span,
            code: Some(code),
            severity: info.severity,
            args,
            context: Vec::new(),
        }
    }

    /// The diagnostic as the analyzer of reference `r` reports it: for 3.6,
    /// itself; for 3.13.4, with the code, text and arguments of the variant
    /// ([`Code::variant_3_13`]), or `None` when 3.13.4 does not report it.
    /// The severity follows the target code's, unless it was already
    /// reconfigured.
    pub fn for_reference(mut self, r: AnalyzerReference) -> Option<Diagnostic> {
        if r == AnalyzerReference::V3_6 {
            return Some(self);
        }
        let Some(from) = self.code else { return Some(self) };
        let Some(v) = from.variant_3_13(self.args.len()) else { return Some(self) };
        let to = v.to?;
        let args: Box<[Box<str>]> = v.args.iter().map(|&i| self.args[i as usize].clone()).collect();
        let info = to.info();
        if self.severity == from.info().severity {
            self.severity = info.severity;
        }
        self.message = format_list(info.message, &args);
        self.code = Some(to);
        self.args = args;
        Some(self)
    }

    /// The correction rendered from the official template, if the code has one.
    pub fn correction(&self) -> Option<String> {
        let template = self.code?.info().correction?;
        Some(format_list(template, &self.args))
    }
}

impl std::fmt::Display for Diagnostic {
    /// Formats the message with its byte range.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(c) = self.code {
            write!(f, "{}: ", c.info().name)?;
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
            format_list("The argument type '{0}' can't be assigned to the parameter type '{1}'. {2}", &a),
            "The argument type 'String' can't be assigned to the parameter type 'int'. "
        );
        assert_eq!(format_list("no {0} args", &[]), "no {0} args");
        assert_eq!(format_list("{x} {9}", &a), "{x} {9}");
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
        let count = |prefix: &str| Code::all().filter(|c| c.info().unique.starts_with(prefix)).count();
        assert_eq!(count("CompileTimeErrorCode."), 542 + 4 + 4 + 5 + 12);
        assert_eq!(count("StaticWarningCode."), 7);
        assert_eq!(count("WarningCode."), 144 + 2 + 1);
        assert_eq!(count("ParserErrorCode."), 265 + 6 + 3 + 1);
        for c in Code::all() {
            assert_eq!(Code::by_unique(c.info().unique), Some(c));
        }
    }

    /// The variant table is sorted, and each row fits the templates.
    #[test]
    fn variants_3_13_sorted_and_consistent() {
        let t = &codes_g::VARIANTS_3_13;
        assert!(t.windows(2).all(|w| w[0].from <= w[1].from));
        for v in t.iter() {
            assert!(v.args.iter().all(|&i| i < v.requires));
            assert_eq!(v.from.variants_3_13().first().map(|p| p.from), Some(v.from));
        }
        assert!(codes::compile_time_error::UNDEFINED_FUNCTION.variants_3_13().is_empty());
    }

    /// Cases c02, c03, c04, c05, c13 and c16 of `corpus/especificacao/t2/v`.
    #[test]
    fn for_reference_switches_name_text_and_arguments() {
        use codes::{compile_time_error as c, parser as p, warning as w};
        let s = Span { start: 0, end: 1 };
        let same = Diagnostic::with_code(c::ENUM_WITHOUT_CONSTANTS, s, Vec::<String>::new());
        assert_eq!(same.clone().for_reference(AnalyzerReference::V3_6), Some(same.clone()));
        let new = same.for_reference(AnalyzerReference::V3_13).unwrap();
        assert_eq!(new.message, "The enum must have at least one enum constant.");
        assert_eq!(new.correction().as_deref(), Some("Try declaring an enum constant."));
        assert_eq!(new.code.unwrap().info().name, "enum_without_constants");

        let field = Diagnostic::with_code(p::REPRESENTATION_FIELD_MODIFIER, s, ["var"]);
        assert_eq!(field.message, "Representation fields can't have modifiers.");
        assert_eq!(field.for_reference(AnalyzerReference::V3_13).unwrap().message, "Representation fields can't have the modifier 'var'.");

        let param = Diagnostic::with_code(w::UNUSED_ELEMENT_PARAMETER, s, ["p"]);
        assert_eq!(param.code.unwrap().info().name, "unused_element");
        let param = param.for_reference(AnalyzerReference::V3_13).unwrap();
        assert_eq!(param.code.unwrap().info().name, "unused_element_parameter");
        assert_eq!(param.message, "A value for optional parameter 'p' isn't ever given.");
        assert_eq!(param.severity, Severity::Warning);

        let superclass = Diagnostic::with_code(c::CONST_CONSTRUCTOR_WITH_NON_CONST_SUPER, s, ["B", "A"]);
        assert!(superclass.message.ends_with("of 'B'."));
        assert!(superclass.for_reference(AnalyzerReference::V3_13).unwrap().message.ends_with("of 'A'."));
        // Without the argument only 3.13 uses, the diagnostic does not change.
        let old = Diagnostic::with_code(c::CONST_CONSTRUCTOR_WITH_NON_CONST_SUPER, s, ["B"]);
        assert_eq!(old.clone().for_reference(AnalyzerReference::V3_13), Some(old));

        let two = Diagnostic::with_code(
            c::AMBIGUOUS_EXTENSION_MEMBER_ACCESS,
            s,
            ["m", "extension 'E1' and extension 'E2'", "extension E1 on int", "extension E2 on int"],
        );
        assert_eq!(
            two.for_reference(AnalyzerReference::V3_13).unwrap().message,
            "A member named 'm' is defined in 'extension E1 on int' and 'extension E2 on int', and neither is more specific."
        );
        // Three or more: the 3.13.4 text is the 3.6.2 one (case c25).
        let three = Diagnostic::with_code(c::AMBIGUOUS_EXTENSION_MEMBER_ACCESS, s, ["m", "extension 'E1', extension 'E2', and extension 'E3'"]);
        assert_eq!(three.clone().for_reference(AnalyzerReference::V3_13), Some(three));

        let private = Diagnostic::with_code(c::NON_EXHAUSTIVE_SWITCH_EXPRESSION, s, ["P", "P._b", "P._b", ""]);
        let private = private.for_reference(AnalyzerReference::V3_13).unwrap();
        assert_eq!(private.message, "The enum 'P' isn't exhaustively matched by the switch cases because some of the enum constants are private.");
        assert_eq!(private.correction().as_deref(), Some("Try adding a wildcard pattern."));

        let outside = Diagnostic::with_code(c::FIELD_INITIALIZER_OUTSIDE_CONSTRUCTOR, s, Vec::<String>::new());
        assert_eq!(outside.for_reference(AnalyzerReference::V3_13), None);
        let from_parser = Diagnostic::with_code(p::FIELD_INITIALIZER_OUTSIDE_CONSTRUCTOR, s, Vec::<String>::new());
        assert!(from_parser.for_reference(AnalyzerReference::V3_13).is_some());
    }

    #[test]
    fn by_name_prefers_own_name() {
        let c = Code::by_name("abstract_field_initializer").unwrap();
        assert_eq!(c.info().unique, "CompileTimeErrorCode.ABSTRACT_FIELD_INITIALIZER");
        assert_eq!(c.info().severity, Severity::Error);
        let w = Code::by_name("unused_local_variable").unwrap();
        assert_eq!(w.info().error_type, ErrorType::StaticWarning);
        assert_eq!(w.info().severity, Severity::Warning);
    }

    #[test]
    fn code_serde_by_unique_name() {
        let d = Diagnostic::with_code(codes::compile_time_error::NON_BOOL_CONDITION, Span { start: 1, end: 2 }, Vec::<String>::new());
        let j = serde_json::to_string(&d).unwrap();
        assert!(j.contains("CompileTimeErrorCode.NON_BOOL_CONDITION"), "{j}");
        let back: Diagnostic = serde_json::from_str(&j).unwrap();
        assert_eq!(back, d);
        let old: Diagnostic = serde_json::from_str(r#"{"message":"x","span":{"start":0,"end":1}}"#).unwrap();
        assert_eq!(old, Diagnostic::new("x", Span { start: 0, end: 1 }));
    }
}
