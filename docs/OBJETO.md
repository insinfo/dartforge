# Protocolo `Object` e acessores de instância

Este documento descreve o que o subconjunto implementa de `operator ==`,
`hashCode`, `toString`, getters e setters de instância, e registra cada limite
com a mensagem exata do diagnóstico. Um recurso só entra aqui depois de ter
semântica emitida e testada; o que não está implementado é recusado com
diagnóstico próprio, nunca aceito em silêncio.

O oráculo é o Dart 3.6.2 (`dart`) e o Dart 3.13.4
(`D:/DartSDKs/3.13.4/dart-sdk/bin/dart.exe`), que produzem a mesma saída em
todos os programas desta página. Os testes estão em
`crates/compiler/tests/objeto.rs`.

## Getters e setters de instância

```dart
class Celsius {
  int _graus = 0;
  int get graus { return _graus; }
  set graus(int valor) { _graus = valor < -273 ? -273 : valor; }
  int get dobro { return _graus * 2; }
}
```

`c.graus` lê o getter, `c.graus = v` escreve pelo setter, e as duas formas
implícitas — `graus` e `graus = v` dentro da própria classe — resolvem para os
mesmos membros. O par getter/setter de mesmo nome é uma declaração única do
ponto de vista de quem usa a propriedade.

A emissão usa acessores nativos do JavaScript (`get x()` / `set x(v)`), então
leitura e escrita continuam sendo leitura e escrita de propriedade: nenhuma
forma de chamada nova aparece no código gerado e o despacho por herança é o do
próprio JavaScript.

### Representação na árvore

Um setter viaja em `Class::methods` marcado por `Function::is_getter` com
exatamente um parâmetro. A marca não é um campo novo por um motivo concreto: é
`Class::methods` que o linker percorre para renomear membros privados
(`set _x(int v)`) e remapear spans entre bibliotecas. Uma lista paralela na
`Class` ficaria invisível para ele e produziria nomes e spans errados em
programas com mais de uma biblioteca. `Function::is_setter()` e
`Function::is_property_getter()` leem a marca sem repetir a regra.

### Regras e diagnósticos

| Forma recusada | Mensagem |
| --- | --- |
| `int get x` e `int x` na mesma classe | `Duplicate class member` |
| dois setters de mesmo nome, ou setter com campo homônimo | `Duplicate setter or conflicting field` |
| setter e método comum de mesmo nome | `Duplicate class member: a setter cannot share a method name` |
| getter e setter com tipos diferentes | `A getter and its setter must declare the same type` |
| escrever em propriedade só com getter | `Cannot assign to 'x': the property declares a getter and no setter` |
| ler propriedade só com setter | `Cannot read 'x': the property declares a setter and no getter` |
| `int set x(int v)` | `a setter declares no return type or 'void'` |
| setter com aridade diferente de um posicional | `a setter takes exactly one required positional parameter` |
| `set x(int v);` sem corpo | `abstract setters and operator declarations are not supported yet` |
| setter `async` | `setters and operator == cannot be async in this subset` |
| getter de topo | `top-level getters are not supported yet` |
| setter de topo | `top-level setters are not supported yet` |
| getter estático | `static getters are not supported yet` |
| getter em extension | `extension getters are not supported yet` |

Setters continuam restritos a membros de instância de classe e de mixin. Fora
daí a declaração não chega a ser reconhecida como acessor: `set` não é um tipo,
então o parser recusa antes, com a mensagem de tipo inválido. Ampliar isso para
estáticos, topo e extensions é trabalho separado e não está feito.

Getter e setter precisam declarar **o mesmo tipo**. O Dart aceita a relação de
subtipo entre os dois; este subconjunto exige igualdade porque a promoção de
tipo na escrita usa a mesma tabela da leitura. Sobrescrever um setter herdado
segue a contravariância que a análise já aplica a métodos: o valor aceito pela
base precisa continuar sendo aceito pela derivada.

## `operator ==`

```dart
class Point {
  final int x;
  final int y;
  Point(this.x, this.y);
  bool operator ==(Object other) { return other is Point && other.x == x && other.y == y; }
}
```

A assinatura é fixa: `bool operator ==(Object other)`. Qualquer outra forma é
erro, não sobrecarga — `Object` não tem nenhuma.

O nome interno do membro é `==`, que nenhum identificador Dart pode ter. Isso
faz a declaração atravessar linker, otimizadores e emissão como um método
comum, com herança, contrato de override e variância já validados pelo caminho
que os métodos usam. No JavaScript o membro sai como `$df$eq`, fora do espaço
`$df_` reservado aos identificadores escritos pelo usuário.

### As duas regras do Dart que a emissão preserva

