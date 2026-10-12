//! Formatador de Dart: porte em Rust do `dart_style` 2.3.8, a versão que o
//! `dart format` do SDK 3.6.2 traz. O SDK 3.6.2 formata no estilo "curto"
//! (o `SourceVisitor` de `lib/src/short/`); o estilo "alto" só existe atrás do
//! experimento `tall-style` e não é portado.
//!
//! O fluxo é o mesmo do original: [`sintaxe`] lê a fonte na árvore do
//! analyzer, o visitante de [`curto`] a percorre gerando *chunks* com regras
//! de quebra, e o divisor de linhas escolhe as quebras de menor custo que
//! cabem na largura da página.
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

/// Opções do formatador (`DartFormatter`).
#[derive(Debug, Clone)]
pub struct Opcoes {
    /// Largura da página (`pageWidth`), 80 por padrão.
    pub largura: usize,
    /// Recuo inicial de todas as linhas (`indent`).
    pub recuo: usize,
    /// Fim de linha; `None` infere da fonte (`\r\n` se a primeira quebra for
    /// `\r\n`, senão `\n`).
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

/// Falha ao formatar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroFormatacao {
    /// A fonte tem erro de sintaxe (`FormatterException`).
    Sintaxe(ErroSintaxe),
    /// A saída mudou mais que espaços (`UnexpectedOutputException`): erro do
    /// formatador.
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

/// Formata uma unidade de compilação inteira (`DartFormatter.format`).
///
/// # Erros
/// [`ErroFormatacao::Sintaxe`] quando a fonte não é Dart válido e
/// [`ErroFormatacao::SaidaInesperada`] se a saída não for igual à entrada a
/// menos de espaços (a mesma checagem de sanidade do `dart_style`).
pub fn formatar(source: &str, options: &Opcoes) -> Result<String, ErroFormatacao> {
    let (tokens, tree) = syntax::parser::parse(source).map_err(ErroFormatacao::Sintaxe)?;
    let line_ending = options.fim_de_linha.clone().unwrap_or_else(|| infer_line_ending(source));
    let root = tree.root.expect("unidade de compilação");
    let visitor = SourceVisitor::new(source, &tokens, &tree, options.largura, options.recuo as i32, line_ending);
    let (mut arenas, chunks) = visitor.run(root);
    let mut cache = BlockCache::new();
    let result = LineWriter::new(chunks, &mut cache).write_lines(&mut arenas, true);
    if !equal_ignoring_whitespace(source, &result.texto) {
        return Err(ErroFormatacao::SaidaInesperada {
            input: source.to_string(),
            output: result.texto,
        });
    }
    Ok(result.texto)
}

/// `\r\n` se a primeira quebra de linha da fonte for `\r\n`; senão `\n`.
fn infer_line_ending(source: &str) -> String {
    let info = short::visitor::LineInfo::from(source);
    let starts = info.starts();
    if starts.len() > 1 && starts[1] >= 2 && source.as_bytes()[starts[1] - 2] == b'\r' {
        "\r\n".into()
    } else {
        "\n".into()
    }
}

/// `string_compare.equalIgnoringWhitespace`: compara ignorando brancos e
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
