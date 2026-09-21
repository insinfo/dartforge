# Coleções e operadores: contrato, limites e diagnósticos

Este incremento fecha as lacunas de coleções e operadores do subconjunto:
literais de `Set`, espalhamentos `...`/`...?`, elementos `if` e `for` dentro de
literais, o acesso null-aware `?.`/`?[` com curto-circuito, e os operadores
bit a bit e de deslocamento. O backend é o JavaScript; o LLVM recusa todas as
formas novas com mensagem própria, antes do driver.

Base de comparação: Dart 3.6.2 (`dart` no PATH) e Dart 3.13.4
(`D:/DartSDKs/3.13.4/dart-sdk/bin/dart.exe`). Os fixtures executáveis são
`tests/conformance/cases/colecoes.dart` e `tests/conformance/cases/colecoes_bits.dart`,
e os testes ficam em `crates/compiler/tests/colecoes.rs`.

## Regras que valem em tudo que está aqui

1. **Ordem de avaliação.** Da esquerda para a direita, com cada operando
   avaliado **exatamente uma vez**. Nenhuma subexpressão é duplicada no
   JavaScript emitido: `?.` encadeado e `...?` guardam o valor num temporário
   em vez de repetir o texto da expressão. Dois testes travam isso contando as
   ocorrências no módulo emitido, e não apenas o resultado impresso — o
   resultado sozinho não distingue uma segunda avaliação de uma expressão pura.
2. **`a?.b` é anulável** mesmo quando `b` não é, porque a cadeia inteira produz
   null quando o receptor é null. A única exceção é o seletor `void`:
   `a?.m();` continua sendo instrução e não produz valor.
3. **`...?` sobre null não acrescenta nada**; `...` exige operando não anulável.
4. **`Set` preserva a ordem de inserção**, como o `LinkedHashSet` padrão do Dart.

## Literais de `Set`

```dart
final s = <int>{3, 1, 3, 2};   // {3, 1, 2}
s.add(1);                       // false: já pertencia
s.add(9);                       // true
for (final x in s) print(x);    // 3 1 2 9
```

Membros disponíveis: `add`, `contains`, `length`, `isEmpty`, `isNotEmpty`,
`first`, `last`, `toList`, `map`, `where`, `any`, `forEach` e iteração por
`for-in`. `Set.add` devolve `bool` (false quando o elemento já pertencia),
enquanto `List.add` continua devolvendo `void`, como no oráculo.

A igualdade de elementos é a **identidade** já adotada por `Map` neste
subconjunto (ver [OBJETO.md](OBJETO.md)): `hashCode` e `operator ==` do usuário
não participam da chave. Para os escalares do subconjunto — `int`, `double`,
`String`, `bool`, `null` — isso coincide com o Dart.

### Desambiguação de `{}`

A decisão é **sintática**, nesta ordem:

| Forma escrita | Resultado |
| --- | --- |
| `<K, V>{...}` | mapa |
| `<T>{...}` | conjunto |
| `{}` com contexto `Set<T>` | conjunto vazio |
| `{}` sem argumentos de tipo e sem contexto de conjunto | **mapa vazio** |
| primeiro elemento decisivo é `chave: valor` | mapa |
| primeiro elemento decisivo é um valor solto | conjunto |
| só espalhamentos, sem argumentos de tipo | recusado |

Um elemento `if` ou `for` é inspecionado em profundidade, na ordem escrita, até
encontrar o primeiro elemento decisivo; um espalhamento nunca decide, porque a
forma dependeria do tipo estático do operando. Dart resolve esse caso pela
inferência; aqui ele é recusado com pedido de anotação, para não aceitar
silenciosamente uma semântica que o emissor não escolheu.

## Espalhamentos

```dart
print([...a, b]);                       // lista
print(<int>{...a, 1});                  // conjunto
print(<String, int>{...m, 'b': 2});     // mapa
print([...?talvez()]);                  // omite quando é null
```

`...` exige operando não anulável, exatamente como os SDKs 3.6.2 e 3.13.4, que
respondem `A nullable expression can't be used in a spread`. Por isso, o
lançamento em execução de `...` sobre null só é alcançável por um cast
(`...(x as List<int>)`), e quem lança é o cast, antes do espalhamento; o teste
`espalhar_null_lanca_em_execucao` fixa esse comportamento.

Num literal sem `if` nem `for`, o espalhamento sai como `...` do próprio
JavaScript, dentro do mesmo literal de array de antes: o caminho comum não
ganha construtor imperativo nem alocação extra.

