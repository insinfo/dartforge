# Avaliação inicial do Rust Sitter

Referências fornecidas pelo proprietário:
- https://www.shadaj.me/writing/introducing-rust-sitter
- https://github.com/hydro-project/rust-sitter

O clone está em references/rust-sitter, com commit registrado no manifesto local.
A licença é MIT. O artigo de 2022 apresenta gramáticas anotadas em tipos Rust, geração
de parser Tree Sitter e extração de AST tipada.

Verificação do código atual clonado:
- runtime/Cargo.toml usa tree-sitter-c2rust por padrão, um runtime convertido para Rust.
- tool/src/lib.rs, build_parsers, ainda gera parser.c e usa cc::Build para compilá-lo.
- Portanto runtime em Rust não significa que todo o parser gerado seja Rust.

Decisão deste incremento: manter o parser próprio em Rust e usar Rust Sitter como referência
de desenho de gramática, precedência e extração de AST. Não foi adicionado como dependência.
Isso respeita o objetivo de implementar o compilador em Rust e evita uma migração sem
medição de custo/benefício. O novo suporte a funções foi implementado no parser existente.

Avaliação futura: comparar corpus, diagnóstico/recuperação, memória, latência de parse completo
e edições incrementais. Conferir o caminho de geração e dependências transitivas antes de
adotar qualquer alternativa como parte de uma implementação inteiramente em Rust.
