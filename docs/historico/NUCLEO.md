# Núcleo de `dart:core`: tipos base e membros de biblioteca

Este documento registra o que o subconjunto implementa dos **tipos base** e dos
**membros** de `dart:core`, o que falta, e — o que importa mais — a **política
semântica de cada decisão que não é tradução óbvia** para JavaScript.

O oráculo é o Dart 3.6.2 (`dart`) e o Dart 3.13.4
(`D:/DartSDKs/3.13.4/dart-sdk/bin/dart.exe`); os dois produzem a mesma saída em
todos os programas desta página. Os testes estão em
`crates/compiler/tests/nucleo.rs`.

## O ponto de partida: onze membros

Antes deste trabalho o compilador reconhecia **onze** nomes de membro no total —
`add`, `contains`, `first`, `isEmpty`, `isNotEmpty`, `last`, `length`, `toList`,
`toString`, `values`, `where` — e **nenhum** em `String`, `int` ou `double`:
`'abc'.substring(1)` parava em `Unknown instance or extension method`.

A escolha de quais acrescentar veio de contagem, não de julgamento. Contando as
ocorrências nos quatro pacotes medidos de `references/pub/` (`pdf` 3.13.1, `intl`
0.20.3, `collection` 1.19.1, `http` 1.6.0):

| Ocorrências | Membro | | Ocorrências | Membro |
| --- | --- | --- | --- | --- |
| 360 | `length` | | 38 | `toList` |
| 212 | `add` | | 36 | `round` |
| 87 | `isNotEmpty` | | 35 | `codeUnitAt` |
| 66 | `values` | | 33 | `hashCode` |
| 62 | `map` | | 31 | `toInt` |
| 61 | `toString` | | 30 | `iterator` |
| 59 | `substring` | | 29 | `padLeft` / `addAll` |
| 56 | `containsKey` | | 26 | `forEach` |
| 55 | `isEmpty` | | 24 | `sublist` / `abs` |
| 50 | `contains` | | 22 | `reduce` / `join` |
| 49 | `write` | | 21 | `keys` / `clear` |
| 47 | `current` | | 20 | `split` / `cast` |
| 44 | `moveNext` | | 19 | `startsWith` / `last` / `indexOf` / `filled` |
| 39 | `toDouble` | | 17 | `compareTo` |

E os **nomes de tipo** de `dart:core`, por arquivos afetados:

| Ocorrências | Arquivos | Tipo |
| --- | --- | --- |
| 225 | 50 | `Iterable` |
| 187 | 25 | `DateTime` |
| 104 | 23 | `RegExp` |
| 89 | 28 | `Uri` |
| 81 | 28 | `Type` |
| 36 | 9 | `Comparable` |
| 30 | 13 | `Iterator` |
| 24 | 6 | `MapEntry` |
| 20 | 4 | `Comparator` |
| 16 | 8 | `StringBuffer` |
| 13 | 6 | `Error` |
| 12 | 8 | `Exception` |
| 4 | 2 | `Stopwatch` |

## Como os tipos nominais de `dart:core` existem sem declaração

Não há fonte de `dart:core` neste projeto: `int`, `String` e `Object` são
variantes de `dartforge_syntax::Type`, e `List`, `Set`, `Iterable` e `Map` são
formas estruturais em `TypeShape`. `Comparable<T>`, `Iterator<T>`, `Exception` e
`StringBuffer` não cabem em nenhum dos dois: são tipos **nominais**, que uma
classe do usuário implementa e que aparecem em anotação de tipo.

Eles usam uma **faixa reservada de identificadores de classe** no topo de `u32`
(`dartforge_syntax::NUCLEO_COMPARABLE` e vizinhos). O ligador atribui
identificadores de classe a partir de zero, um por declaração do programa, e já
recusa com `excesso de classes` muito antes de chegar perto da faixa.

A propriedade que torna isso seguro é concreta: um identificador reservado
aparece **apenas** em `Class::interfaces` e em `Type::Class`, e cada tabela do
ligador que os consulta usa `get`, não indexação — um identificador sem
declaração atravessa sem alterar nada e sem panicar. Nada é emitido por ele:
`implements` não herda implementação. A análise semântica sintetiza as entradas
correspondentes na própria tabela de classes, com os membros que o contrato de
`implements` exige e todo o resto vazio.