## Elementos `if` e `for`

```dart
[if (c) x]
[if (c) x else y]
[for (var i = 0; i < n; i++) i]
[for (final x in lista) f(x)]
[0, ...base, if (c) 3, for (final x in base) x + 10]
<String, int>{if (c) 'x': 1 else 'y': 2}
<String, int>{for (final n in nomes) n: 7}
```

`else` liga-se sempre ao `if` mais interno (`[if (a) if (b) 1 else 2]` produz
`[2]` quando `a` e não `b`, e `[]` quando não `a`). O cabeçalho do `for`
reaproveita as regras das instruções `for` e `for-in`: a ligação só existe
dentro do elemento, a condição precisa ser `bool` e a atualização roda depois
do elemento. `for (var i = 0; i < 2; i++) 'k': i` produz `{k: 1}`, porque a
última ocorrência de uma chave repetida vence — a regra do `LinkedHashMap`.

Um literal que contenha `if` ou `for` é emitido como um construtor imperativo
(um array local numa função seta invocada na hora), porque essas formas
produzem uma quantidade variável de elementos. Literais sem elas continuam
saindo como o mesmo literal de array de antes.

### Limite: `if` e `for` em mapa sem argumentos de tipo

`{if (c) 'a': 1}` é recusado. A distinção entre mapa e conjunto acontece antes
da análise de tipos, e um ramo de `if` num literal indeciso seria lido como
valor solto, não como entrada. Escreva `<String, int>{if (c) 'a': 1}`.

## Acesso null-aware

```dart
a?.b
a?.b()
a?[i]
a?.b.c      // `.c` só é avaliado quando `a` não é null
a?.b?.c
```

O reconhecimento de `?.` e `?[` exige **adjacência em bytes**, como no scanner
do Dart: `c ? .a : .b`, que combina o operador condicional com os atalhos de
ponto do Dart 3.10, continua sendo um condicional. `?..` é a cascata
null-aware e tem contrato próprio em [IMPLEMENTACAO-20.md](IMPLEMENTACAO-20.md).

O receptor precisa ser anulável. Dart apenas avisa (`invalid_null_aware_operator`)
quando ele não é; aqui isso é recusado, porque um receptor não anulável tornaria
o resultado anulável sem motivo observável.

Emissão: `(($t) => $t === null ? null : <cadeia>)(<receptor>)`. O receptor
aparece uma única vez e cadeias aninhadas empilham temporários distintos.

## Operadores bit a bit e de deslocamento

`|`, `&`, `^`, `~`, `<<`, `>>` e `>>>` (este a partir do Dart 2.14) aceitam
apenas `int` não anulável nos dois lados e produzem `int`. Precedência do Dart,
do mais frouxo para o mais apertado: `|` < `^` < `&` < deslocamentos < `+`/`-`.

`>>` e `>>>` chegam ao parser como `>` adjacentes: o lexer **não** os junta,
para que `List<List<int>>` continue fechando dois argumentos de tipo com o
mesmo token em toda a leitura de tipos. A distinção é puramente léxica — só há
deslocamento sem espaço nem comentário entre os `>`.

### A política de inteiro: alvo web

O DartForge segue a semântica de inteiro do **alvo web**, que é a do
`dart compile js` e o oráculo de `scripts/conformance.ps1`:

- `&`, `|`, `^` e `~` devolvem o inteiro **sem sinal de 32 bits**;
- `<<` devolve 0 a partir de 32 posições e trunca em 32 bits sem sinal;
- `>>>` devolve 0 a partir de 32 posições;
- `>>` com receptor positivo devolve 0 a partir de 32 posições; com receptor
  não positivo, satura a contagem em 31 e devolve o valor sem sinal;
- contagem de deslocamento negativa lança `ArgumentError`, com o mesmo texto
  do alvo web (`Invalid argument: -1`).

Essa é a política **já adotada** pelo projeto para `int`: número do JavaScript,
sem garantia de equivalência integral à VM ([SUBCONJUNTO.md](SUBCONJUNTO.md)).
A aritmética continua sem truncamento — `2147483647 + 1` vale `2147483648` nos
três alvos.

A equivalência com o alvo web foi conferida numa grade de **696 casos** (doze
valores, incluindo negativos, `2^31-1`, `-2^31` e `10^12`, cruzados entre si e
com sete contagens de deslocamento de 0 a 40), comparando a implementação
emitida com `dart compile js -O2` executado no Node: **zero divergências**.

### Divergência conhecida com a VM

