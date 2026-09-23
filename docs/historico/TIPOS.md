# Tipos e declarações: genéricos escritos, `late` e `typedef`

Este documento registra o contrato e os limites de três construções que a
aferição contra código real ([CORPUS-REAL.md](../CORPUS-REAL.md)) apontou como as
mais frequentes depois das anotações: tipos genéricos escritos (51% dos
arquivos), `late` (28%), `typedef` (6%) e classes genéricas (4%).

Todo comportamento afirmado aqui foi conferido com dois oráculos — `dart run` no
SDK **3.6.2** e no **3.13.4** — e está fixado em `crates/compiler/tests/tipos.rs`.
Onde o DartForge divergir do Dart, a divergência está escrita, não omitida.

## 1. Tipos genéricos escritos

A arena de formas estruturais (`Program.types`, `TypeShape` em
`crates/syntax`) já representava tudo o que é preciso: `List<T>`, `Set<T>`,
`Iterable<T>`, `Future<T>`, `Map<K,V>`, `Nullable(T)` e tipos de função. A
escrita dessas anotações é aceita em toda posição:

| Forma | Exemplo |
| --- | --- |
| coleção simples | `List<int> a = [1];` |
| mapa | `Map<String, int> m = {};` |
| aninhamento arbitrário | `Map<String, List<int>>`, `List<List<List<int>>>` |
| anulável | `List<int>?`, `Map<String, List<int>?>` |
| retorno e parâmetro | `Map<String, List<int>> f(List<int> a)` |
| campo | `class C { Map<String, List<int>> v = {}; }` |
| dentro de `Future` | `Future<List<int>> f() async => [1];` |

O aninhamento é limitado por `MAX_DEPTH`; acima dele o diagnóstico é
`type nesting limit exceeded`, com o span do tipo que estourou.

### `T?` é idempotente

O bound implícito de um parâmetro de classe é `Object?`. Como o parser resolve
`T` para o bound (ver §2), `T?` pedia a nulabilidade de um tipo **já anulável** e
era recusado com `type cannot be nullable`. Isso reprovava
`class A<T> { T? v; }`, a forma mais comum de campo genérico anulável. Agora
`T?` com `T` já anulável é `T?`, como em Dart.

## 2. Classes genéricas: aceitação com apagamento (erasure)

Substituição completa de tipos na herança não existe. O que existe é **erasure
declarado**: um parâmetro de classe `T` resolve, no parse, para o seu bound.

```dart
class Caixa<T extends int> { T v; Caixa(this.v); }   // T vale int
class A<T> { T? v; }                                  // T vale Object?
```

São aceitos os cabeçalhos `class C<T>`, `class C<T extends Base>`,
`extends Base<T>`, `implements Interface<T>` e `with Mixin<T>`, e a construção
com argumentos escritos, `Caixa<int>(5)`.

### O que o erasure custa, exatamente

`Caixa<int>` e `Caixa<String>` compartilham **uma** representação, então `is`
sobre um tipo parametrizado só distingue a classe crua. A divergência é medida,
não hipotética:

```dart
class Caixa<T extends num> { T v; Caixa(this.v); }
void main() {
  var s = Caixa<double>(1.5);
  print(s is Caixa<int>);      // Dart: false — DartForge: true
  print(s is Caixa<double>);   // Dart: true  — DartForge: true
}
```

Programas que dependem de genéricos reificados para decidir fluxo **não** são
cobertos por este subconjunto, e um `is` sobre classe parametrizada não deve ser
usado como guarda. O alvo reificado está em
[GENERICS-REIFIED-REFERENCIAS.md](GENERICS-REIFIED-REFERENCIAS.md).

Um segundo custo aparece em `print`: com `class Caixa<T>` sem bound, `T` vale
`Object?`, e imprimir `Object?` exige a semântica de `toString` que o
subconjunto não emite. A mensagem é
`Printing objects or function values requires unsupported toString semantics`.
Escrever o bound (`class Caixa<T extends int>`) resolve, porque aí `T` vale `int`.

### O que **não** é aceito em silêncio

Erasure apagaria, junto com os argumentos, os erros de programa que eles
carregam. Os três são recusados antes do apagamento:

| Programa | Diagnóstico |
| --- | --- |
| `Caixa<String>(1)` com `T extends int` | `Type mismatch: expected Int, found String` |
| `Caixa<int, int>(1)` | `Incorrect class type argument count` |
| `C<int>(1)` com `class C` sem genéricos | `Class 'C' declares no type parameters` |

`C<int>(...)` é sintaticamente indistinguível de uma chamada de função genérica;
quem decide é a análise semântica, que tem os bounds declarados. O parser não os
tem, e por isso não é ele que valida.

## 3. `late`

### O contrato do Dart, medido

O oráculo separa dois regimes, e a diferença não é cosmética:

| Caso | Dart 3.6.2 e 3.13.4 |
| --- | --- |
| local `late`, leitura **definitivamente** não escrita | **erro de compilação** |
| local `late`, leitura possivelmente não escrita | `LateInitializationError: Local 'x' has not been initialized.` |
| local `late final`, escrita **definitivamente** repetida | **erro de compilação** |
| local `late final`, escrita possivelmente repetida | `LateInitializationError: Local 'x' has already been initialized.` |
| campo ou variável de topo, leitura antes da escrita | `LateInitializationError: Field 'x' has not been initialized.` |
| campo ou variável de topo `late final`, segunda escrita | `LateInitializationError: Field 'x' has already been initialized.` |

Duas observações que só a medição dá: uma variável de **topo** diz `Field`, não
`Local`, na mensagem; e o lado direito de uma atribuição repetida é **avaliado
antes** do lançamento — `c.y = rhs(2)` executa `rhs` e só então lança.

### O que o DartForge emite

Uma declaração `late` sem inicializador nasce num **sentinela exclusivo**,
`const $dartforgeLate = Symbol("late")`. O sentinela é um `Symbol` e não `null`
nem `undefined` justamente porque o programa pode atribuir esses dois: só um
valor que o usuário não consegue produzir distingue "ainda não inicializado".

- **Leitura**: `$dartforgeLateRead(valor, tipo, nome)` lança se for o sentinela.
- **Escrita em `late final`**: `$dartforgeLateWrite(corrente, valor, …)` lança se
  já houver valor. O valor novo é avaliado antes da checagem, que é a ordem do
  oráculo.
- **Escrita em `receptor.campo`**: `$dartforgeLateSet(objeto, chave, valor, …)`.
  O receptor pode ter efeito colateral, então é avaliado **uma única vez** e
  passado como objeto, em vez de reavaliado para conferir o sentinela.

Um `late` **não-final** não paga checagem de escrita: aceita qualquer número de
atribuições, como em Dart.

`late final` sem inicializador aceita a **primeira** atribuição. Recusá-la — o
que este subconjunto fazia, com `Cannot assign to final variable` — negava a
única escrita que Dart permite.

### Divergência declarada: compilação contra execução

Os dois casos que o Dart resolve por análise de fluxo de atribuição definida —
leitura definitivamente não escrita e escrita definitivamente repetida em um
**local** — o DartForge relata em **execução**, não em compilação. A análise de
atribuição definida não está implementada.

O erro aparece, com a mesma mensagem, no mesmo ponto do programa; o que muda é
que o programa **roda até lá** em vez de ser recusado antes. Em
`late final int x; x = 1; x = 2;` isso significa que os efeitos anteriores à
segunda atribuição acontecem. Nenhum valor errado é produzido em silêncio: a
leitura de um `late` não inicializado nunca devolve `null`, como devolvia antes.

### Recusado: `late` com inicializador

`late T x = init;` é **recusado**, em local, campo e variável de topo:

```
late with an initializer is not supported: in Dart the initializer runs on the
first read and a write before that read cancels it; declare `late T name;` and
assign before reading
```

O span cobre a palavra `late`. O motivo é que essa forma é uma célula
preguiçosa, não uma declaração com valor: o inicializador roda na **primeira
leitura**, e uma escrita anterior a ela o cancela sem jamais executá-lo. Medido:

```dart
int f() { print('init'); return 1; }
class C { late int v = f(); }
void main() { var c = C(); c.v = 9; print(c.v); }   // imprime só 9
```