1. **Receptor `null` nunca chama o operador.** `e1 == e2` com `e1` nulo é
   verdadeiro somente contra `null`. `null == p` é `false` sem executar nada de
   `Point`.
2. **Quem decide é o lado esquerdo.** Com `e1` não nulo, a implementação
   escolhida é a de `e1`, inclusive quando `e2` é `null`: `p == null` chama
   `Point.==` com `null`, exatamente como no oráculo.

O auxiliar emitido é:

```js
function $dartforgeEquals(left, right) {
  if (left === null) { return right === null; }
  const operator = typeof left === 'object' ? left.$df$eq : undefined;
  return operator === undefined ? left === right : operator.call(left, right);
}
```

Com o runtime de records presente, `$dartforgeEquals` é apelido de
`$dartforgeEqual`, que já compara records e `Duration` e passou a consultar o
mesmo `$df$eq`. Assim um record que contém instâncias compara os campos com o
operador declarado, como o Dart faz.

### Custo: só quem declara paga

A análise só marca uma comparação quando **alguma** classe do programa declara
`operator ==` **e** um dos operandos tem tipo estático capaz de guardar uma
instância (`Class`, `Class?`, `Object`, `Object?`, parâmetro genérico cujo
limite seja um desses). `1 == 2`, `'a' == 'b'` e `x == null` com `x` escalar
continuam saindo como `===` do JavaScript, sem runtime algum. Um programa que
não declara operador nenhum emite exatamente o mesmo JavaScript de antes: o
conjunto `Resolution::equality_operators` fica vazio e não aloca.

A marcação é conservadora de propósito. Uma variável declarada como a base pode
conter a derivada que declara o operador, e `Object` pode conter qualquer
instância; errar para o lado do despacho preserva a semântica, enquanto errar
para o lado de `===` daria resposta errada em silêncio.

### `identical`

`identical(a, b)` é identidade de referência e **nunca** consulta o operador
declarado. Ela é emitida como `===` do JavaScript.

Até este trabalho o parser reescrevia `identical(a, b)` em `a == b`. A troca
era exata **porque** `operator ==` não existia: `==` sempre saía como `===`.
Com o operador declarável ela deixaria de ser exata — `identical` passaria a
chamar o operador do usuário, que é justamente o que `identical` não faz. A
reescrita foi removida e a chamada chega inteira à análise, que a trata como
intrínseco.

Operandos `double` e `num` são recusados:

```
identical on double or num operands is unsupported: the JavaScript Number erasure cannot distinguish the identities Dart distinguishes
```

É o único caso em que o apagamento para `Number` destrói a identidade que o
Dart preserva: `identical(1, 1.0)` é `false` no oráculo e `1 === 1.0` é `true`
no JavaScript, e `0.0` e `-0.0` são valores distintos lá e iguais por `===`
aqui. Os demais tipos — `int`, `String`, `bool`, `Null`, instâncias, coleções,
records — comparam por referência com a mesma resposta do oráculo.

Sombrear `identical` com um local ou membro é recusado, como já acontece com
`print`:

```
Invocation of local 'identical' is unsupported; it shadows the built-in function
```

Uma função de topo chamada `identical` continua válida e tem precedência: a
chamada passa a ser a dela.

## `hashCode`

```dart
class Point {
  int get hashCode { return x * 31 + y; }
}
```

A assinatura é fixa: `int get hashCode`. Qualquer outra forma produz

```
hashCode must be declared as 'int get hashCode'
```

**O contrato é seu.** Objetos iguais por `==` devem ter o mesmo `hashCode`;
objetos com o mesmo `hashCode` não precisam ser iguais. O compilador não
verifica essa relação — nenhum compilador Dart verifica — e quebrar o contrato
não produz erro, produz comportamento errado em qualquer estrutura que use
hash.

**`Map` e `Set` deste subconjunto não usam `hashCode`.** É preciso dizer isso
explicitamente porque é a diferença mais fácil de não perceber: `Map` é emitido
sobre o `Map` do JavaScript e `Set` sobre a mesma base, ambos comparando chaves
por *SameValueZero*, que é identidade para objetos. Duas instâncias iguais por
`==` e com o mesmo `hashCode` continuam sendo **duas chaves distintas**:

```dart
var m = <Point, int>{};
m[Point(1, 2)] = 1;
m[Point(1, 2)] = 2;   // no Dart substitui; aqui cria a segunda entrada
```

Declarar `hashCode` é útil para quem o chama diretamente e para preparar o
código que também roda na VM do Dart; ele **não** muda o comportamento das
coleções deste subconjunto. Uma tabela hash própria, que respeite `==` e
`hashCode`, é trabalho separado e não está feita.

A divergência está fixada em teste — `map_keys_compare_by_identity_not_by_the_declared_equality`
em `crates/compiler/tests/objeto.rs` — justamente para que ela não passe
despercebida: o programa acima imprime `1` nos dois SDKs do oráculo e `2` aqui.

