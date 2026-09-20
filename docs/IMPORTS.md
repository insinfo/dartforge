# Grafo de imports relativos

A crate `dartforge-packages` carrega arquivos UTF-8 e constrói um grafo de dependências.
Esta crate cuida do carregamento; **não resolve nomes nem compila múltiplas unidades**.
A compilação conjunta já é oferecida por `dartforge-linker` e pelo CLI `compile`,
conforme [a documentação de bibliotecas](MODULES.md).

A API `load(&Path) -> Result<SourceGraph, GraphError>` entrega `units` e `entry`. Cada
unidade contém caminho canônico, fonte original e imports. Cada import registra URI,
ID do destino e span da diretiva completa, em bytes da fonte importadora. `GraphError`
fornece caminho, mensagem e span opcional; erros de acesso à entrada não têm span.

## Subconjunto

São aceitas diretivas `import 'relativo.dart';` ou strings raw equivalentes antes de
qualquer declaração. Comentários e espaços podem separar diretivas. O caminho usa `/`,
é relativo ao diretório do importador e pode conter `..`. Não há confinamento ao
diretório da entrada: o carregador acessa as dependências relativas declaradas.

Caminhos canônicos são deduplicados, inclusive aliases como `./a.dart` e `sub/../a.dart`.
A ordem de descoberta em largura e a ordem textual dos imports determinam IDs estáveis.
Imports repetidos mantêm arestas distintas para a mesma unidade. Ciclos são aceitos pelo
carregador; isso não significa que namespaces ou inicialização cíclica estejam resolvidos.

Não são suportados `package:`, `dart:`, caminhos absolutos, escapes em URIs, percent-encoding,
query, fragmento, `as`, `show`, `hide`, `deferred`, imports condicionais, `export`, `part`
ou `library`. Diretivas import depois de declarações são rejeitadas. Todos esses casos
produzem erro explícito, sem tentar resolver parcialmente a diretiva.

O lexer atual tokeniza o arquivo inteiro. Assim, a descoberta de imports ainda exige
que as fontes pertençam ao subconjunto léxico do protótipo, mesmo que os corpos não
sejam analisados semanticamente nesta etapa. Não existe cache persistente: a deduplicação
vale por chamada de `load`. Não há resolução de pacotes, pubspec ou package_config.

## Verificação

    cargo test -p dartforge-packages

Os testes usam diretórios temporários exclusivos e cobrem dependências aninhadas,
diamantes, aliases, ciclos, ordem dos IDs, spans por arquivo, arquivos ausentes,
diretivas não suportadas e uma cadeia carregada iterativamente.
