# Referências — incremento 06: imports, combinadores e reexports

## Versão usada como referência normativa de implementação

- Dart SDK **3.6.2**, tag `3.6.2`, commit `b0cc5495e0f5e8ae150825a5352e708cb49e65ff`.
- Leitura dos arquivos feita com `git -C references/dart-sdk show 3.6.2:<caminho>`. O checkout não foi trocado.
- SDK instalado usado nos experimentos: `C:\tools\dartsdk-3.6.2\bin\dart.exe`.
- O HEAD do clone (`6c4009687d2c881f880127fc12b9ad7c72111d33`, tools/VERSION 3.14.0 main) foi consultado inicialmente apenas como contexto. Não foi tratado como se fosse a versão 3.6.2.

## Arquivos oficiais consultados no commit 3.6.2

| Arquivo no Dart SDK | Decisão verificada |
| --- | --- |
| `tests/language/import/combinators_test.dart` | Composição de show/hide e distinção entre nomes contextuais e palavras das diretivas. |
| `tests/language/export/cyclic_test.dart` | Reexports transitivos e ciclos de exports precisam convergir sem recursão infinita. |
| `tests/language/export/local_a.dart` | Declaração própria prevalece sobre nome vindo de export. |
| `tests/language/export/duplicate_export_test.dart` | Reexportar a mesma declaração por caminhos distintos não é ambiguidade. |
| `tests/language/export/duplicate_collision_test.dart` | Colisões de declarações diferentes em exports geram erro, inclusive sem referência ao nome em main. |

Também foi inspecionado, apenas como contexto no HEAD mais novo, `pkg/analyzer/test/src/dart/resolution/library_export_test.dart`, cobrindo resolução dos combinadores e nomes inexistentes. Não copiamos código dos testes do SDK para a implementação.

## Experimentos com Dart 3.6.2 instalado

Fontes temporárias foram gravadas em `target/semantic-review/exports/` e executadas com o SDK fixado.

1. `import 'names.dart' show a,b show a;` com chamada de `a()` é aceito e imprime `1`; o analyzer emite somente warning sobre o nome `b` mostrado e não utilizado. Portanto, múltiplos combinadores continuam suportados neste alvo.
2. Um barrel com `export 'names.dart' show a,b hide b;` permite a chamada de `a()` pelo importador e imprime `1`.
3. O mesmo barrel, com export irrestrito e declaração própria `int a(){return 10;}`, faz o importador imprimir `10`: a declaração própria substitui a exportada.
4. `export 'names.dart'; void main(){print(a());}` produz `undefined_function` no analyzer. Export não equivale a import para o escopo local da biblioteca.

O HEAD mais novo contém `tests/language/export/single_combinators/single_combinators_error_test.dart` com a opção experimental `single-combinators`, datado de 2026. Essa restrição **não foi aplicada** ao alvo 3.6.2, contrariaria o experimento 1.

## Implementação derivada das observações

- `show` mantém somente nomes listados; `hide` remove nomes listados. A composição é aplicada à mesma origem declarativa e nunca torna `_privados` públicos.
- Exports usam um ponto fixo monotônico: cada nome carrega um conjunto de origens, não apenas o texto do identificador. Assim, diamantes da mesma origem são idempotentes e conflitos de origens diferentes permanecem detectáveis.
- Declarações próprias sobrepõem exports antes da união; imports não entram no conjunto exportado.
- O escopo de uma biblioteca contém suas declarações próprias e os namespaces exportados pelos imports diretos, após filtros. Reexports são transitivos; imports comuns continuam não transitivos.
- Imports ambíguos ainda são rejeitados conservadoramente mesmo sem uso do nome. Prefixos `as`, combinadores condicionais e extensions importadas seguem fora deste incremento.

## Validação automatizada prevista

Testes do linker cobrem composição de filtros, ambiguidade resolvida por hide, reexports com ciclos e diamantes, identidade de origem, precedência local, privacidade, distinção import/export e execução de classes reexportadas em Node nos modos direto e otimizado. Os testes de carregamento das diretivas e resolução de URIs pertencem à crate packages.

## package_config v2

Frontend consultou antes da implementação a especificação local references/dart-language/accepted/2.8/language-versioning/package-config-file-v2.md, revisão 9cdc5a5e8ecfbedca4e10d3ae880e82258dc346b, e package_config 2.2.0 instalado no Pub cache: lib/src/package_config_json.dart, SHA256 3542ABCBCA87E571BA3A3AE7E25FEDBADC0DAF90A991BD47410896820329A8B2.

Decisões, limites e oracle executado estão em docs/PACKAGES.md. Os experimentos alvo3.6.2 validaram percentencoding Unicode/espaço, diretórios sem barra terminal e packageUri omitida. SDK HEAD3.14 não foi tratado como alvo; tag3.6.2 resolve para b0cc5495e0f5e8ae150825a5352e708cb49e65ff.

### Revisão adicional de namespaces com o oráculo 3.6.2

- Um barrel com `export 'a.dart'; export 'b.dart';`, ambos declarando `f`, gera `ambiguous_export` no analyzer e erro de compilação dart2js mesmo quando o importador usa `hide f`. Portanto, conflitos do próprio namespace exportado não são suprimidos pelo filtro do importador.
- Um barrel que importa `a.dart` (f retorna 1), exporta `b.dart` (f retorna 2) e declara `int local(){return f();}` faz um importador de barrel imprimir `3` para `f()+local()`. Isso confirma que o import resolve o corpo local sem substituir o símbolo reexportado.
- A implementação usa fila de dependentes para o ponto fixo: só propaga namespaces alterados, preservando identidade de origem e terminando quando nenhuma origem nova pode ser adicionada.
