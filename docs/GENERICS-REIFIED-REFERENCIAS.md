# Genéricos reificados: primeira etapa do incremento 18

Esta etapa atende funções genéricas top-level com bounds, nulabilidade, testes
`is`/`is!` e casts `as`. Classes e métodos genéricos do usuário exigem um trabalho
posterior de instanciação nominal e escopos de parâmetros; não são apagados para
simular suporte. `List` e `Iterable` continuam sendo formas estruturais conhecidas.

## Contrato semântico

O bound omitido é `Object?`, que admite tipos nullable. Assim, `identity<int?>(null)`
é válido para `T identity<T>(T value) => value`. Declarar `T extends Object`
proíbe usar `int?` como argumento de tipo. `T` e `T?` são distintos na AST:
`T?` acrescenta null ao argumento escolhido, mesmo quando o bound de T é não nullable.

```dart
abstract class LoginService {
  int login();
}
int run<T extends LoginService>(T service) => service.login();
bool accepts<T>(Object? value) => value is T;
T checked<T>(Object? value) => value as T;
T? maybe<T extends Object>(T? value) => value;
```

Todos os corpos genéricos são analisados sob os bounds declarados, inclusive
funções nunca chamadas. Um método inexistente em `LoginService` não fica válido
porque determinada instanciação concreta possuiria esse método. Isso evita uma
especialização que esconda erros em declarações não utilizadas.

Argumentos explícitos e inferidos são verificados contra bounds substituídos.
Bounds dependentes não recursivos são resolvidos sem substituir referências
adiantadas prematuramente por `Object?`. Bounds recursivos, incluindo F-bounds,
recebem diagnóstico explícito neste recorte.

`Resolution.generic_arguments` registra argumentos de tipo por intervalo da
chamada. Dentro de outra função genérica, esses argumentos podem continuar
simbólicos e precisam consultar o ambiente reificado do chamador. Os backends
não podem substituir todo T por Object nem decidir um `is T` apenas pelo nome
da classe JavaScript.

## Coleções, funções e inferência

Listas passam a admitir covariância com checagem de escrita pelo descritor do
elemento original. Por exemplo, atribuir uma `List<int>` a uma referência
`List<int?>` é permitido estaticamente, mas inserir null nessa lista deve falhar
antes da mutação. O descritor não pode ser alargado junto com o tipo da referência.
`Iterable` preserva seu tipo de elemento; `map` precisa registrar o tipo do resultado.

O supertipo comum é relevante para testes reificados. O literal
`[<int>[1], <String>['s']]` tem tipo `List<List<Object>>`, não `List<Object>`.
A análise calcula esse resultado recursivamente. Para classes nominais, usa um
ancestral comum mais específico quando único. Para funções de mesma aridade,
compara parâmetros contravariantemente e resultados covariantemente. Interseções
de parâmetros não representáveis, aridades diferentes e ancestrais incomparáveis
não são aproximados silenciosamente: exigem diagnóstico/contexto explícito.

Funções monomórficas e closures carregam assinaturas de execução para testes de
tipo. Tear-offs de funções genéricas e de métodos continuam fora do recorte.
`print` não é aceito como valor funcional. A impressão de Object arbitrário não
é confundida com o protocolo JavaScript `String(objeto)`.

## Promoções e limites

Testes `is`/`is!` promovem locais quando o tipo testado pode representar um
subtipo do tipo declarado. Curto-circuito e retornos antecipados preservam os
controles conservadores já existentes. Campos não são promovidos; uma closure
capaz de escrever no local impede promoção insegura. Atribuições e laços
invalidam informações que deixaram de ser garantidas.

Não são implementados dynamic, Never, interseções gerais como `T & Object`,
tipos de função genéricos ou toda a inferência de bounds da especificação. Formas
que exigem essas representações são recusadas explicitamente. Um cast aceito
estaticamente ainda deve verificar o valor na execução e falhar quando incompatível.

## Referências fixadas e oracle

Fontes consultadas no SDK tag `3.6.2`, commit
`b0cc5495e0f5e8ae150825a5352e708cb49e65ff`, com `git show`:

- `sdk/lib/core/object.dart`: Object como raiz dos objetos não null.
- `tests/language/generic/function_bounds_test.dart`: dependências entre bounds,
  instanciação e checagens de argumentos. Os testes SDK também cobrem caminhos
  dynamic que não fazem parte do subconjunto atual.
- `tests/language/generic_methods/bounds_test.dart`: incompatibilidade de bound
  detectada estaticamente ou em execução conforme a forma da chamada.

Experimentos originais em `target/semantic-review/` foram executados com
`C:/tools/dartsdk-3.6.2/bin/dart.exe`:

- `reified18.dart` confirmou chamada via bound LoginService e produziu
  `42`, `true`, `false`, `true`, `null`, `true` para os testes de nulabilidade,
  Object e lista aninhada. O analyzer apenas avisou que o último teste de tipo
  era necessariamente verdadeiro.
- `reified_promotions.dart` confirmou que `Object? x = 1` não promove x para int
  simplesmente pela atribuição: `x + 1` foi rejeitado. A mesma fonte permitiu
  atribuir `List<int>` a `List<int?>` e tentar inserir null, cuja segurança deve
  ser garantida em execução.

Regressões semânticas estão em `crates/semantic/tests/reified.rs`; fixtures
diferenciais compartilhadas ficam em `tests/conformance/modules/reified18/`.
Essas evidências não afirmam cobertura completa dos genéricos Dart 3.6.2 nem
disponibilidade dos novos recursos no backend LLVM, que deve diagnosticar suas
limitações em vez de emitir uma representação apagada incorreta.