A alternativa seria acrescentar variantes a `Type`/`TypeShape`, cujos `match`
exaustivos vivem em `crates/linker`, `crates/llvm`, `crates/cranelift-jit` e
`crates/asmjit-jit`. A faixa reservada entrega os mesmos tipos nominais sem
tocar em nenhum deles.

### Uma classe homônima do programa sempre ganha

`Comparable`, `Iterator`, `Exception`, `Iterable`, `StringBuffer` e `Comparator`
só resolvem para a faixa reservada quando o programa **não** declara uma classe
com aquele nome. É a mesma regra que `Timer` já seguia. Declarar
`class Exception { ... }` na própria biblioteca faz `implements Exception`
apontar para ela.

A cláusula `implements` lê qualquer palavra como nome de interface, inclusive
as reservadas (`Iterable`, `List`, `String`): na posição de tipo, a palavra é
um nome, não um uso. Sem isso, `class C implements Iterable<int>` — a forma
que todo programa real usa — morria em `expected a non-reserved identifier`.
E um nome conhecido mas não modelado (`Error`, `StackTrace`, `Uri`...) cai na
recusa nomeada de `nucleo_sem_modelo`, nunca em `unknown interface` genérico.

## `Object` como raiz

O protocolo `Object` — `toString`, `hashCode`, `operator ==` e `identical` —
já estava implementado antes deste trabalho e está descrito em
[OBJETO.md](OBJETO.md); nada dele foi refeito aqui. O que este trabalho
acrescentou é o **alcance**: `toString()` e `hashCode` passaram a existir também
em `String`, `int`, `double` e `bool`, que antes não tinham membro nenhum.

### `runtimeType` e `noSuchMethod` continuam recusados, agora com a razão

```
runtimeType is unsupported: reading it would force every class of the program to carry its Dart name into the emitted JavaScript, because the text is chosen by the object and not by the static type — the same cost that made this subset refuse "Instance of 'Name'"; use 'is' or 'as' to recover the concrete type
```

```
noSuchMethod is unsupported: it only has an effect under dynamic dispatch, which this subset refuses on purpose; declare the member, or use 'is' to branch on the concrete type
```

A recusa é a mesma decisão registrada em OBJETO.md sobre `Instance of 'Nome'`:
o texto é escolhido pelo objeto, não pelo tipo estático, então honrar
`runtimeType` obrigaria **toda** classe do programa a carregar seu nome Dart no
JavaScript gerado, inclusive nos programas que nunca leem `runtimeType`.

### `hashCode` dos escalares e o que ele promete

| Tipo | Emissão | Valor igual ao do SDK? |
| --- | --- | --- |
| `int` | `$dartforgeNumHash` | sim para inteiros pequenos, não em geral |
| `double` | `$dartforgeNumHash` | não |
| `bool` | `1231` / `1237` | sim (é o que o oráculo 3.6.2 produz) |
| `String` | `$dartforgeStringHash` | **não** |

O Dart **não especifica** o valor de `hashCode`, e a VM e o dart2js já discordam
entre si. O único contrato que existe é "valores iguais têm o mesmo hash", e é
esse que a emissão preserva. Nenhum teste compara o número com o do SDK, porque
comparar afirmaria uma garantia que não existe.

### `hashCode` de coleção é recusado

```
'hashCode' on 'List<int>' is unsupported: collections inherit the identity hash of Object — two lists with the same elements have different hashes — and reproducing that would need an identity table kept only for this member; compare with 'identical', or hash the elements you care about
```

`List`, `Set` e `Map` herdam o `hashCode` de `Object`, que é **identidade**:
`[1, 2].hashCode != [1, 2].hashCode` no oráculo. Devolver um hash **estrutural**
daria a resposta errada com aparência de certa, e reproduzir a identidade exigiria
uma tabela lateral mantida só por causa deste membro.

## `Comparable<T>` e o contrato de ordenação

```dart
class Versao implements Comparable<Versao> {
  final int maior;
  final int menor;
  Versao(this.maior, this.menor);
  @override
  int compareTo(Versao outra) {
    if (maior != outra.maior) { return maior - outra.maior; }
    return menor - outra.menor;
  }
  @override
  String toString() { return '$maior.$menor'; }
}

void main() {
  var vs = <Versao>[Versao(1, 2), Versao(1, 0), Versao(0, 9)];
  vs.sort();
  print(vs);                                  // [0.9, 1.0, 1.2]
  print(Versao(1, 0).compareTo(Versao(1, 2))); // -2
}
```

