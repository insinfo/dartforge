# FFI e anotações: referência Dart 3.6.2

Referência fixada: SDK tag `3.6.2`, commit
`b0cc5495e0f5e8ae150825a5352e708cb49e65ff`. Foram lidos os arquivos dessa
revisão com `git show`, independentemente do checkout atual de `references/dart-sdk`.
A implementação Rust é original; nenhum código com licença do SDK foi copiado.

## Fontes consultadas

- `sdk/lib/ffi/ffi.dart`: documentação e declaração de `Native<T>` e `DefaultAsset`.
- `sdk/lib/core/annotations.dart`: `override`, `Deprecated` e `deprecated`.
- `tools/experimental_features.yaml`: macros permanece uma funcionalidade experimental nessa revisão.
- Pacote local `ffi` **2.1.3**, em `C:/Users/pmro/AppData/Local/Pub/Cache/hosted/pub.dev/ffi-2.1.3/`: `lib/src/arena.dart` e `lib/src/utf8.dart`. Trata-se de `package:ffi`, distinto de `dart:ffi` e versionado separadamente do SDK.

## O que as anotações significam

`@override` fornece informação a ferramentas sobre um membro de instância que
sobrescreve outro membro. Não muda despacho, assinatura nem resultado do programa.
O analyzer pode avisar quando a declaração não sobrescreve nada; não é correto
tratar toda ausência de sobrescrita como erro obrigatório da linguagem.

`@deprecated` e `@Deprecated('mensagem')` sinalizam descontinuação para ferramentas.
Não removem nem proíbem executar o membro. O DartForge reconhece esse subconjunto
de metadata, mas ainda não implementa o catálogo de warnings do analyzer. Aceitar
metadata não significa oferecer geração de código por anotações ou macros.

No subconjunto atual, `@override` é aceito em membros, enquanto sua aplicação a
classes/funções top-level é recusada explicitamente. Essa é uma restrição do
frontend DartForge, não uma alegação de que metadata modifica a semântica Dart.
Metadata arbitrária, anotações em campos e execução de macros não estão prontas.
O status experimental de macros no SDK 3.6.2 não deve ser descrito como suporte
estável, tampouco como implementação existente neste compilador.

## Native e external

`@Native<T>` associa uma declaração `external` a uma implementação ou localização
nativa. O SDK 3.6.2 também permite certos membros estáticos e variáveis externas;
o recorte deste projeto contempla somente **funções top-level externas**.

Para funções, `T` representa a assinatura nativa, não a assinatura Dart. Por
exemplo, `Int32 Function(Int64)` corresponde a `int Function(int)` no nível Dart,
mas especifica larguras distintas na ABI. Nullable, objetos gerenciados e tipos
de função genéricos não podem ser tratados como inteiros por conveniência.

O SDK não determina que `@Native` sempre resolva símbolos estaticamente na
linkagem. Sua documentação descreve resolução por asset fornecido/default,
resolver registrado com `Dart_SetFfiNativeResolver` e processo corrente, nessa
ordem. O compilador/runtime e a plataforma determinam o mapeamento. Se nenhuma
origem fornecer o símbolo, a chamada/acesso falha. Sem `symbol`, o nome da
declaração Dart é usado; isso deve ser fixado antes de qualquer renomeação do linker.

`assetId` e `@DefaultAsset` fazem parte desse modelo SDK, mas ainda não são
implementados pelo DartForge. Não devem ser aceitos e ignorados silenciosamente.

`isLeaf` no SDK é um contrato restritivo: a função deve ser curta, não bloqueante,
não chamar Dart e não usar APIs do VM. Não é uma opção genérica de segurança ou
uma promessa universal de desempenho. O projeto preserva o booleano literal na
AST; isso não equivale a implementar o protocolo de safepoints/GC do VM Dart.

## Recorte validado neste incremento

- Resultados nativos `Int32`, `Int64` e `Void`; parâmetros `Int32` e `Int64`.
- Correspondência exata com Dart `int` ou `void`, sem nullable.
- Mesma aridade nas assinaturas Dart e nativa; parâmetros Dart com nomes únicos.
- Declaração externa sem corpo Dart, sem exigência artificial de um `return`.
- Funções top-level, sem genéricos, getters, métodos ou entrada `main` externa.
- Símbolos limitados a identificadores C ASCII neste subconjunto. Colisões com
  símbolos do runtime são verificadas adicionalmente pelo backend LLVM.
