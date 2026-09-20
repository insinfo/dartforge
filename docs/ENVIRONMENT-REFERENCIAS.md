# Ambiente de compilação: evidências Dart 3.6.2

Implementação original em Rust, consultando o SDK na tag `3.6.2`, commit
`b0cc5495e0f5e8ae150825a5352e708cb49e65ff`. O checkout corrente do clone pode
ser outro; as consultas usaram `git -C references/dart-sdk show 3.6.2:<arquivo>`.
Não houve cópia de código SDK para os fontes MIT.

## Fontes primárias consultadas

- `sdk/lib/libraries.json`: inventários `vm`/`vm_common`, `dart2js`/`_dart2js_common`, `wasm`/`wasm_common` e marcadores `supported`.
- `pkg/front_end/lib/src/source/source_loader.dart`, `getLibrarySupportValue`: nomes fora de `dart.library.*` retornam string vazia; uma biblioteca indisponível retorna `null`, inclusive para impedir correspondência com `== ''`.
- `pkg/front_end/lib/src/source/source_builder_factory.dart`: import e export percorrem configurações na ordem da fonte, selecionam a primeira igualdade e interrompem a busca.
- `pkg/kernel/lib/target/targets.dart`, `DartLibrarySupport`: suporte depende da biblioteca, do inventário e das restrições do alvo; ausência não é uma definição cujo valor seja a string `false`.
- `pkg/compiler/lib/src/kernel/dart2js_target.dart`, `Dart2jsDartLibrarySupport`: exceção explícita `_dart2js_only` disponível para condicionais.
- `pkg/vm/lib/modular/target/vm.dart`: mirrors depende de `supportMirrors`.
- `pkg/dart2wasm/lib/target.dart`: ffi não é anunciado pelo alvo padrão sem o experimento específico.

## Matriz observada

As linhas agrupam chaves com resultados iguais. `true` significa que
`String.fromEnvironment('dart.library.<nome>', defaultValue:'ABSENT')` retornou
`true`; ausente significa que retornou `ABSENT`. Não é uma lista universal
de flags de todas as versões/configurações do Dart.

| Nomes | dart2js | Native AOT | dart2wasm |
|---|---|---|---|
| async, collection, convert, core, developer, math, typed_data | true | true | true |
| html, html_common, indexed_db, js, svg, web_audio, web_gl | true | ausente | ausente |
| js_interop, js_interop_unsafe, js_util | true | ausente | true |
| cli, concurrent, ffi, io, vmservice_io | ausente | true | ausente |
| isolate, nativewrappers | ausente | true | true |
| mirrors | ausente | ausente | ausente |
| _dart2js_only | true | ausente | ausente |
| _ddc_only, _internal, wasm, unknown | ausente | ausente | ausente |

VM JIT e AOT **diferem**: o mesmo programa no VM JIT anunciou mirrors; o
executável AOT não. `CompilationEnvironment::native()` reproduz AOT, pois é
o backend nativo do projeto. O inventário de bibliotecas também confirma a
ausência de `dart:ui` nesses perfis SDK sem Flutter.

## Experimentos executados

Em `target/environment-review/` foram gerados pequenos programas originais.
As saídas foram efetivamente executadas, não inferidas dos bytes dos artefatos:

1. `dart.exe flags.dart`: VM JIT.
2. `dart.exe compile exe flags.dart -o flags.exe`, seguido de `flags.exe`: AOT.
3. `dart.exe compile js flags.dart -o flags.js`, seguido de `node flags.js`: dart2js.
4. `dart.exe compile wasm flags.dart -o flags.wasm`, seguido de Node importando
   `flags.mjs`, chamando `compile(readFileSync(...))`, `instantiate({})` e
   `invokeMain()`: dart2wasm.

O executável consultado foi `C:/tools/dartsdk-3.6.2/bin/dart.exe`.

Verificação adicional de seleção real em Wasm:

```dart
import 'default.dart'
    if (dart.library.js_util) 'first.dart'
    if (dart.library.js_interop) 'second.dart';
void main() { print(choice()); }
```

Cada arquivo implementou `choice()` retornando seu nome. Compilar com
dart2wasm 3.6.2 e executar no Node imprimiu `first`. Portanto, nesse SDK,
`dart.library.js_util` **é true no Wasm**, e a primeira condição vence quando
duas são verdadeiras. Isso não significa que toda API legada de js_util
seja utilizável em qualquer ambiente ou que o DartForge a tenha implementado.

Outros resultados confirmados por execução:

- Nome comum ausente: `if (absent.key == '')` selecionou o ramo.
- Biblioteca ausente: `if (dart.library.unknown == '')` não selecionou o ramo.
- Com `-Dcustom=yes`, `String.fromEnvironment('custom')` imprimiu `yes`, mas
  a URI `if (custom == 'yes') first if (custom == '') second` selecionou
  `second`, tanto no VM quanto no dart2js. O frontend 3.6.2 consulta suporte
  de bibliotecas para URIs, não as definições comuns de ambiente.
- `-Ddart.library.io=false` não substituiu a flag do alvo: no VM ela continuou
  `true`. O DartForge não expõe mutação das flags reservadas.

## Contrato do DartForge

`CompilationEnvironment` contém apenas um alvo imutável. `get` consulta flags
de bibliotecas; `condition` reproduz a regra de URIs descrita acima. Condição
sem igualdade compara exatamente com `true`, respeitando maiúsculas/minúsculas.
O frontend é responsável pela primeira correspondência e pelo URI padrão.
Não há leitura implícita das variáveis do sistema nem opção `-D` nesta etapa.

Esses são **perfis de seleção compatíveis com o SDK**, não declarações de
implementação das bibliotecas. No DartForge, somente o subconjunto de `dart:core`
é atualmente atendido. Selecionar um arquivo que importe outra biblioteca
continua produzindo diagnóstico explícito. O perfil Wasm serve à inspeção
do grafo; não cria ou promete um backend Wasm.