`compareTo` existe em `int`, `double`, `num` e `String`, e `implements
Comparable<T>` numa classe concreta **exige** `int compareTo(P)` com um `P`
capaz de receber a própria classe:

```
'Versao' declares 'implements Comparable' but no 'int compareTo(Versao other)': the ordering contract is what 'sort()' and 'compareTo' resolve against, so it cannot be left out
```

### Por que `Comparable` não usa a checagem comum de contrato

O subconjunto **apaga** argumentos de tipo de classe (`docs/TIPOS.md`), então um
contrato herdado de `Comparable<T>` teria o parâmetro apagado para `Object`. A
análise aplica **contravariância** a todo override — o valor aceito pela base
tem de continuar sendo aceito pela derivada — e `int compareTo(Versao)` não aceita
`Object`. A checagem comum recusaria o programa que o Dart aceita.

A exigência é conferida à parte, por uma regra que diz exatamente o que
`Comparable<Self>` significa na prática: um `compareTo` que receba a própria
classe. Uma classe **abstrata** que declare `implements Comparable` está isenta,
como no Dart.

O método **entra** na tabela sintética como `int compareTo(Object?)`, para que
chamadas por receptor de tipo `Comparable` (`Comparable<Versao> c = ...;
c.compareTo(...)`) resolvam — mas `validate_contracts` isenta contratos vindos
desse ancestral, senão a contravariância comum voltaria pela porta dos fundos.
E `nucleo_comparavel` procura a implementação só na classe e nos `extends`
(`implementation`, nunca `method`): olhar `implements` acharia o contrato
apagado na própria classe que deveria implementá-lo e liberaria `implements
Comparable` sem `compareTo`.

### `sort()` sem comparador recusa em compilação o que o Dart recusa em execução

```
'sort()' without a comparator orders by 'compareTo', and 'P' declares none: declare 'int compareTo(P other)' and 'implements Comparable<P>', or call 'sort((a, b) => ...)'
```

Esta é uma divergência **deliberada e mais forte** que o oráculo. No Dart,
`dart analyze` aceita `List<P>.sort()` com `P` não comparável e o programa falha
em execução com `TypeError`. Este subconjunto resolve todo membro
estaticamente, então recusa em compilação. A forma com comparador —
`sort((a, b) => ...)` — sempre funciona e nunca é recusada.

`sort` é emitido sobre o `Array.prototype.sort` do JavaScript, que é **estável**
desde ES2019. O `List.sort` do Dart não promete estabilidade, então esta
implementação é mais forte, nunca mais fraca.

## `Iterator<T>` e `Iterable<T>` implementáveis pelo usuário

```dart
class Contagem implements Iterator<int> {
  int _atual = 0;
  final int _limite;
  Contagem(this._limite);
  @override
  int get current => _atual;
  @override
  bool moveNext() {
    if (_atual >= _limite) { return false; }
    _atual = _atual + 1;
    return true;
  }
}
```

`implements Iterator<T>` exige `bool moveNext()` e um getter `current`. O
contrato sintetizado declara `Object? get current`, porque o argumento de tipo é
apagado; `int get current` o satisfaz por **covariância de retorno**, que a
análise já aplica a qualquer override.

`implements Iterable<T>` exige `Iterator<T> get iterator`.

### O limite: `for-in` sobre uma `Iterable` do usuário é recusado

```
'for-in' over a class that implements Iterable<T> is unsupported: this subset erases class type arguments, so the element would bind as Object?; iterate explicitly with 'var it = x.iterator; while (it.moveNext()) { ... it.current ... }', which keeps the type your own Iterator declares
```

A razão é o apagamento, e ela é específica: `Iterator<T> get iterator` chega à
análise como `Iterator`, sem o `T`. Um `for-in` ligaria o elemento como `Object?`,
que quase nenhum corpo de laço consegue usar sem `as`. O laço explícito sobre a
classe **concreta** do iterator preserva o tipo que ela declara — `Contagem`
declara `int get current` — e é isso que a mensagem oferece.

Pela mesma razão, `Iterable<T>` implementada pelo usuário **não** ganha os demais
membros do protocolo (`length`, `first`, `map`, `join`…). No SDK eles vêm de
`IterableMixin`/`IterableBase`, de `dart:collection`, que este subconjunto não
tem. Fornecê-los aqui exigiria o elemento, que o apagamento não preserva.

### `Iterator` como anotação de tipo

