# Parâmetros nomeados, opcionais e valores padrão

Este documento fixa o contrato dos parâmetros não posicionais no DartForge: o
que o parser aceita, o que a análise semântica exige, como o JavaScript é
emitido e quais limites permanecem, com a mensagem exata de cada diagnóstico.

O oráculo de comportamento é o Dart 3.6.2 instalado nesta máquina (`dart run`).
A única exceção é o parâmetro nomeado privado (`this._x`), recurso de Dart 3.12
que o SDK local rejeita: para ele **não há oráculo local** e o contrato abaixo
segue a especificação da linguagem.

## O que é aceito

| Forma | Exemplo |
| --- | --- |
| Nomeado opcional | `void f({int? a})` |
| Nomeado com padrão | `void f({int a = 1})` |
| Nomeado obrigatório | `void f({required int a})` |
| Posicional opcional com padrão | `void f(int a, [int b = 2])` |
| Posicional opcional anulável | `void f(int a, [int? c])` |
| Initializing formal nomeado | `C({required this.host, this.porta = 80})` |
| Nomeado privado (Dart 3.12) | `C({required this._apiKey})`, chamado por `apiKey:` |

Vale para funções de topo, métodos de instância, métodos abstratos, construtores
generativos (inclusive o construtor sem nome de uma classe com herança) e
fábricas nomeadas.

```dart
class Cliente {
  final String host;
  final int porta;
  Cliente({required this.host, this.porta = 80});
  int descrever({int extra = 1}) => porta + extra;
}

void main() {
  print(Cliente(host: 'a').descrever());
  print(Cliente(porta: 8080, host: 'b').descrever(extra: 2));
}
```

## Regras da declaração

1. A assinatura aceita **um único** grupo opcional: `[...]` **ou** `{...}`, sempre
   depois dos posicionais obrigatórios. Os dois grupos juntos são rejeitados,
   como em Dart.
2. Um parâmetro opcional — posicional ou nomeado — **sem valor padrão** exige
   tipo anulável. Sem padrão e sem anulabilidade o valor não teria como existir.
3. `required` e valor padrão são mutuamente exclusivos.
4. Um valor padrão só é aceito dentro de um grupo opcional; `void f(int a = 1)`
   é erro.
5. O valor padrão passa pelo avaliador de constantes de
   `crates/semantic/src/constants.rs` e é tipado contra o tipo do parâmetro.
   Depois disso ele ainda precisa ser um **literal escalar**: `int`, `String`,
   `bool`, `null` ou a negação de um literal inteiro.

   O limite existe por uma razão concreta: o linker reescreve os spans ao
   combinar bibliotecas, mas **não percorre a expressão de um valor padrão**.
   Um padrão cujo valor dependesse da tabela de constantes indexada por span
   poderia colidir com o span já remapeado de outra unidade. Restringir a
   literais torna a emissão do padrão independente de qualquer tabela. Por isso
   `{int a = 1 + 2}` e `{List<int> a = const [1]}` são rejeitados.
6. Um parâmetro nomeado só pode ter nome privado quando é um initializing
   formal (`this._x`). Nesse caso o campo continua `_x` e o rótulo externo é
   `x`. Dois nomeados com o mesmo rótulo externo — por exemplo `this._x` e
   `int x` — colidem e são rejeitados.

## Regras da chamada

1. Os nomeados podem aparecer em **qualquer ordem entre si**.
2. **Limite deliberado**: todos os nomeados vêm depois de todos os posicionais.
   Dart permite intercalar; o DartForge rejeita com diagnóstico explícito. A
   simplificação preserva a ordem de avaliação sem introduzir temporários: o
   objeto de nomeados é construído no fim da lista de argumentos e o JavaScript
   avalia suas propriedades na ordem escrita.
3. A **ordem de avaliação é a ordem escrita**, incluindo os nomeados entre si.
   `soma(f(1), c: f(2), b: f(3))` avalia `f(1)`, `f(2)` e `f(3)` nessa ordem,
   exatamente como o Dart 3.6.2 local.
4. Rótulo repetido, rótulo desconhecido e obrigatório ausente são erros, cada um
   com mensagem própria.
5. Os tipos dos argumentos continuam verificados como antes; nenhuma coerção
   nova foi introduzida.

## Sobrescritas

Uma sobrescrita precisa aceitar toda chamada válida para o contrato herdado:

- não pode exigir mais posicionais do que a base;
- não pode aceitar menos posicionais do que a base;
- não pode **remover** um nomeado declarado na base;
- não pode **acrescentar** um nomeado obrigatório que a base não exigia.

As mesmas regras valem para contratos de interface.

## Emissão JavaScript

Os posicionais saem na ordem declarada. Um posicional opcional sem padrão
explícito recebe `= null`, porque Dart não possui `undefined`.

Os nomeados viram **um único objeto no fim da lista**, desestruturado no próprio
cabeçalho, com padrão `{}` para que a chamada sem nomeado algum continue válida:

```dart
void f(int a, {required int b, String c = 'x', int? d}) {}
```

```js
function $df_f($df_a, {$dfn$b: $df_b, $dfn$c: $df_c = "x", $dfn$d: $df_d = null} = {}) {}
```

- A **chave** é `$dfn$` mais o **rótulo externo** (sem sublinhado). O prefixo
  `$dfn$` é reservado e fica fora do espaço `$df_` dos identificadores do
  usuário, de modo que uma chave nunca colide com um nome Dart nem com um
  auxiliar do runtime.
- A **ligação** é o nome interno já prefixado por `$df_`. Para `this._apiKey` a
  chave é `$dfn$apiKey` e a ligação é `$df__apiKey`.

Na chamada:

```dart
f(1, c: 'y', b: 2);
```

