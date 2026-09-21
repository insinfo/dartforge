# Lacunas fechadas contra o corpus de produção

Este documento registra as construções que a aferição de
[CORPUS-REAL.md](CORPUS-REAL.md) encontrou no pacote `pdf` 3.13.1 — 179 arquivos
de Dart de produção — e que este subconjunto passou a aceitar, com o contrato de
cada uma e a mensagem exata dos limites que sobraram.

O critério de escolha não foi julgamento sobre o que "parece" importante: cada
item desta lista é um diagnóstico que o aferidor contou contra código real. O que
não entrou continua **recusado com diagnóstico próprio**, no formato da recusa de
`dynamic`: a mensagem diz a razão, o que a aceitação desativaria e o que escrever
no lugar. Tolerar em silêncio é pior do que recusar, porque promete honrar algo
que não é lido.

O oráculo de toda semântica afirmada aqui é o **Dart SDK 3.6.2** (`dart` no
PATH) e o **Dart SDK 3.13.4** (`D:/DartSDKs/3.13.4/dart-sdk/bin/dart.exe`), que
concordam em todos os casos deste arquivo. Os testes estão em
`crates/compiler/tests/lacunas.rs`.

## 1. `const` e `static const` sem anotação de tipo

`const kIndentSize = 2;` e `static const kMax = 255;` são aceitos: o tipo vem do
inicializador constante, decidido **sintaticamente** no parser.

A dedução cobre literais (`int`, `double`, `String`, `bool`), operadores
constantes sobre eles, literais de coleção com tipo de elemento escrito ou
uniforme, invocações de construtor e referências a `const` declarados antes. Um
`const` de topo já deduzido entra numa tabela, o que faz `const b = a + 3;`
funcionar.

O tipo deduzido é um tipo de verdade, não `dynamic`: `const k = 2;` seguido de
`String v = k;` continua sendo recusado por incompatibilidade de tipos.

Fora disso a declaração é recusada, com a razão e a saída:

```
a const declaration without a type annotation takes its type from the
initializer, and this initializer does not decide it syntactically: only
literals, constant operators over them, collection literals with a written or
uniform element type, constructor invocations and references to const
declarations written earlier are inferred here; write the type before the name
```

O span cobre o trecho do inicializador que não decidiu — `f()` em
`const k = f();`, não a declaração inteira. Deduzir daí exigiria resolver
chamadas antes da análise semântica, e o parser não tem essa informação.

Um `const` sem inicializador tem mensagem própria:
`a const declaration requires an initializer`.

## 2. Anotação em parâmetro

`void f(@Deprecated('…') String? x)` é aceito, em parâmetro posicional e
nomeado, de função de topo e de método. A lista tolerada é a mesma das
declarações: `Deprecated`/`deprecated` e os metadados sem efeito semântico de
`package:meta` (`@immutable`, `@protected`, `@mustCallSuper`,
`@visibleForTesting`, `@internal`, `@nonVirtual`, `@useResult`, …).

Uma exceção conhecida: `@required`, `@factory` e `@sealed` estão na lista de
metadados ignoráveis, mas **não** são alcançáveis, porque `required`, `factory` e
`sealed` são palavras reservadas do parser e a leitura do nome da anotação usa o
mesmo caminho de qualquer identificador. `@required` devolve
`expected a non-reserved identifier`. É o `required` de antes da null safety, e
não aparece no corpus medido; a alternativa é o `required` de parâmetro nomeado,
que é sintaxe e já é aceito.

Anotação que **não** é dessa lista é recusada no parâmetro:

```
this annotation targets a declaration, not a parameter; a parameter accepts
Deprecated and the semantics-free annotations of package:meta
```

E `@Native` continua exclusiva de função externa de topo:

```
Native annotations belong to a top-level external function, not to a parameter
```

### `@pragma` não é metadado sem semântica

`@pragma` dirige o compilador. Aceitá-la em silêncio prometeria honrar uma
diretiva que este compilador não lê, então ela mantém diagnóstico próprio:

```
@pragma directs the compiler and cannot be ignored; it is not implemented
```

Esta é a única correção deste documento que **reduz** o que era aceito: uma
rodada anterior acrescentou `pragma` à lista de metadados ignoráveis, o que
tornava inalcançável o arm que produzia essa mensagem e contradizia tanto o
comentário da própria função quanto a decisão registrada em CORPUS-REAL.md.