## `toString`

```dart
class Point {
  String toString() { return 'Point($x, $y)'; }
}
```

A assinatura é fixa: `String toString()`. Qualquer outra forma produz

```
toString must be declared as 'String toString()'
```

`print(obj)` e a interpolação `'$obj'` passam a funcionar para qualquer
instância cuja classe — ou um ancestral — declare `toString`. A conversão é
feita por `$dartforgeString`, que consulta `$df_toString` **no objeto**, não no
tipo estático: uma derivada com `toString` próprio aparece mesmo através de uma
referência da base, como no oráculo. Instâncias dentro de coleções também usam
o `toString` declarado, porque o formatador de coleções faz a mesma consulta.

### A decisão sobre `Instance of 'Nome'`

Dart imprime `Instance of 'Nome'` para um objeto sem `toString` declarado.
**Este subconjunto recusa imprimir esse objeto** em vez de emitir aquele texto:

```
Class 'SemToString' declares no 'String toString()'; this subset does not emit "Instance of 'SemToString'"
```

A escolha tem um motivo mensurável. Emitir `Instance of 'Nome'` corretamente
exige que o texto seja escolhido pelo **objeto**, não pelo tipo estático — uma
referência de `Base` pode apontar para `Derivada`. Isso obriga toda classe do
programa a carregar seu nome Dart no JavaScript gerado, mesmo nos programas que
nunca imprimem instância alguma. Recusar mantém o custo em zero para quem não
usa o recurso e não produz nenhuma saída diferente do oráculo: o programa não
compila em vez de imprimir algo plausível e errado.

A consequência aparece com polimorfismo: se `Base` não declara `toString` e
`Derivada` declara, `print(baseRef)` é recusado mesmo que o objeto em tempo de
execução saiba se imprimir. Declarar `toString` também na base resolve, e o
despacho continua escolhendo o da derivada.

## `dynamic`

`dynamic` é recusado no parser, e a mensagem diz por quê:

```
dynamic is unsupported: this subset resolves every member statically, and dynamic dispatch would disable tree shaking and force the runtime to keep every same-named member; use an explicit type, Object or Object?
```

Não é falta de trabalho: é a escolha de não ter meio `dynamic`. Um único
receptor dinâmico desliga o tree shaking de membros — nenhum método de nome
igual pode ser removido, porque qualquer um deles pode ser o alvo — e obriga o
emissor a manter tabelas de despacho por nome. `Object` cobre o caso de
"qualquer valor" com verificação estática, e `is`/`as` recuperam o tipo
concreto onde ele importa.

## Limites que permanecem

- **Acessores abstratos.** `int get x;` continua aceito como assinatura
  abstrata; `set x(int v);` não: `abstract setters and operator declarations are not supported yet`.
- **Outros operadores.** Só `==` é declarável:
  `only 'operator ==' is supported; other operator declarations are not supported yet`.
  `<`, `+`, `[]` e os demais continuam fora.
- **Enums.** Valores de enum são objetos congelados sem cadeia de protótipo
  própria: `Enums cannot declare setters, operator ==, toString or hashCode in this subset`.
- **Extensions.** `Extensions of Object members are unsupported` continua
  valendo para `toString`, `hashCode`, `runtimeType` e `noSuchMethod`.
- **Backend nativo.** O layout LLVM indexa métodos por nome e ainda não tem
  entrada separada para acessor e operador:
  `setters de instância e operator == (lowering nativo pendente)`.
- **`runtimeType` e `noSuchMethod`.** Continuam recusados com
  `Member name requires unsupported Object or constructor semantics`.
- **`Map`/`Set` por `hashCode`.** Descrito acima; a igualdade de chaves é
  identidade, não `==`.

## Desempenho

As tabelas por classe da análise semântica são compartilhadas por `Rc` e
copiadas a cada ramificação de fluxo, então nada caro pode entrar nelas. Os
setters de uma classe ficam num `Vec` ordenado por nome, consultado por busca
binária; um `Vec` vazio não aloca, de modo que a classe sem acessor de escrita
— o caminho comum — não paga nada. `Validator::declares_equals` é um `bool`.
`Resolution::equality_operators` é um conjunto vazio em todo programa que não
declara operador.

Os números medidos com `cargo bench -p dartforge-compiler --bench incremental`
estão em [DESEMPENHO.md](DESEMPENHO.md).

## Contornos que agora podem ser revistos

`crates/macros` gera `igualA` e `descrever()` na macro de classe de dados
porque `operator ==` e `toString` não existiam. Com os dois implementados, a
macro pode passar a gerar os membros do protocolo `Object`. A mudança não foi
feita aqui e continua pendente.
