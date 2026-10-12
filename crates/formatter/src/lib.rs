//! Dart formatter: a Rust port of `dart_style` 2.3.8, the version shipped with
//! `dart format` in SDK 3.6.2. SDK 3.6.2 formats in the "short" style (the
//! `SourceVisitor` in `lib/src/short/`); the "tall" style only exists behind
//! the `tall-style` experiment and is not ported.
//!
//! The flow is the same as the original: [`syntax`] reads the source into the
//! analyzer's tree, the visitor in [`short`] walks it producing *chunks* with
//! split rules, and the line splitter picks the cheapest splits that fit in
//! the page width.
//!
//! ```
//! use dartforge_formatter::{formatar, Opcoes};
//! let output = formatar("main(){print( 'oi' );}", &Opcoes::default()).unwrap();
//! assert_eq!(output, "main() {\n  print('oi');\n}\n");
//! ```
pub mod short;
pub mod edicoes;
pub mod syntax;

use short::writer::{BlockCache, LineWriter};
use short::visitor::SourceVisitor;
pub use syntax::token::ErroSintaxe;

/// Formatter options (`DartFormatter`).
#[derive(Debug, Clone)]
pub struct Opcoes {
    /// Page width (`pageWidth`), 80 by default.
    pub largura: usize,
    /// Initial indentation of every line (`indent`).
    pub recuo: usize,
    /// Line ending; `None` infers it from the source (`\r\n` if the first line
    /// break is `\r\n`, otherwise `\n`).
    pub fim_de_linha: Option<String>,
}

impl Default for Opcoes {
    fn default() -> Opcoes {
        Opcoes {
            largura: 80,
            recuo: 0,
            fim_de_linha: None,
        }
    }
}

/// Formatting failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroFormatacao {
    /// The source has a syntax error (`FormatterException`).
    Sintaxe(ErroSintaxe),
    /// The output changed more than whitespace (`UnexpectedOutputException`):
    /// a formatter bug.
    SaidaInesperada { input: String, output: String },
}

impl std::fmt::Display for ErroFormatacao {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErroFormatacao::Sintaxe(e) => write!(f, "{}", e.mensagem),
            ErroFormatacao::SaidaInesperada { input, output } => write!(
                f,
                "The formatter produced unexpected output. Input was:\n{input}\nWhich formatted to:\n{output}"
            ),
        }
    }
}

impl std::error::Error for ErroFormatacao {}

/// Formats a whole compilation unit (`DartFormatter.format`).
///
/// # Errors
/// [`ErroFormatacao::Sintaxe`] when the source is not valid Dart, and
/// [`ErroFormatacao::SaidaInesperada`] if the output differs from the input
/// beyond whitespace (the same sanity check `dart_style` does).
pub fn formatar(source: &str, options: &Opcoes) -> Result<String, ErroFormatacao> {
    let (tokens, tree) = syntax::parser::parse(source).map_err(ErroFormatacao::Sintaxe)?;
    let line_ending = options.fim_de_linha.clone().unwrap_or_else(|| infer_line_ending(source));
    let root = tree.root.expect("compilation unit");
    let visitor = SourceVisitor::new(source, &tokens, &tree, options.largura, options.recuo as i32, line_ending);
    let (mut arenas, chunks) = visitor.run(root);
    let mut cache = BlockCache::new();
    let result = LineWriter::new(chunks, &mut cache).write_lines(&mut arenas, true);
    if !equal_ignoring_whitespace(source, &result.text) {
        return Err(ErroFormatacao::SaidaInesperada {
            input: source.to_string(),
            output: result.text,
        });
    }
    Ok(result.text)
}

/// `\r\n` if the source's first line break is `\r\n`; otherwise `\n`.
fn infer_line_ending(source: &str) -> String {
    let info = short::visitor::LineInfo::from(source);
    let starts = info.starts();
    if starts.len() > 1 && starts[1] >= 2 && source.as_bytes()[starts[1] - 2] == b'\r' {
        "\r\n".into()
    } else {
        "\n".into()
    }
}

/// `string_compare.equalIgnoringWhitespace`: compares ignoring whitespace and
/// `,`, `[`, `]`, `{`, `}`, `;`.
pub fn equal_ignoring_whitespace(a: &str, b: &str) -> bool {
    fn blank(c: u16) -> bool {
        matches!(c, 0x2c | 0x5b | 0x5d | 0x7b | 0x7d | 0x3b | 0x09..=0x0d | 0x20 | 0x85 | 0xa0 | 0x1680 | 0x2000..=0x200a | 0x2028 | 0x2029 | 0x202f | 0x205f | 0x3000 | 0xfeff)
    }
    let mut x = a.encode_utf16().filter(|&c| !blank(c));
    let mut y = b.encode_utf16().filter(|&c| !blank(c));
    loop {
        match (x.next(), y.next()) {
            (None, None) => return true,
            (Some(p), Some(q)) if p == q => {}
            _ => return false,
        }
    }
}