## 3. `mixin on`

`mixin M on Base` é aceito, com as duas propriedades que a restrição existe para
garantir:

* **dentro do corpo do mixin, os membros de `Base` estão visíveis** — campo e
  método, pelo `this` implícito;
* **`M` só pode ser aplicado onde `Base` está na cadeia de superclasses**.

A verificação da aplicação acontece na expansão de mixins
(`crates/hir/src/mixins.rs`), que é o ponto em que a cadeia da aplicação fica
conhecida. Ela percorre **apenas ligações de superclasse**: uma classe que apenas
`implements Base` não satisfaz `on Base`, porque a restrição promete que os
membros de `Base` estarão lá em execução. Uma aplicação sintética conta quando o
mixin que a originou é a própria restrição, o que cobre
`class C extends B with A, M` com `mixin M on A`.

```
Mixin 'M' constrains on 'Base', which is not in the superclass chain here
```

A visibilidade dos membros vem de registrar a restrição como superclasse da
**tabela semântica** do mixin (`crates/semantic/src/lib.rs`), não da AST: a
expansão continua lendo `Class::superclass`, que num mixin permanece vazio, de
modo que a cadeia da aplicação é a da classe que aplica. O efeito colateral é
correto e desejado: `M` passa a ser subtipo de `Base`, como no Dart.

Dois limites explícitos:

```
only a mixin declaration accepts an on constraint
```

```
this subset accepts a single on constraint per mixin: several constraints would
need a synthesized intersection type to resolve members against; declare the
shared supertype and constrain on it
```

`super.membro` dentro de um mixin continua fora do subconjunto, pela recusa geral
de `super method calls are not supported yet`: a forma que resolve pelo `this`
implícito — `descreve()` em vez de `super.descreve()` — é a aceita.

## 4. `assert` na lista de inicialização

`C(this.x) : assert(x > 0);` é aceito. A asserção roda **antes do corpo e antes
de `super`**, e `assert` e `campo = valor` são entradas da **mesma** lista,
avaliadas na ordem escrita: `C(int v) : a = v, assert(v > 0), b = v + 1;` emite
a escrita de `a`, depois a asserção, depois a escrita de `b`.

A ordem é preservada sem transformar `ConstructorExtras::initializers` numa lista
de variantes — o que quebraria todo construtor montado por literal. Cada
`ConstructorAssert` guarda em `before` quantas entradas `campo = valor` foram
escritas antes dela, e tanto a análise quanto a emissão percorrem a lista por
esse índice.

No Dart, um formal `this.campo` também é parâmetro e a lista de inicialização o
lê pelo nome escrito. A função de inicialização emitida recebe esses formais como
`$dartforgeFormal{índice}`, então a emissão acrescenta `const $df_x =
$dartforgeFormal0;` — e só quando há asserção, para que o caminho comum continue
emitindo exatamente o mesmo texto de antes.

**Divergência conhecida, e é do `assert` de instrução também:** a mensagem do
DartForge é `Failed assertion: is not true.` (ou `Failed assertion: <mensagem>`),
enquanto o SDK produz `'file:///…': Failed assertion: line 24 pos 29: 'x > 0':
is not true.`, com arquivo, linha, coluna e o texto da condição. O DartForge
**sempre** emite a asserção, enquanto `dart run` só a executa com
`--enable-asserts`; por isso os oráculos deste documento foram rodados com essa
flag.

Condição não booleana e mensagem que não seja `String` são recusadas pelo mesmo
caminho do `assert` de instrução.

## 5. Construtores redirecionadores

Duas formas, as duas aceitas:

* **generativo** — `C.nomeado() : this(0);` e `C.nomeado() : this.outro();`
* **fábrica** — `factory C.x() = Outra;` e `factory C.x() = Outra.nomeado;`

Um redirecionador **delega inteiramente**: não executa corpo próprio, não
inicializa campo algum e não chama `super`. A função de inicialização emitida
chama a do alvo com os argumentos escritos, reposicionando rótulos para o índice
do parâmetro correspondente — a mesma máquina que já repassava os argumentos de
`super`.

A fábrica redirecionadora vira um corpo sintetizado que repassa cada parâmetro e
converte o resultado com `as`, o que mantém a verificação em execução e dispensa
sub-tipagem na análise.

Os limites, com span sobre o `this(...)` escrito — o último aponta o formal:

```
a redirecting constructor delegates entirely and cannot declare a body
```

```
a redirecting constructor delegates entirely: it cannot also initialize fields
or call super; move those to the target constructor
```

```
the redirection must be the last entry of an initializer list
```

```
A redirecting constructor cannot declare the initializing formal 'this.x': it
delegates the whole initialization to the target; declare a plain parameter and
pass it in the redirection
```

O Dart recusa a mesma forma
(`field_initializer_redirecting_constructor`: *The redirecting constructor can't
have a field initializer*), conferido com `dart analyze` nos dois SDKs.

```
a const redirecting constructor is unsupported: the canonical instance is built
from a single recipe of fields, and following a redirection would need a second
recipe per target; declare the const constructor that initializes the fields
directly
```

Este último é o único limite que o Dart **não** tem: `const C.r() : this(1);` é
legal lá. A razão é a canonicalização, que neste subconjunto monta a instância
por uma única receita de campos.

Alvo inexistente, ciclo e aridade errada têm diagnóstico próprio:
`Unknown redirected constructor 'C.ausente'`,
`A redirecting constructor cannot redirect to itself`,
`Redirecting constructors form a cycle` e
`Incorrect redirected constructor argument count`.

## 6. `factory` com metadados

`@Deprecated('…') factory C.x() = Outra;` é aceito, assim como os metadados sem
semântica. O que não é dessa lista é recusado onde está escrito:

```
this annotation targets a class or a method, not a factory; a factory accepts
Deprecated and the semantics-free annotations of package:meta
```

Uma fábrica continua exigindo uma declaração de classe comum
(`factory declarations require an ordinary class declaration`) e não aceita
`@Native`.

## 7. Incremento e decremento como expressão

`x++`, `x--`, `++x` e `--x` valem em posição de expressão: `a[i++]`, `x = y++`,
`a[--d]`, `a[e++] = 99`.

A forma pós-fixa produz o valor **anterior** à atualização; a prefixa, o já
atualizado. **`a[i++]` avalia `i` uma única vez e indexa com o valor anterior**,
porque `$dartforgeIndex(receptor, índice)` recebe o índice como argumento — uma
avaliação só — e o `i++` do JavaScript tem exatamente a semântica do Dart. O
mesmo vale para `$dartforgeIndexSet` na escrita.

O alvo é sempre um **nome simples**, e a AST guarda a expressão inteira, não só o
nome, para que ele atravesse o linker e os otimizadores pelo mesmo caminho de
qualquer identificador — renomeação de membro privado e remapeamento de span
inclusive.

`x++;` como **instrução isolada** não chega a essa variante: o parser já a
reescrevia como `x = x + 1`, e continua. Só a posição de expressão produz
`ExprKind::Increment`, e é por isso que os testes negativos desta seção usam
`print(...)` — um `x++;` solto cai nos diagnósticos de atribuição.

Os limites:

```
`++` accepts only a simple variable as target: updating `a[i]` or `o.field` has
to evaluate receiver and index exactly once, which needs temporaries this subset
does not emit yet; write `a[i] = a[i] + 1` as a statement instead
```

```
`++` requires an int, double or num target: Dart defines it as `s = s + 1`,
which needs the numeric operator
```

```
Cannot apply `++` to final variable 'x'
```
(também `final field` e `final top-level variable`)

```
`++` on the late declaration 'x' is unsupported: it reads and writes the same
name, and a late read is a checked call rather than an assignment target; write
`x = x + 1` as a statement
```

```
`++` on 'x' is unsupported: the declaration has a setter but no field, so
reading and writing would go through two different members; write the pair
explicitly
```

## 8. `expression complexity limit exceeded`

Esta linha da tabela **não** era uma lacuna de linguagem, e mexer no número sem
entender por que ele existe trocaria um problema por outro. A investigação achou
duas coisas diferentes confundidas num contador só.

`MAX_DEPTH` protege a pilha. Ele conta a profundidade da **árvore**, não a da
leitura: `a+b+c+d` é lido num laço, sem recursão, mas produz uma espinha esquerda
tão funda quanto a cadeia é longa, e é essa espinha que a análise semântica, os
otimizadores, a emissão e até o `Drop` da árvore percorrem recursivamente. Por
isso `binary` e `postfix` aprofundam o nível a cada iteração, e o teto continua
em 64.

