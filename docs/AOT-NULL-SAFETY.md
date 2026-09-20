# Null safety no backend AOT escalar

## Referências e verificação

Alvo: Dart SDK **3.6.2**, commit `b0cc5495e0f5e8ae150825a5352e708cb49e65ff`. Antes de especificar os casos, foram lidos por `git show 3.6.2:<arquivo>`:

- [sdk/lib/core/null.dart](https://github.com/dart-lang/sdk/blob/b0cc5495e0f5e8ae150825a5352e708cb49e65ff/sdk/lib/core/null.dart): valor null, operadores e representação textual.
- [tests/language/if_null/behavior_test.dart](https://github.com/dart-lang/sdk/blob/b0cc5495e0f5e8ae150825a5352e708cb49e65ff/tests/language/if_null/behavior_test.dart): seleção de operandos nulos e não nulos.
- [tests/language/nnbd/syntax/non_null_assertion_test.dart](https://github.com/dart-lang/sdk/blob/b0cc5495e0f5e8ae150825a5352e708cb49e65ff/tests/language/nnbd/syntax/non_null_assertion_test.dart): asserções que preservam valores e falham em null.
- [tests/language/nnbd/type_promotion/assignment_inference_after_null_check_test.dart](https://github.com/dart-lang/sdk/blob/b0cc5495e0f5e8ae150825a5352e708cb49e65ff/tests/language/nnbd/type_promotion/assignment_inference_after_null_check_test.dart): refinamento de tipos não equivale a substituir o tipo declarado por Null.

Também foi consultado `references/dartino-llvm/src/vm/codegen_llvm.cc`, commit `07f12fa22c5a3c2652d258dde3770848e4542819`: o experimento histórico usa `null_object` e valores tagged. Isso não estabelece uma ABI de null safety moderna e não foi copiado para esta implementação.

O teste original `tests/native/cases/null_safety.dart` foi executado em `C:/tools/dartsdk-3.6.2/bin/dart.exe`; sua saída foi gravada em `null_safety.stdout`. A versão JavaScript gerada pelo frontend atual executou em Node e produziu a mesma saída. A verificação do novo executável nativo deve usar essa saída como oráculo nos níveis de otimização suportados.

## Contrato do subconjunto

- `int?` e `bool?` aceitam o primitivo correspondente e null. Passagem de argumentos, atribuição e retorno preservam essa distinção. Função com retorno anulável que alcança o fim retorna null implicitamente.
- `??` avalia o operando esquerdo uma vez. Somente se ele for null avalia o direito; o resultado pode continuar anulável. O valor `false` e o inteiro zero são valores presentes.
- `!` avalia seu operando uma vez, devolve o primitivo presente e falha em null. O teste inclui uma função que imprime um marcador antes de devolver o valor.
- Comparações de igualdade entre null e null são verdadeiras mesmo quando os tipos declarados são `int?` e `bool?`. Valores presentes de tipos distintos não são iguais: `0 == false` é false. Não comparar apenas os bits do payload.
- `print` de um valor ausente imprime `null`; presente imprime o primitivo. Não se deve ler um payload indefinido antes de consultar a presença.
- A análise compartilhada permite promoção após comparação com null, nos ramos aplicáveis, em curto circuito `&&`/`||` e após retorno antecipado. Uma atribuição que permite null invalida a promoção; não se deve confiar em uma promoção anterior na emissão.

Representação interna implementada pelo emissor: agregado LLVM `{ i1, i64 }` para `int?` e `{ i1, i1 }` para `bool?`, com tag true indicando presença. Null convertido a esses agregados tem tag false; o payload não representa um objeto nem um ponteiro. Essa ABI é interna ao programa gerado, não um layout Rust público estável. Chamadas ao runtime usam funções C explícitas para impressão e falha.

Não há necessidade de GC para esses valores escalares. Isso não fornece suporte nativo a classes, strings, objetos, `this`, closures ou coleções.

## Falhas e limitações explícitas

Em Dart, `!` sobre null lança uma exceção. Neste marco, `null_assert_fail` encerra o processo com código **101** e diagnóstico: é uma falha imediata e **não uma exceção Dart capturável por catch**. Deve ser testada em subprocesso; nenhuma instrução posterior pode executar. Um experimento com `int? absent() { return null; }` e `print(absent()!)` confirmou a falha correspondente no Dart VM; o código de saída da VM não é o contrato do runtime DartForge.

`var value = null; value = 4; value = false;` é válido em Dart 3.6.2 e imprime os dois valores quando solicitado: a variável infere dynamic. O frontend DartForge rejeita a inferência a partir de null com diagnóstico explícito de dynamic não suportado. Não deve aceitar esse programa inferindo um tipo Null restritivo, nem afirmar suporte completo a inferência Dart. A alternativa suportada é declarar `int?` ou `bool?` conforme necessário.

O analyzer 3.6.2 confirmou os seguintes casos negativos, preparados em `target/semantic-review/native-null/negative.dart`:

| Caso | Erro confirmado |
| --- | --- |
| `void f(int? x) { print(x + 1); }` | Uso incondicional de valor anulável |
| `void f(bool? x) { if (x) { print(1); } }` | Condição anulável |
| `int f() { return null; }` | Retorno incompatível |
| `if (x != null) { x = null; print(x + 1); }` | A escrita impede usar a promoção anterior |
| `int? x = null; x = false;` | Atribuição incompatível |

Os arquivos de testes são originais. Os testes e implementações SDK/Dartino permanecem apenas referências com suas licenças próprias.