`Iterator` e `Iterator<T>` são aceitos em anotação. O tipo tem dois membros:
`bool moveNext()` e `Object? get current`. Recuperar o elemento concreto pede
`as`, como qualquer outro valor `Object?` do subconjunto.

Ler `.iterator` de uma coleção **interna** (`List`, `Set`, `Map.keys`) é recusado:

```
reading '.iterator' of a built-in collection is unsupported in this subset: use 'for (var x in ...)', which lowers to the JavaScript iteration protocol; a user class can still declare 'Iterator<T> get iterator' and implement Iterator itself
```

## `Exception` e `Error`

```dart
class FalhaDeRede implements Exception {
  final String mensagem;
  FalhaDeRede(this.mensagem);
  @override
  String toString() { return 'FalhaDeRede: $mensagem'; }
}

void main() {
  try {
    throw FalhaDeRede('tempo esgotado');
  } on FalhaDeRede catch (e) {
    print(e);                     // FalhaDeRede: tempo esgotado
  }
}
```

`implements Exception` é aceito e **não exige membro nenhum**, porque `Exception`
não declara nenhum no SDK. `throw` e `catch` já existiam no subconjunto; este
trabalho só removeu a recusa de `unknown interface` que bloqueava o idioma mais
comum de exceção em Dart — `class ClientException implements Exception` do
`package:http` é exatamente essa forma.

`Error` é **recusado**, e não por falta de trabalho:

```
Error is unsupported: Dart's Error declares 'StackTrace? get stackTrace', and this subset has no StackTrace type to satisfy it — 'implements Error' without that getter is a compile error in the SDK too; use 'implements Exception', which declares no members, and 'on YourType catch (e)' to recover it
```

A afirmação sobre o SDK foi conferida: `dart analyze` de uma classe concreta com
`implements Error` e sem o getter reporta `Missing concrete implementation of
'getter Error.stackTrace'`. Aceitar `implements Error` aqui aceitaria um programa
que o próprio Dart recusa.

### `catch (e)` liga `Object`, e `Object` não imprime

`catch (e)` sem `on` liga `e` como `Object`, e o subconjunto não tem
representação textual para `Object` — pela mesma decisão de OBJETO.md. Use
`on MinhaExcecao catch (e)`, que liga o tipo concreto e imprime pelo `toString`
declarado.

## `StringBuffer`

```dart
void main() {
  var b = StringBuffer();
  b.write('a');
  b.write(1);
  b.writeln('!');
  print(b.length);       // 4
  print(b.isNotEmpty);   // true
  print(b.toString());   // a1!\n
  b.clear();
  print(b.isEmpty);      // true
}
```

Membros reconhecidos: `write`, `writeln`, `writeCharCode`, `clear`, `length`,
`isEmpty`, `isNotEmpty`, `toString`.

`length` é o número de **unidades UTF-16 já escritas**, como no SDK, e não o
número de chamadas a `write` — por isso é somado na escrita, não contado depois.
O buffer acumula os pedaços num vetor e junta só no `toString`: concatenar a cada
`write` seria quadrático, e um buffer existe justamente para não ser.

`write` converte pelo mesmo `$dartforgeString` de `print` e da interpolação,
então vale a mesma regra: um valor sem representação textual definida é
**recusado**, e não vira `[object Object]`:

```
'StringBuffer.write' writes the value through its 'toString', and 'SemTexto' declares none: emitting the JavaScript "[object Object]" — or Dart's "Instance of 'Name'" — would be a plausible wrong answer; declare 'String toString()' on it
```

`StringBuffer('inicial')` com valor inicial é recusado:

```
'StringBuffer(...)' with an initial value is unsupported in this subset: write 'StringBuffer()' and call 'write' once
```

### Como `StringBuffer()` atravessa o pipeline

`StringBuffer` não é declarado em lugar nenhum, então `ExprKind::Construct`
— que exige um identificador de classe registrado pelo ligador — não serve. O
parser emite `StringBuffer()` como **invocação de um identificador**, forma que o
ligador atravessa sem tocar, e a análise semântica a reconhece como intrínseco.
Um local, parâmetro ou função de topo chamado `StringBuffer` tem precedência,
porque quem decide é a análise e não o parser — a mesma regra de `identical`.

## `Comparator<T>`

`Comparator<T>` é o `typedef int Comparator<T>(T a, T b)` do SDK: um **tipo de
função**, não um tipo nominal. Ele resolve para `int Function(T, T)` no parse, e
`Comparator` cru resolve para `int Function(Object?, Object?)`. Nenhuma máquina
nova foi necessária.