- `isLeaf` literal e `symbol` literal/default; opções não implementadas produzem
  diagnóstico. A presença de `dart:ffi` e o prefixo da anotação são conferidos
  pelo pipeline de imports.

Ponteiros, callbacks nativos, structs/unions, strings C, arenas e alocadores FFI
não estão implementados por esse recorte. Emissão JavaScript não pode fingir
executar o binding nativo.

## Arena, Utf8 e using

`Arena` e `Utf8` vêm de **`package:ffi/ffi.dart`**, não de `dart:ffi` nem de
palavras-chave da linguagem. `Utf8` representa memória UTF-8 terminada em zero
através de ponteiros; não é sinônimo de uma `String` Dart gerenciada.

`using` é uma função genérica do pacote `ffi`. Ela cria uma arena, executa um
callback e libera os recursos no encerramento apropriado, incluindo tratamento
do resultado assíncrono na implementação consultada. Não existe aqui uma nova
instrução da linguagem equivalente a `using` de outras linguagens. Implementar
essa API exige primeiro suporte real a ponteiros, alocação/liberação e callbacks.

## Experimentos com o SDK instalado

Executado `C:/tools/dartsdk-3.6.2/bin/dart.exe analyze` sobre fontes originais em
`target/ffi-review/`, sem carregar bibliotecas nativas arbitrárias:

```dart
import 'dart:ffi';
@Native<Int32 Function(Int32)>(symbol: 'increment')
external int inc(int x);
@Native<Void Function(Int64)>(symbol: 'consume', isLeaf: true)
external void consume(int x);
void main() {}
```

O analyzer retornou `No issues found!`. Substituir o parâmetro Dart do primeiro
binding por `bool` produziu `must_be_a_subtype`. Isso verifica tipagem; não
comprova que os símbolos existam ou que a ABI da biblioteca carregada corresponda
à declaração. Os testes do projeto cobrem também aridade, nullable, resultado,
corpo indevido, genéricos e opções não implementadas.

### Oracle real de chamadas externas

A fixture compartilhada agora fica em `tests/ffi/main.dart`, `native.c` e
`main.stdout`; o teste Rust `crates/compiler/tests/ffi_native.rs` usa
`include_str!` desses mesmos arquivos. O C expõe cinco símbolos e inclui um
macro de exportação para DLL Windows, com visibilidade equivalente em outros
sistemas.

O oracle **executou as próprias declarações `@Native`**, sem substituí-las por
`lookupFunction` ou closures. Clang compilou uma DLL com os cinco símbolos
exportados. Um pequeno arquivo Dart separado importou a fixture original,
chamou `DynamicLibrary.open` com o caminho da DLL e então `fixture.main()`.
Esse wrapper foi executado no VM 3.6.2 e compilado/executado como AOT 3.6.2.
Ambos produziram exatamente:

```text
-1
-2147483648
7
4294967303
-42
12
2
```

O resultado verifica truncamento e extensão de sinal de Int32, preservação de
Int64, retorno Void, estado C compartilhado e avaliação ordenada de argumentos.
Evidências locais: `target/ffi-review/oracle-vm.stdout`, `oracle-aot.stdout`,
`wrapper.dart` e `wrapper.exe`. O sucesso da resolução após carregar a DLL foi
observado em Windows; não foi generalizado para os loaders de outros sistemas.

`scripts/conformance-ffi.ps1` reproduz o teste em Windows com SDK fixado e
compara VM, AOT e DartForge O0/O2, gravando relatório JSON e saídas em
`target/ffi-conformance/<id>/`. Aceita `-DartExe`, `-ClangExe` e `-SkipBuild`.
A fixture C também é reutilizada pelos testes nativos Rust multiplataforma.
O script foi executado localmente após o build release, com aprovação em O0/O2.
O [relatório integrado](conformance-ffi-increment-16-17.json) conserva resultados
e hashes das fontes compartilhadas.

Há uma distinção intencional: o SDK resolve os símbolos da DLL carregada durante
a execução; o DartForge recebe o objeto C explicitamente com `--link-object`.
Saídas equivalentes não demonstram igualdade entre esses mecanismos de resolução
nem suporte ao sistema de assets nativos do SDK.
