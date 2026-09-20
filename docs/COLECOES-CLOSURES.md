# Coleções, funções como valores e capturas — Dart 3.6.2

## JavaScript

O frontend representa tipos estruturais em uma arena: `List<T>`, `Iterable<T>` e
`R Function(A, B)`. As formas podem ser aninhadas e atravessar imports. O incremento
18 acrescenta funções genéricas com bounds, Object/Object? e formas estruturais
anuláveis. Classes genéricas, typedef e dynamic permanecem pendentes.

Listas expansíveis aceitam literais `[1, 2]` ou `<int>[]`, indexação, substituição
por índice, `add`, `length`, `isEmpty`, `isNotEmpty`, `first`, `last` e `toList`.
Listas vazias precisam de tipo explícito ou contexto. `where`, `map`, `forEach`
e `any` contextualizam os parâmetros omitidos dos callbacks. Desde o incremento
18, List permite covariância e preserva o tipo real do elemento para verificar
escritas em execução. Iterable permite covariância apenas de leitura.

Closures aceitam corpo de expressão ou bloco, podem ser armazenadas, passadas,
retornadas e chamadas como expressões. JavaScript preserva o ambiente léxico,
compartilha bindings mutáveis e mantém capturas vivas após retorno da função.
Closures de métodos usam arrow functions para capturar `this`. Tear-offs de funções
top-level são aceitos; tear-offs de métodos e funções locais nomeadas ainda não.

`where` e `map` são preguiçosos e repetem os efeitos a cada travessia. Consultar
`map.length` ou `map.isEmpty` não executa o conversor; `map.last` transforma apenas
o último elemento, conforme o SDK 3.6.2. Modificar estruturalmente uma lista durante
travessia é detectado. Indexação inválida produz erro no runtime JS; ainda não há
hierarquia completa de exceções Dart nem try/catch neste frontend.

A biblioteca padrão inicial é um subconjunto intrínseco de `dart:core`. Import
explícito sem filtros é aceito pelo carregador de arquivos. Exportação, filtros
show/hide de core, outras bibliotecas `dart:`, Set, Map, coleções const, spreads,
collection-if/for, `List()` e a API completa de Iterable continuam pendentes.
A impressão de listas é completa; a de iteráveis segue os limites de abreviação
do SDK 3.6.2, incluindo contagem UTF-16 e interrupção após 101 elementos.
Iteráveis aninhados são formatados sem antecipar os efeitos de seus elementos.

## Null safety e otimização

A análise elimina promoções de capturas mutáveis ao entrar no corpo de uma closure
e impede promoção de nomes escritos por closures. É deliberadamente conservadora;
pode rejeitar casos válidos até haver análise de captura/fluxo por IDs de bindings.
Retorno e controle de laço de cada closure são independentes do contexto externo.

A fusão opt-in de funções fica desativada no módulo quando existem closures ou
referências a funções como valores. Essa política preserva identidade de funções
top-level e ambientes diferentes, mesmo com corpos iguais. Constantes ainda podem
ser simplificadas dentro dos corpos sem executar callbacks nem compartilhar closures.

## LLVM e GC

O [runtime Rust](../crates/runtime/CONTRACT.md) já representa células mutáveis,
ambientes, closures e listas com referências rastreadas, incluindo ciclos e capturas
que sobrevivem ao retorno. **O lowering dessas construções para LLVM ainda não está
implementado**: o backend emite diagnóstico, inclusive para construções não executadas.
O operador `%` deste incremento também está disponível somente no backend JS.

Próximas etapas: IDs de bindings, closure conversion, ABI de chamadas indiretas,
boxing seletivo das capturas escritas, raízes para ambientes/listas e execução
nativa diferencial. Depois, genéricos declarados pelo usuário, substituição de tipos,
checks reificados e especialização orientada por medições.

## Referências

SDK [3.6.2](https://github.com/dart-lang/sdk/tree/3.6.2): `sdk/lib/core/list.dart`,
`sdk/lib/core/iterable.dart`, `sdk/lib/collection/list.dart`,
`sdk/lib/internal/iterable.dart`, `tests/language/closure/closure2_test.dart` e
`tests/language/function_type/function_type0_test.dart`. Fixtures próprias e layout de objetos próprio. A rotina de formatação abreviada
adapta o algoritmo do SDK; sua licença BSD e atribuição estão preservadas em
[THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md) e na saída JS.