## Formas cruas de `List`, `Set` e `Iterable` (e por que `Map` é exceção)

`List`, `Set`, `Iterable` e `Future` sem argumento de tipo passaram a ser
aceitos. Em Dart o tipo cru é `List<dynamic>`, e `dynamic` está fora deste
subconjunto; a leitura adotada é `List<Object?>` (e `Iterable<Object?>`), que é
**mais restrita**: exige `as` onde o Dart dispensaria e não aceita nada que ele
recuse. São 153 ocorrências no corpus medido — 21 de `Iterable`, 32 de `List`,
41 de `Map`, 59 de `Set` — quase todas em anotação de tipo.

`Map` cru é recusado de propósito ("Only String Map keys are supported"): a
leitura seria `Map<Object?, Object?>`, mas a emissão só representa mapas de
chave `String`. Aceitar a anotação seria prometer o que a emissão não cumpre;
quando o mapa de chave geral chegar, ele precisa de representação e de
semântica de igualdade de chaves próprias, não só de relaxar a checagem.

Detalhe de implementação que quase virou regressão: a tabela sintética do
núcleo registra uma classe "Iterable" (e "Iterator", "Comparable"...), e o
teste de sombra de `check_shape_name` a confundia com declaração do programa —
toda anotação `Iterable x` falhava. A checagem isenta entradas sintéticas
(`library_id == usize::MAX`); uma classe do usuário com o mesmo nome continua
recusada, como antes.

## Membros de `String`

| Membro | Resultado | Decisão que não é tradução direta |
| --- | --- | --- |
| `length`, `isEmpty`, `isNotEmpty` | `int`, `bool`, `bool` | comprimento em unidades UTF-16, como `String.length` do JavaScript |
| `substring(int, [int?])` | `String` | índice fora de faixa **lança** `RangeError`; `slice` grampearia em silêncio |
| `indexOf(String, [int])`, `lastIndexOf(String)` | `int` | `start` fora de faixa lança, como no oráculo |
| `contains(String, [int])`, `startsWith(String, [int])`, `endsWith(String)` | `bool` | o parâmetro é `String`, não `Pattern`: não há `RegExp` no subconjunto |
| `toLowerCase`, `toUpperCase` | `String` | independentes de locale, como no Dart (não `toLocaleLowerCase`) |
| `trim`, `trimLeft`, `trimRight` | `String` | `trim` do JavaScript remove o mesmo conjunto, BOM incluído — conferido em `'\u{FEFF}a'.trim()` |
| `split(String)` | `List<String>` | `''.split('')` é `[]` e `'ab'.split('')` é `[a, b]` nos dois |
| `replaceAll(String, String)` | `String` | `'ab'.replaceAll('', 'x')` é `xaxbx` nos dois |
| `replaceFirst(String, String, [int])` | `String` | |
| `replaceRange(int, int?, String)` | `String` | |
| `padLeft(int, [String])`, `padRight(int, [String])` | `String` | **divergem de `padStart`/`padEnd`**: ver abaixo |
| `codeUnitAt(int)` | `int` | fora de faixa lança; `charCodeAt` devolveria `NaN` |
| `codeUnits` | `List<int>` | |
| `compareTo(String)` | `int` | ordem de unidades UTF-16, a mesma de `<` do JavaScript |
| `toString` | `String` | identidade |
| `hashCode` | `int` | valor próprio; ver a seção de `hashCode` |

### `padLeft` não é `padStart`

```dart
print('7'.padLeft(5, 'ab'));  // abababab7
print('7'.padRight(5, 'ab')); // 7abababab
print('7'.padLeft(1, 'ab'));  // 7
```

O Dart repete a cadeia de preenchimento **`delta` vezes**, onde `delta` é
`n - length`; `padStart` do JavaScript repetiria até o comprimento pedido e
truncaria. A emissão segue o Dart.

## Membros de `int`, `double` e `num`

`int` é `Number` do JavaScript neste alvo, e a política de inteiro de
[COLECOES-OPERADORES.md](COLECOES-OPERADORES.md) — 32 bits para operações de bits
— vale aqui sem mudança: `bitLength` e `toRadixString` usam a mesma faixa, e a VM
do Dart, com 64 bits, diverge fora dela exatamente como já divergia lá.