```js
$df_f(1, {$dfn$c: "y", $dfn$b: 2});
```

Construtores seguem o mesmo esquema dentro de `$dartforgeNew{id}`, ligando cada
nomeado ao `$dartforgeArgument{índice}` da posição declarada.

## Limites que permanecem

Cada limite abaixo tem diagnóstico explícito, com a mensagem exata:

| Situação | Mensagem |
| --- | --- |
| Closure com `[`, `{`, `required` ou `=` | `closures support only required positional parameters` |
| Dois grupos opcionais na mesma assinatura | `a signature accepts a single optional or named group` |
| Posicional escrito depois de um nomeado (parser) | `positional arguments must precede named arguments` |
| Posicional depois de nomeado (AST construída fora do parser) | `Positional arguments must precede named arguments` |
| Método de extension com opcional ou nomeado | `Extension methods support only required positional parameters` |
| Função genérica com opcional ou nomeado | `Generic functions support only required positional parameters` |
| Função `@Native` com opcional ou nomeado | `@Native functions support only required positional parameters` |
| Backend AOT com opcional ou nomeado | `LLVM AOT ainda não suporta parâmetros opcionais ou nomeados no backend nativo` |
| Backend AOT com argumento nomeado | `LLVM AOT ainda não suporta argumentos nomeados` |
| Padrão fora de grupo opcional | `Only optional or named parameters accept a default value` |
| Padrão em `required` | `A required named parameter cannot have a default value` |
| Opcional posicional sem padrão e não anulável | `Optional positional parameter requires a default value or a nullable type` |
| Opcional nomeado sem padrão e não anulável | `Optional named parameter requires a default value or a nullable type` |
| Padrão constante mas não literal | `A default value must be a literal scalar constant` |
| Padrão não constante | `Const expression: calls, getters and this expression are unsupported` |
| Tear-off de função com opcional ou nomeado | `Tear-offs of functions with optional or named parameters are unsupported` |
| Nome privado em nomeado que não é initializing formal | `A named parameter accepts a private name only as an initializing formal` |
| Dois nomeados com o mesmo rótulo externo | `Duplicate named parameter 'x'` |
| Rótulo repetido na chamada | `Duplicate named argument 'x'` |
| Rótulo desconhecido na chamada | `Unknown named argument 'x'` |
| Obrigatório ausente na chamada | `Missing required named argument 'x'` |
| Sobrescrita remove um nomeado obrigatório | `Override drops required named parameter 'x'` |
| Sobrescrita remove um nomeado opcional | `Override drops named parameter 'x'` |
| Sobrescrita acrescenta um nomeado obrigatório | `Override adds required named parameter 'x'` |
| `NamedArgument` fora de lista de argumentos | `A named argument is valid only in an argument list` |

Além desses, `super()` implícito continua exigindo uma superclasse **sem
parâmetro algum** no construtor, incluindo nomeados: a cadeia de inicialização
emitida não repassa argumentos para a base.

## Desempenho

A representação foi escolhida para não encarecer o caminho comum — a função com
apenas posicionais obrigatórios:

- `Parameter::kind` é um `enum` `Copy` de um byte e `Parameter::default` é um
  `Option<Box<Expr>>`, que não aloca quando ausente.
- `Signature` (em `crates/semantic/src/lib.rs`) ganhou `required_positional:
  usize` e `named: Vec<NamedParameter>`. A lista fica **ordenada por rótulo** e é
  consultada por busca binária; uma assinatura nunca carrega `HashMap`. Sem
  nomeados, o `Vec` fica vazio e não aloca, de modo que clonar a assinatura
  continua custando o que custava.
- `signature_parts` faz **uma varredura** para achar onde começa o grupo
  nomeado e depois fatia a lista. Isso importa: `filter(...).collect()` perde o
  tamanho exato e faria o `Vec` crescer por dobras, trocando uma alocação por
  várias em assinaturas longas. Fatiar preserva o `ExactSizeIterator` e a
  alocação única que a assinatura já fazia.
- A verificação de chamada usa `split_arguments`, que devolve dois **slices
  emprestados** da lista escrita: nenhum `Vec` novo por chamada.
- Duplicatas e ausências são detectadas varrendo listas de poucos elementos, sem
  construir tabela alguma.

Medição com `cargo bench -p dartforge-compiler --bench incremental`, mesma
máquina, corpus de 25 unidades da metodologia de [DESEMPENHO.md](../DESEMPENHO.md):

| Métrica (compilação fria) | Antes | Depois |
| --- | --- | --- |
| mediana | 5,20 ms | 5,06 ms |
| p95 | 6,15 ms | 5,83 ms |
| **alocações por compilação** | **16.716** | **16.716** |

As alocações são **idênticas nos sete cenários** do benchmark, que é o número
que não depende da carga da máquina. O corpus não usa parâmetros nomeados, e é
exatamente esse o ponto: o caminho comum não paga nada pelo recurso novo.

O tempo, nesta máquina e neste momento, tem ruído grande: três execuções
consecutivas do **mesmo binário** deram medianas entre 5,06 ms e 5,63 ms, e o
`load_ns` — fase de syscalls que este trabalho não toca — variou de 1,29 ms a
1,76 ms entre elas. A linha da tabela usa a execução mais silenciosa de cada
lado, identificada pelo `load_ns` equivalente. Na mesma execução, a fase
`analyze_ns`, onde está quase toda a mudança, ficou em 1,12 ms contra 1,13 ms
da linha de base.

## Testes

`crates/compiler/tests/parametros.rs` cobre cada regra acima, com mensagem e
span exatos, e inclui um teste de execução em Node marcado
`#[ignore = "requer Node.js no PATH"]` cuja saída foi conferida contra
`dart run` do SDK 3.6.2 local.
