# AOT nativo: referências verificadas e roteiro

## Identificação do experimento histórico

O experimento citado no anexo é o **Dartino LLVM**, não uma branch LLVM moderna do Dart SDK 3.6.2. O artigo primário [Dart-on-LLVM, de Erik Corry, publicado em 11/01/2017](https://dart.dev/blog/dart-on-llvm) aponta para `dartino/sdk/tree/llvm` e o fork LLVM de Erik Corry. A URL histórica `https://github.com/dartino/sdk` redireciona hoje para **[dart-archive/sdk](https://github.com/dart-archive/sdk)**. O metadado `isArchived` desse repositório foi confirmado como `true` via GitHub CLI; no fork LLVM foi confirmado como `false`.

| Referência local | Repositório e branch verificados | Commit fixado | Data do commit |
| --- | --- | --- | --- |
| `references/dartino-llvm` | `https://github.com/dartino/sdk.git`, branch `llvm`; canonical `dart-archive/sdk` | `07f12fa22c5a3c2652d258dde3770848e4542819` | 01/12/2016 |
| `references/llvm-dartino` | `https://github.com/ErikCorryGoogle/llvm.git`, branch `dartino-llvm` | `e29ca27990a14fc9b5795a67814b44711bb9bc11` | 02/12/2016 |
| Dart SDK alvo | `dart-lang/sdk`, tag `3.6.2` | `b0cc5495e0f5e8ae150825a5352e708cb49e65ff` | Consultado pela tag, sem alterar o checkout de referência |

`git ls-remote --heads` confirmou também `dartino/sdk` branch `llvm-shadowstack`, em `bf0a554066be225fe36e1fee324fc1949fb22b58`. Essa branch adicional foi identificada, mas não clonada como outra implementação e não foi usada como se fosse a branch `llvm`.

Os dois clones usam `--depth 1 --filter=blob:none --single-branch`, com checkout esparso. Dartino materializa `src/vm` e os arquivos da raiz; LLVM materializa `include/llvm/IR`, `lib/Transforms/Scalar` e a raiz. Outros arquivos permanecem disponíveis por `git show HEAD:<caminho>` e podem ser materializados sob demanda. Isso evita baixar todo o histórico e toda a árvore LLVM. Os forks antigos **não foram compilados**.

## Arquivos históricos lidos

| Arquivo | Observação extraída do código |
| --- | --- |
| `dartino-llvm/src/vm/codegen_llvm.cc` | Traduz operações e funções Dartino para IR LLVM. Usa `statepoint-example`, reescrita de statepoints, metadados de GC, chamadas/invoke e bitcode. A aritmética de pequenos inteiros usa detecção de overflow e outra representação numérica. |
| `dartino-llvm/src/vm/codegen_llvm_main.cc` | Carrega um snapshot Dartino e chama o gerador; não é um parser Dart moderno. |
| `dartino-llvm/src/vm/gc_llvm.cc` | Percorre stack maps e atualiza ponteiros base/derivados movidos pelo coletor. Existem pressupostos explícitos sobre layout do frame e tipos de localizações suportadas. |
| `dartino-llvm/src/vm/llvm_eh.cc` | Usa unwinder, identificação de exceção, personality/LSDA e codificações DWARF. Não demonstra suporte automático ao ABI de exceções Windows atual. |
| `dartino-llvm/src/vm/llvm_embedder.cc` | Inicializa stack maps/runtime, carrega dados estáticos do programa e executa o main Dartino. |
| `dartino-llvm/compile.sh` | Pipeline histórico snapshot → bitcode → assembly → objeto → link com runtime C++; o script usa ferramentas e bibliotecas do ambiente x64 Unix daquela época. |
| `llvm-dartino/CMakeLists.txt` | Declara versão LLVM 3.9.0. |
| `llvm-dartino/lib/Transforms/Scalar/LICM.cpp` | Contém tratamento do metadado customizado `never.faults`. |
| `llvm-dartino/lib/Transforms/Scalar/RewriteStatepointsForGC.cpp` | Torna explícitas as relocações de referências em pontos de GC. |

O artigo descreve um frontend derivado do dart2js e bytecodes Dartino como entrada do backend, além de experimentos com GC móvel e exceções. Isso fornece uma referência histórica de integração, não compatibilidade com Dart 3.6.2 nem comprovação de desempenho do DartForge. [Fonte primária](https://dart.dev/blog/dart-on-llvm).

## Diferenças que impedem reaproveitamento direto

O código é anterior ao null safety moderno. Seus bytecodes, representações de objetos, APIs LLVM, runtime e ABI não são o contrato do DartForge. A presença de statepoints não implementa o coletor: ainda é preciso definir raízes, safepoints, atualização de ponteiros, barreiras, layouts e fronteiras entre runtime e código gerado. O unwinder antigo não fornece sozinho exceções Dart portáveis. A integração textual com LLVM atual será original e usará o frontend Rust existente; nenhuma parte dos forks foi copiada para as crates MIT.

As licenças originais permanecem em `dartino-llvm/LICENSE.md` (BSD com exceções e componentes externos separados) e `llvm-dartino/LICENSE.TXT` (licença histórica University of Illinois/NCSA e componentes listados separadamente). Ambos os diretórios continuam ignorados pelo Git do DartForge. A licença MIT do projeto não substitui essas licenças.

## Semântica de int nativo: referência exata 3.6.2

Foi lido `git -C references/dart-sdk show 3.6.2:sdk/lib/core/int.dart`. A documentação dessa versão estabelece inteiros nativos de **64 bits, complemento de dois, com wrap no overflow**. O mesmo arquivo distingue o alvo JavaScript, representado por números double e sujeito a diferenças de precisão e operações bit a bit. [Fonte fixada no commit 3.6.2](https://github.com/dart-lang/sdk/blob/b0cc5495e0f5e8ae150825a5352e708cb49e65ff/sdk/lib/core/int.dart).

Experimento reproduzido em `target/semantic-review/aot/wrap.dart`, usando somente literais que cabem no parser i32 atual:

```dart
void main() {
  int x = 1073741824;
  x = x * x;
  x = x * 8;
  print(x);
  print(x - 1);
  print(-x);
  print(x * 2);
}
```

Dart VM 3.6.2 imprimiu, nessa ordem: `-9223372036854775808`, `9223372036854775807`, `-9223372036854775808`, `0`. O mesmo programa em dart2js 3.6.2 `-O2` e Node imprimiu números double diferentes. Essa diferença é esperada entre os alvos e **não** deve ser mascarada pelo backend AOT.

Consequências para emissão: operações nativas `add/sub/mul i64` não recebem flags `nsw`/`nuw`; negação usa `sub i64 0, valor` com wrap. Comparações numéricas usam a interpretação assinada. O oráculo diferencial nativo é Dart VM/AOT 3.6.2, não a saída dart2js. O domínio dos literais do frontend ainda precisa ser distinguido da largura das operações do backend; ampliar literais para i64 exige alteração e testes próprios do parser/AST.

## Arquitetura e plano do primeiro incremento

Pipeline proposto: **frontend Rust compartilhado → HIR validada → LLVM IR textual → clang emite objeto → link por rustc com runtime Rust → executável**. LLVM/Clang continuam dependências externas, predominantemente C++; isso não significa que toda a cadeia seja implementada em Rust.

1. **Contrato de alvo e ABI.** Fixar arquitetura, triple e data layout compatíveis entre clang e rustc; começar no host Windows x64. Definir funções de runtime com ABI C explícita para impressão de i64 e bool, evitando depender de layouts internos Rust. Guardar o `.ll` e diagnósticos das ferramentas para inspeção.
2. **Subconjunto sem heap.** Suportar funções e parâmetros `int`, `bool`, `void`, locais, atribuições, comparações, operadores já implementados, `if`, `while`, `do/while`, `for`, `break`, `continue`, return e impressão desses valores. Operações inteiras usam i64 com wrap. Curto circuito deve preservar efeitos; continue de for deve passar pela atualização, e continue de do/while pela condição.
3. **Emissão verificável.** Gerar blocos de controle terminados corretamente e valores/armazenamento tipados. Compilar LLVM IR para `.obj` com clang. Integrar ao runtime e à entrada por rustc. Recursos fora do contrato geram diagnóstico explícito antes de invocar a toolchain.
4. **Validação diferencial.** Comparar executável gerado com Dart 3.6.2 nativo em funções, recursão, escopos, operadores, retornos, laços, curto circuito e limites de overflow. Verificar exit code, stdout e falhas de compilação. Testar as opções de otimização nativa sem assumir equivalência com otimização JavaScript.
5. **Medições separadas.** Registrar parsing/análise/emissão, clang, link e execução. Não afirmar vantagem sobre Dart AOT, DDC ou dart2js sem programas compatíveis e medições reproduzíveis; o primeiro marco é correção de executáveis reais.

**Fora do primeiro incremento:** alocação de objetos, GC, strings dinâmicas, classes e despacho nativos, closures, generics, dynamic, coleções, exceções Dart, async/await, isolates, dart:io e FFI geral. A HIR já reconhecer alguns desses recursos para JavaScript não significa suporte nativo. GC, objetos e exceções **não estão prontos** por existir o backend LLVM ou por termos clonado os forks históricos.

Depois de estabilizar o subconjunto sem heap, o plano deve abrir marcos separados para representação de valores/objetos, runtime de strings e coleções, estratégia de GC e raízes, despacho, closures, null safety em execução e exceções por plataforma. Essas decisões precisam de testes próprios antes de ampliar a declaração de compatibilidade.