| Membro | `int` | `double` | `num` |
| --- | --- | --- | --- |
| `abs` | `int` | `double` | `num` |
| `sign` | `int` | `double` | `num` |
| `round`, `floor`, `ceil`, `truncate` | `int` (identidade) | `int` | `int` |
| `roundToDouble`, `floorToDouble`, `ceilToDouble`, `truncateToDouble` | — | `double` | — |
| `toInt`, `toDouble` | `int`, `double` | `int`, `double` | `int`, `double` |
| `compareTo(num)` | `int` | `int` | `int` |
| `isNegative`, `isNaN`, `isFinite` | `bool` | `bool` | `bool` |
| `isInfinite` | — | `bool` | `bool` |
| `isEven`, `isOdd`, `bitLength` | `bool`, `bool`, `int` | — | — |
| `gcd(int)`, `toRadixString(int)` | `int`, `String` | — | — |
| `toStringAsFixed(int)` | — | `String` | — |
| `toString` | `String` | `String` | **recusado** |
| `hashCode` | `int` | `int` | `int` |

### Quatro divergências entre Dart e JavaScript, todas corrigidas na emissão

**1. `round` arredonda meio para longe de zero.**

```dart
print((-2.5).round());          // -3    (Math.round(-2.5) é -2)
print((2.5).round());           //  3
print((-2.5).roundToDouble());  // -3.0
```

A emissão espelha em torno de zero: `x < 0 ? -Math.round(-x) : Math.round(x)`.

**2. `round`, `floor`, `ceil`, `truncate` e `toInt` lançam sobre `NaN` e infinito.**

```
Unsupported operation: Infinity or NaN toInt
```

`Math.floor(NaN)` devolveria `NaN`, que não é um `int`.

**3. `(-0.0).isNegative` é `true`.**

`-0 < 0` é `false` no JavaScript. A emissão é `x < 0 || Object.is(x, -0)`.

**4. `compareTo` tem regras próprias para `NaN` e para `-0.0`.**

```dart
print((0.0).compareTo(-0.0));           //  1
print(double.nan.compareTo(double.nan)); //  0
print(double.nan.compareTo(1.0));        //  1
print((1.0).compareTo(double.nan));      // -1
```

`NaN` é maior que tudo e igual a si mesmo, e `0.0` é maior que `-0.0`, que `===`
não distingue. O auxiliar reproduz as quatro linhas.

Também `(-0.0).toStringAsFixed(1)` é `-0.0` no oráculo e `toFixed` devolveria
`0.0`; a emissão recoloca o sinal.

### `num.toString()` é recusado

```
'num.toString()' is unsupported: the JavaScript Number erasure cannot tell 1 from 1.0, and the oracle prints '1' for the int and '1.0' for the double; call 'toInt().toString()' or 'toDouble().toString()' to state which text you mean
```

No oráculo, `(1 as num).toString()` é `"1"` e `(1.0 as num).toString()` é
`"1.0"`. As duas são o mesmo `Number` no JavaScript, então **nenhuma emissão
acerta as duas**. Recusar é a única resposta que não inventa texto. `int` e
`double` têm `toString` e nenhum dos dois é ambíguo.

`print(x)` com `x` de tipo estático `num` continua aceito, com a divergência que
já estava documentada para doubles de valor inteiro; este documento não a muda,
só registra que `toString()` — cujo resultado o programa pode comparar e
concatenar — é o caso em que ela deixa de ser tolerável.

## Membros de `List`, `Set` e `Iterable`

Já existiam: `length`, `isEmpty`, `isNotEmpty`, `first`, `last`, `add`,
`contains`, `toList`, `where`, `map`, `any`, `forEach`.

Acrescentados:

| Membro | Onde | Resultado |
| --- | --- | --- |
| `single` | Iterable | `E`; lança `Bad state: No element` / `Too many elements` |
| `join([String])` | Iterable | `String` |
| `toString()` | Iterable | `String` |
| `elementAt(int)` | Iterable | `E` |
| `skip(int)`, `take(int)` | Iterable | `Iterable<E>`, preguiçosos |
| `toSet()` | Iterable | `Set<E>` |
| `every(test)` | Iterable | `bool` |
| `firstWhere(test)` | Iterable | `E` |
| `reduce(combine)` | Iterable | `E` |
| `fold(inicial, combine)` | Iterable | tipo do inicial |
| `expand(f)` | Iterable | `Iterable<R>` |
| `reversed` | **List** | `Iterable<E>` |
| `indexOf(E, [int])` | **List** | `int` |
| `sublist(int, [int?])` | **List** | `List<E>` |
| `insert(int, E)`, `removeAt(int)`, `clear()` | **List** | `void`, `E`, `void` |
| `sort([comparador])` | **List** | `void` |
| `addAll(Iterable<E>)` | List, Set | `void` |
| `remove(Object?)` | List, Set | `bool` |
| `clear()` | List, Set | `void` |