A VM usa `int` de 64 bits com sinal. Nos casos em que o resultado tem o bit de
sinal ligado, ou em que o deslocamento passa de 32 bits, a VM e o alvo web
respondem diferente. O fixture `tests/conformance/cases/colecoes_bits.dart`
registra a lista inteira:

| Expressão | Alvo web (DartForge) | VM (`dart run`) |
| --- | --- | --- |
| `~0` | 4294967295 | -1 |
| `~5` | 4294967290 | -6 |
| `-1 \| 0` | 4294967295 | -1 |
| `-2 ^ 0` | 4294967294 | -2 |
| `-8 >> 1` | 4294967292 | -4 |
| `-1 >>> 28` | 15 | 68719476735 |
| `-1 >>> 0` | 4294967295 | -1 |
| `-1 << 1` | 4294967294 | -2 |
| `1 << 31` | 2147483648 | 2147483648 |
| `1 << 32` | 0 | 4294967296 |
| `1 << 40` | 0 | 1099511627776 |
| `8 >> 32` | 0 | 0 |
| `-8 >> 40` | 4294967295 | -1 |
| `2147483647 + 1` | 2147483648 | 2147483648 |

Onde os dois alvos concordam — operandos negativos cujo resultado cabe em 32
bits sem sinal, como `-1 & 3` (3) e `-5 & 255` (251) — o fixture principal
`tests/conformance/cases/colecoes.dart` cobre os casos e a saída é a mesma nos
dois SDKs e no dart2js.

### Limites dos operadores

- `&=`, `|=`, `^=`, `<<=` e `>>=` não existem no subconjunto: as atribuições
  compostas continuam limitadas a `+=`, `-=` e `*=`.
- `double` e `num` não têm esses operadores em Dart, e aqui também não: o
  diagnóstico é `Type mismatch: expected Int, found Double`.
- Um operando anulável precisa de `!` ou `??` antes.

## Diagnósticos, com a mensagem exata

Sintaxe:

```
set literals do not accept `key: value` entries              {1, 'a': 2}
map literals require `key: value` entries                    {'a': 1, 2}
a literal built only from spreads needs explicit <T> or <K, V> type arguments   {...a}
if and for elements in map literals require explicit <K, V> type arguments      {if (c) 'a': 1}
await for in collection literals is not supported            [for await (...) x]
nested null-aware collection elements are not supported      [??x]
list literals require exactly one type argument              <int, int>[1]
```

Semântica:

```
Empty Set requires explicit element type or context          <T>{} sem contexto
Unsupported set element type                                 elemento void ou não resolvido
A nullable expression cannot be spread; use `...?`           ... sobre anulável
Spread requires a List, Set or Iterable                      [...1]
Spread in a map requires a Map                               <String, int>{...lista}
Null-aware access requires a nullable receiver               lista?.length
Null-aware access on a null-only value is unsupported        null?.x
This element form is valid only inside a collection literal  forma de elemento fora de literal
Unsupported for header in a collection literal               cabeçalho fora das duas formas
Type mismatch: expected Int, found ...                       operando não int em bits/deslocamento
```

LLVM (recusa na validação da HIR, antes do driver):

```
LLVM AOT ainda não suporta conjuntos e elementos `...`/`if`/`for` de coleção
LLVM AOT ainda não suporta o acesso null-aware `?.`
LLVM AOT ainda não suporta operadores de bits e deslocamento (`&`, `|`, `^`, `~`, `<<`, `>>`, `>>>`)
LLVM AOT ainda não suporta o complemento de bits `~`
```

## O que continua fora

- `Set` literal `const`, `Set.from`, `Set.of`, `remove`, `union`, `difference`,
  `intersection` e o restante da API de `dart:core`.
- `if-case` em literais (`[if (x case int y) y]`) e `await for`.
- Espalhamento de um `Iterable` preguiçoso dentro de literal `const`.
- `?.` sobre membros estáticos e `?.call()`.
- Deslocamento com contagem fora de `int` e atribuições compostas de bits.
- Qualquer uma dessas formas tem diagnóstico próprio; nenhuma é aceita em
  silêncio.

## Desempenho

O caminho comum não mudou de forma: um literal sem espalhamento e sem `if`/`for`
continua saindo como `new $dartforgeList([...], T)` e não passa por construtor
imperativo. A decisão é um `any` sobre um slice emprestado, sem alocação por nó
nem por elemento. As cópias do `Validator` acontecem apenas onde o fluxo se
ramifica de verdade — um `if` de coleção e o cabeçalho de um `for` de col