O subconjunto avaliava o inicializador na declaração, o que dá a ordem de
efeitos errada sem avisar. Recusar é a única alternativa honesta enquanto a
célula preguiçosa não for emitida. A medição justifica a ordem: nas 69
declarações `late` do corpus `pdf` 3.13.1, **nenhuma** tem inicializador.

### Ainda recusado

`late const` não existe em Dart (`late const is not supported`). `late` em
membro estático de classe cai na recusa geral de escrita a estático,
`static field assignments are not supported yet`. `late` em parâmetro não é
sintaxe válida em Dart e continua recusado no parse do tipo.

### Só o backend JavaScript emite a célula

O sentinela e as checagens são emissão do backend JavaScript. Os backends
**LLVM AOT** e os dois **JIT** não os emitem, e por isso recusam a declaração em
vez de tratá-la como `null`:

| Backend | Mensagem |
| --- | --- |
| LLVM AOT | `LLVM AOT ainda não suporta late` |
| Cranelift JIT | `Cranelift JIT ainda não suporta late` |
| asmjit JIT | `asmjit JIT ainda não suporta late` |

Sem essa recusa a declaração viraria um `null` nesses backends e a leitura antes
da escrita devolveria null em vez de lançar — o mesmo erro silencioso que o
sentinela existe para impedir, só que restrito a um backend, o que é pior porque
os dois discordariam sem aviso.

## 4. `typedef`

As duas formas resolvem, no parse, para o tipo subjacente — o apelido não
sobrevive à análise, e `is F` testa o tipo apontado:

| Forma | Exemplo |
| --- | --- |
| moderna | `typedef F = int Function(int);` |
| clássica | `typedef int G(int x);` |
| alvo genérico | `typedef M = Map<String, List<int>>;` |
| função com genéricos | `typedef F = List<int> Function(Map<String, int>);` |

## 5. Custo no caminho comum

As duas tabelas novas da análise (`late_reads` e `late_final_writes` em
`Resolution`) são `BTreeSet` vazios em todo programa que não declara `late`, e um
`BTreeSet` vazio não aloca. O mesmo vale para `generic_constructions`. Os campos
`is_late` acrescentados a `Binding`, `FieldInfo` e `StaticInfo` são `bool`, e
`generic_bounds` em `ClassInfo` é um `Vec` vazio numa classe sem genéricos —
o mesmo raciocínio que já valia para `setters`.

Os auxiliares de runtime de `late` só entram no JavaScript quando alguma
declaração `late` existe, pelo mesmo mecanismo de `$dartforgeNullAssert`. Um
programa sem `late` não contém a palavra `$dartforgeLate`, e isso é um teste.

Medição em `cargo bench -p dartforge-compiler --bench incremental`, com o corpus
de 25 unidades que não usa `late` nem genérico escrito. As alocações por
compilação são idênticas em **todos** os cenários:

| Cenário | Alocações antes | Alocações depois |
| --- | --- | --- |
| frio | 16.717 | 16.717 |
| sem edição (acerto de cache) | 124 | 124 |
| acerto em disco | 87 | 87 |
| edição de comentário | 15.815 | 15.815 |
| edição de corpo | 15.813 | 15.813 |
| edição de assinatura | 15.826 | 15.826 |
| edição de constante | 15.815 | 15.815 |
| edição de import | 16.574 | 16.574 |

A mediana da compilação fria foi 4,235 ms antes e 4,450 ms depois. A diferença é
ruído de máquina, e a rodada prova por que o projeto mede alocações: no mesmo
par de execuções o cenário de edição de comentário foi de 5,5 ms para 21,4 ms
sem **nenhuma** alocação a mais, porque havia outros processos compilando. O
contador de trabalho não se mexeu; o relógio, sim.

## Como repetir

```sh
cargo test -p dartforge-compiler --test tipos -- --include-ignored
cargo test -p dartforge-compiler --test corpus_real -- --ignored --nocapture
```

Os testes marcados `#[ignore = "requer Node.js no PATH"]` executam o JavaScript
emitido e comparam com a saída que os SDKs Dart produzem.