### O que é de `List` e não de `Iterable`

`reversed`, `indexOf`, `sublist`, `insert`, `removeAt` e `sort` só existem em
`List` no SDK, e aqui também: aceitá-los em `Iterable` aceitaria código que o
Dart recusa. A mensagem diz o que fazer:

```
'Iterable<int>' is a lazy sequence and declares no 'insert': materialize it with '.toList()' first
```

```
'Set<int>' declares no 'sort': only List is ordered in place; call '.toList()' first
```

### `fold` toma o tipo do acumulador do valor inicial

O Dart infere `T` de `fold<T>` do argumento de tipo escrito ou do valor inicial.
Este subconjunto não tem métodos genéricos, então **o valor inicial decide**:

```dart
print(<int>[3, 1, 2].fold(0, (a, b) => a + b));   // 6
```

Um inicial sem tipo próprio é recusado com a razão, em vez de virar `dynamic`.

### `join` e `toString` recusam o que não tem texto

```
'join' on 'List<SemTexto>' has no defined text in this subset: some element type declares no 'String toString()', and emitting the JavaScript "[object Object]" — or Dart's "Instance of 'Name'" — would be a plausible wrong answer; declare 'String toString()' on it, or map the elements to String first
```

É a mesma regra de `print` e da interpolação, aplicada ao ponto onde o texto é
realmente produzido. A verificação é recursiva: `List<List<SemTexto>>` também é
recusada.

### `remove` e `indexOf` comparam por identidade

Pela política já registrada em [OBJETO.md](OBJETO.md): `Map` e `Set` deste
subconjunto comparam chaves por identidade e não por `operator ==`. `List.remove`
e `List.indexOf` seguem a mesma regra, para que as três estruturas não discordem
entre si. Duas instâncias iguais por `==` continuam sendo dois elementos
distintos.

### `cast` e `whereType` são recusados

```
'cast' is unsupported: it needs a written type argument on a method, and generic methods are outside this subset; declare the target collection with its element type and copy into it
```

## Membros de `Map`

Antes deste trabalho só `length` era reconhecido, e **nenhuma chamada de método**
em `Map` existia: o mapa caía na resolução nominal e terminava em classe
desconhecida. `containsKey` é o 8º membro mais usado no corpus medido.

| Membro | Resultado |
| --- | --- |
| `length`, `isEmpty`, `isNotEmpty` | `int`, `bool`, `bool` |
| `keys`, `values` | `Iterable<K>`, `Iterable<V>` |
| `containsKey(Object?)`, `containsValue(Object?)` | `bool` |
| `remove(Object?)` | `V?` |
| `putIfAbsent(K, V Function())` | `V` |
| `addAll(Map<K, V>)`, `clear()` | `void` |
| `forEach(void Function(K, V))` | `void` |
| `toString()` | `String` |

**A ordem de `keys`, `values`, `forEach` e `toString` é a de inserção**, como o
`LinkedHashMap` que o Dart usa para `{}`. O `Map` do JavaScript preserva a mesma
ordem, então a emissão é direta — mas a propriedade está afirmada por teste, e
não deduzida da implementação.

`remove` devolve o valor removido ou `null` quando a chave não existia, então o
resultado é **anulável** mesmo num mapa de valores não anuláveis.

`putIfAbsent` só chama a fábrica quando a chave falta, e a inserção mantém a
ordem — as duas propriedades estão afirmadas por teste.

`entries` é recusado, porque o resultado é `MapEntry<K, V>`:

```
'entries' is unsupported because its result type is MapEntry<K, V>, which this subset does not model; iterate 'keys' and index the map, or call 'forEach((k, v) { ... })'
```

## `List.unmodifiable` e as fábricas nomeadas de `List`

`List.filled`, `List.generate`, `List.from` e `List.unmodifiable` **não** estão
implementados. `List` não é uma classe declarada neste subconjunto, e uma fábrica
nomeada sobre um tipo estrutural precisaria de um caminho próprio no parser, no
ligador e na emissão.

