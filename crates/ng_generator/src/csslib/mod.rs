//! Porte do csslib 1.0.2 (`package:csslib`) no que o shim de estilo do
//! ngcompiler usa: `parse(css, errors: …)` com as opções padrão, a árvore, o
//! `Visitor` e a `CssPrinter` compacta.
//!
//! - [`tokenizer`] — `src/tokenizer.dart`, `src/tokenizer_base.dart`,
//!   `src/token.dart`;
//! - [`token_kind`] — `src/token_kind.dart`;
//! - [`parser`] — o `_Parser` de `parser.dart`;
//! - [`arvore`] — `src/tree.dart`, `src/tree_base.dart`;
//! - [`visitor`] — `visitor.dart`;
//! - [`impressora`] — `src/css_printer.dart`.
//!
//! Fica de fora o que o caminho do shim não percorre: `analyzer.dart`
//! (`compile`, não `parse`), `polyfill.dart`, `tree_printer.dart`,
//! `validate.dart` e, de `property.dart`, os valores dos estilos para Dart
//! (ninguém os lê; ver [`parser`]). As opções são as padrão do `parse`:
//! `checked` falso (os avisos condicionados a ele não existem) e
//! `lessSupport` verdadeiro.
pub mod arvore;
pub mod impressora;
pub mod parser;
pub mod token_kind;
pub mod tokenizer;
pub mod visitor;

/// Uma exceção que o código oficial lança (o nome do tipo do Dart, ou a
/// descrição de um laço sem fim): o builder falharia.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Excecao(pub &'static str);

impl std::fmt::Display for Excecao {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

/// `parse(input, errors: errors)`: a folha, lida do texto como o Dart o vê
/// (unidades UTF-16). Os erros de parse não interrompem a leitura.
pub fn parse(css: &str) -> Result<arvore::Folha, Excecao> {
    let fonte: Vec<u16> = css.encode_utf16().collect();
    let mut p = parser::Parser::novo(&fonte)?;
    p.parse()
}