`MAX_EXPR_NODES` é outra coisa: um teto de **tamanho**, que não protege pilha
nenhuma. O valor antigo, 128, confundia largura com profundidade.
`type1_fonts.dart` do pacote `pdf` declara `const List<double>` com 255
elementos — uma árvore larga e de profundidade 1, sem risco nenhum de estouro — e
ainda assim era recusada. Um literal de tabela é código real e precisa caber: o
teto passou a 65.536, dimensionado para isso e ainda finito para entrada
arbitrária.

A separação é o ponto. Aumentar um contador que protege a pilha teria trocado um
diagnóstico chato por um estouro; aumentar o que só limita tamanho não tem esse
efeito, porque a profundidade continua limitada pelo outro.

Os testes de `crates/parser/src/lib.rs` afirmam as duas metades da separação, e
foram reescritos para isso: `calls_and_if_share_complexity_guards` prova que uma
tabela de 1000 elementos — larga, de profundidade 1 — **cabe**, e que uma
expressão com `MAX_EXPR_NODES + 2` argumentos **não cabe**; as cadeias de 1000
níveis de `f(` e de `if(true){` continuam recusadas pelo limite de profundidade,
que não mudou. Os casos que provavam recusa com 1000 nós — largura, não
profundidade, inclusive as seções de cascata — passaram a usar o próprio
`MAX_EXPR_NODES`, de modo que a afirmação acompanha a constante em vez de
repetir um número solto.

## Desempenho: alocações idênticas, tempo com ruído

`cargo bench -p dartforge-compiler --bench incremental`, antes e depois, na mesma
máquina e no mesmo dia. O "antes" foi obtido revertendo **apenas** os arquivos
deste trabalho para o commit anterior (`crates/parser/src/lib.rs`,
`crates/semantic/src/{lib,constructors}.rs`, `crates/codegen/src/{constructors,lib,types}.rs`,
`crates/optimizer/src/lib.rs`), rodando o benchmark e restaurando-os: o "antes" e
o "depois" diferem exatamente por esta mudança, e por nada mais.

| Cenário | Alocações antes | Alocações depois | Mediana antes | Mediana depois |
| --- | --- | --- | --- | --- |
| frio | 16.727 | **16.727** | 4,845 ms | 4,544 ms |
| sem edição (acerto de cache) | 124 | **124** | 0,817 ms | 0,462 ms |
| acerto em disco | 87 | **87** | 0,634 ms | 0,672 ms |
| edição de comentário | 15.815 | **15.815** | 6,831 ms | 20,888 ms |
| edição de corpo | 15.813 | **15.813** | 5,197 ms | 5,343 ms |
| edição de assinatura | 15.826 | **15.826** | 5,284 ms | 5,168 ms |
| edição de constante | 15.815 | **15.815** | 6,282 ms | 5,652 ms |
| edição de import | 16.584 | **16.584** | 6,816 ms | 7,826 ms |
| forma plano (frio) | 16.726 | **16.726** | 5,858 ms | 6,286 ms |
| forma profundo (frio) | 16.924 | **16.924** | 6,410 ms | 6,085 ms |
| forma classes (frio) | 22.010 | **22.010** | 6,913 ms | 6,301 ms |

**As alocações por compilação são idênticas nos onze cenários** — e o pico de
bytes vivos também, byte a byte. Não há regressão a justificar, e a razão é
estrutural: o corpus do benchmark não escreve `mixin on`, `assert` em lista de
inicialização, redirecionador nem `const` sem anotação, então o código novo não
é executado; o que ele executa a mais é um `Option::or` por classe na montagem da
tabela semântica, que não aloca.

Os tempos oscilam nos dois sentidos — a mediana fria caiu, a de "edição de
comentário" triplicou — porque três agentes compilavam em paralelo na mesma
máquina durante as medições. É exatamente o motivo pelo qual
[DESEMPENHO.md](DESEMPENHO.md) mede alocações: elas não dependem da carga, e a
igualdade acima é a afirmação que vale.

## O que os números não mudam

Fechar estas lacunas **não** move muito o total de arquivos aceitos do corpus, e
CORPUS-REAL.md já explicava por quê: um arquivo com uma lacuna real corrigida
continua reprovado pelo primeiro nome de outro arquivo que ele mencione, no modo
de unidade isolada. O que muda é o que o compilador consegue expressar, e é isso
que estes testes afirmam.