`List.unmodifiable` merece nota separada porque o contrato dele **não é só uma
cópia**: a lista devolvida recusa toda mutação em execução
(`Unsupported operation: Cannot add to an unmodifiable list`), e é justamente
esse contrato que o dart2js 3.6.2 implementa de forma defeituosa dentro de uma
máquina de estados `async` — o bug real reproduzido em
`tests/dart2js/unmodifiable-async/README.md`, onde o compilador oficial emite
`result.$flags = 3` num caminho de junção em que `result` nunca foi atribuído.

A consequência para este trabalho é uma escolha e não um esquecimento:
implementar `List.unmodifiable` sem o marcador de imutabilidade em execução
entregaria uma cópia mutável com o nome de uma lista imutável — exatamente a
classe de erro silencioso que este subconjunto recusa. Enquanto o marcador não
existir, a fábrica não existe.

## Desempenho

A resolução de membro fica no caminho quente da análise semântica, e
`docs/DESEMPENHO.md` mede **alocações por compilação** porque o tempo tem ruído.
A estrutura do validador é copiada a cada ramificação de fluxo, então nada que
aloque pode entrar nela.

As tabelas de membro de `String`, `int`, `double`, `num`, `bool` e `StringBuffer`
são `&'static [Membro]` ordenados por nome, consultados por **busca binária**:
zero alocação, zero construção por chamada e nada novo para copiar por
ramificação. Um `HashMap` — mesmo um único, construído na entrada da análise —
pagaria alocação em todo programa, inclusive nos que não chamam membro algum. A
ordenação é afirmada por teste, porque uma busca binária sobre lista desordenada
erraria em silêncio.

Os membros de coleção continuam resolvidos por `match` sobre o nome, que o
compilador Rust reduz a comparações diretas, sem tabela.

As cinco entradas sintéticas das interfaces de `dart:core` entram na tabela de
classes **uma vez por compilação**; todos os seus campos são `HashMap` e `Vec`
vazios, que não alocam, e a tabela é compartilhada por `Rc`.

No JavaScript emitido, nenhum auxiliar entra por existir: o emissor registra os
que realmente usou e emite só esses, em ordem alfabética estável. Um programa que
não chama membro de biblioteca sai byte a byte igual ao de antes.

Os números medidos estão em [DESEMPENHO.md](../DESEMPENHO.md).

## O que a aferição de corpus consegue e não consegue medir

`docs/CORPUS-REAL.md` tem dois modos, e **nenhum dos dois mede cobertura de
membro**:

* **modo unidade** analisa cada arquivo *só até a sintaxe*. Ele mede nomes de
  tipo — e é por isso que `Iterable` cru, `Comparable`, `Iterator`, `Exception`,
  `Comparator` e `StringBuffer` aparecem lá — mas nunca chega a resolver
  `s.substring(1)`.
* **modo grafo** roda o front-end completo, mas todos os pontos de entrada do
  corpus morrem antes na resolução de biblioteca: `package:test`, `dart:io`,
  `dart:convert`, `dart:math`, `dart:collection`, `dart:typed_data`. Na medição de
  linha de base, de 113 entradas, **106 falharam por biblioteca externa** e o
  total de lacunas de linguagem foi **3**.

Ou seja: o trabalho de membros é necessário — Dart de produção usa centenas — mas
o número que ele move não é o do aferidor atual. O que o aferidor mede deste
trabalho são os **nomes de tipo**, e a variação está registrada no relatório da
rodada.

## Limites que permanecem

* `Error`, `StackTrace`, `MapEntry`, `DateTime`, `RegExp`, `Pattern`, `Uri`,
  `Symbol`, `Type`, `BigInt`, `Runes` e `Stopwatch` são recusados, cada um com a
  razão e a alternativa no próprio diagnóstico.
* `num.toString()`, `hashCode` de coleção, `entries`, `cast`, `whereType`,
  `runtimeType` e `noSuchMethod`: recusados, com a razão acima.
* `for-in` sobre `Iterable` do usuário e o restante do protocolo `Iterable`:
  bloqueados pelo apagamento de argumentos de tipo de classe.
* Fábricas nomeadas de `List` (`filled`, `generate`, `from`, `unmodifiable`).
* `Iterable` do usuário não recebe os membros do protocolo.
* O backend LLVM AOT não tem lowering para nada deste documento: uma classe que
  declare `implements Exception` compila no alvo JavaScript e não no nativo.
