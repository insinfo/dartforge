# Backend nativo AOT: recursos de Dart de produção

Este documento é o contrato do backend **LLVM/AOT** (`crates/llvm`, driver em
`crates/native`) para os recursos que o frontend passou a aceitar: variáveis e
constantes de nível superior, membros estáticos de classe, construtores nomeados
com lista de inicialização e `super` explícito, o operador ternário e o
apagamento de genéricos.

A regra que orienta tudo aqui é a do projeto: **o nativo e o JavaScript precisam
concordar no comportamento observável do mesmo programa.** Onde o DartForge
diverge do SDK Dart, a divergência é registrada nesta página e afirmada por
teste — nunca escondida. Os testes estão em
`crates/compiler/tests/nativo_producao.rs`, com o Dart 3.6.2 (`dart run`) e o
Dart 3.13.4 como oráculos.

## 1. Variáveis e constantes de nível superior

### A decisão: inicialização na carga

O alvo nativo inicializa **na carga**, não preguiçosamente. Todos os estáticos
são gravados no começo de `dartforge_entry()`, antes da primeira instrução de
`main`, nesta ordem observável:

1. os campos estáticos de cada classe, percorrendo as classes em ordem de
   herança — bases antes das derivadas, empate resolvido pela ordem escrita;
2. as variáveis de topo da biblioteca, na ordem escrita.

Essa é exatamente a ordem do backend JavaScript, que emite as classes — com seus
membros `static` — antes dos `let`/`const` de módulo e chama `main()` por último.
A ordem foi copiada de lá, e não inventada, justamente para que os dois alvos
concordem.

`const` não recebe tratamento próprio. Um `const` escalar ocupa o mesmo slot que
um `final` e é gravado pela mesma expressão já validada pela análise semântica: o
resultado observável é idêntico, e o alvo nativo não tem canonicalização de
objetos `const` para preservar. `final` e `var` diferem apenas no que a análise
semântica permite escrever depois; para a emissão são o mesmo slot.

### A divergência com o SDK

O Dart 3.6.2 e o 3.13.4 inicializam variáveis de topo e estáticos
**preguiçosamente**, no primeiro acesso. Para este programa:

```dart
int rastro(String nome, int valor) { print(nome); return valor; }
int topo = rastro('topo', 1);
final int fixo = rastro('fixo', 2);
class Base { static int contador = rastro('Base.contador', 10); }
class Derivada extends Base { static int contador = rastro('Derivada.contador', 20); }
void main() { print('main'); print(topo); print(fixo); print(Base.contador); print(Derivada.contador); }
```

| Alvo | Primeiras linhas |
| --- | --- |
| Dart 3.6.2 e 3.13.4 | `main`, `topo`, `1`, `fixo`, `2`, `Base.contador`, `10`, … |
| DartForge JavaScript | `Base.contador`, `Derivada.contador`, `topo`, `fixo`, `main`, `1`, `2`, `10`, … |
| DartForge nativo AOT | idêntico ao JavaScript |

O conjunto de linhas impressas é o mesmo e o **valor** de cada leitura coincide
com o do SDK; só a ordem dos efeitos dos inicializadores muda. A divergência é do
projeto, não deste backend: reproduzi-la no nativo é o que mantém os dois alvos
do DartForge de acordo. O teste
`a_ordem_de_carga_e_a_mesma_nos_dois_backends_e_difere_do_sdk` fixa as duas
saídas e afirma que diferem.

### A consequência: leitura prematura é recusada

Com inicialização na carga, ler um estático cujo inicializador ainda não executou
não tem resposta correta. O JavaScript emitido **falha** nesses casos: `let` e
`const` de módulo e as declarações de classe ficam na zona morta temporal e
lançam `ReferenceError: Cannot access '…' before initialization`. O nativo não
tem como responder melhor e também não pode inventar um zero, então recusa em
tempo de compilação, com o span da leitura. **Nenhum programa que o JavaScript
emitido executa com sucesso é recusado por esta regra.**

| Forma | Mensagem exata |
| --- | --- |
| `int a = b + 1; int b = 2;` | `LLVM AOT ainda não suporta a leitura de um estático ainda não inicializado (a ordem nativa é a da carga)` |
| `int a = a + 1;` | idem |
| `class A { static int x = B.y + 1; } class B { static int y = 2; }` | idem |
| `int f() => b; int a = f(); int b = 2;` | `LLVM AOT ainda não suporta acesso a estático dentro de rotina chamada por inicializador de estático` |

A segunda mensagem é conservadora de propósito: o fecho transitivo de chamadas
não sabe em que ponto da sequência a rotina executa, e o despacho dinâmico é
aproximado pelo nome do método. Aproximar para mais alcança mais rotinas e
portanto recusa mais programas — nunca aceita um que deveria ser recusado. Ler ou
escrever um estático em qualquer outro lugar, inclusive numa função de topo
chamada por `main`, continua livre.

### Representação

Os estáticos vivem numa **área única** alocada no heap gerenciado, com um slot
por declaração — dois para `int?` e `bool?`, como nos campos de instância. O
handle da área fica no global LLVM `@df_statics` e é registrado como raiz do
frame de `dartforge_entry`, que vive por toda a execução. As leituras e escritas
usam `dartforge_object_get`/`dartforge_object_set`, o mesmo protocolo dos campos
de instância, então o GC preciso já rastreia os estáticos de referência sem
nenhuma função nova no runtime — raízes só existem em frames, e um global LLVM
comum não seria alcançado pela coleta.

A área usa a classe sintética `-1`. IDs nominais do frontend são `u32`, então
nenhuma classe do usuário colide com ela e nenhum `switch` de despacho virtual
pode selecioná-la por engano.

Um programa sem estáticos não paga nada: a área não é alocada e `@df_statics` não
é emitido.

## 2. Membros estáticos de classe

Campos e métodos `static` **não participam de herança nem de despacho
dinâmico**. `C.v` e `C.m(...)` resolvem sempre na declaração escrita, a tabela de
estáticos é indexada por `(classe, nome)` e nunca herda do pai — ao contrário da
tabela de métodos de instância, que incorpora a cadeia de bases. Uma subclasse
não recebe o estático como membro: `D.v` com `v` declarado só em `B` é recusado
pela análise semântica, nos dois backends, com a mesma mensagem
`Static member 'v' belongs to 'B' and is not inherited`.

Campos estáticos ocupam slots da área descrita acima. Métodos estáticos viram
funções sem receptor, com símbolo `df_static_{classe}_{índice}`, onde o índice é
a posição escrita do método na declaração.

Escrever num campo estático (`C.v = 1`) ainda é recusado pelo parser
compartilhado (`static field assignments are not supported yet`); escrever numa
variável de topo é suportado nos dois backends.

## 3. Construtores nomeados, listas de inicialização e `super` explícito

A construção é emitida em três camadas, iguais em forma às do backend
JavaScript:

| Símbolo | Papel |
| --- | --- |
| `df_new_{classe}[_{índice}]` | aloca o objeto e devolve o handle |
| `df_init_{classe}[_{índice}]` | executa a ordem de inicialização do Dart |
| `df_ctorbody_{classe}[_{índice}]` | corpo escrito do construtor |

`df_init_` executa, nesta ordem: os inicializadores de declaração dos campos da
própria classe — com um formal `this.campo` no lugar do valor, preservando os
efeitos do inicializador escrito —, a lista de inicialização, a construção da
base (cujos argumentos de `super` são avaliados **nesse ponto**) e por fim o
corpo. Como a base inicializa no meio da rotina derivada, o corpo da base termina
antes de o corpo da derivada começar, que é o que o Dart 3.6.2 faz.

`df_init_` existe também para classes abstratas e aplicações de mixin, porque uma
derivada precisa chamá-la para inicializar o prefixo herdado. `df_new_` só existe
para classes concretas. Uma declaração que só tem construtores nomeados não ganha
a variante sem nome: `C()` não existe e nenhuma derivada pode chamá-la
implicitamente, porque o Dart exige `super.nome(...)` nesse caso.

### Mangling

O sufixo `_{índice}` é a **posição escrita** do construtor nomeado na
declaração — `df_new_3_1` é o segundo construtor nomeado da classe de ID 3. O
mangling é determinístico e estável, e a razão de ser numérico em vez de
`NomeClasse_nomeConstrutor` é uma invariante que este crate mantém em todos os
símbolos: **nenhum identificador do usuário é interpolado na IR**. O mesmo vale
para `df_fn_{índice}`, `df_method_{classe}_{índice}` e
`df_static_{classe}_{índice}`. A forma legível `Classe.nome` mapeia um para um
nesses pares `(classe, índice)`, que aparecem na ordem escrita; o teste
`construtores_nomeados_e_estaticos_usam_mangling_deterministico` confere os
símbolos e também que nenhum nome do programa vazou para a IR.

### Limites

| Forma | Mensagem exata |
| --- | --- |
| lista de inicialização sobre campo herdado | `LLVM AOT ainda não suporta lista de inicialização sobre campo que não é da declaração` |
| `super.nome` inexistente na base | `LLVM AOT ainda não suporta construtor nomeado ausente na base` |
| base com parâmetros obrigatórios sem `super(...)` escrito | `LLVM AOT ainda não suporta construtor da base com aridade diferente da chamada de super` |
| campo não anulável sem valor | `LLVM AOT ainda não suporta campo não nullable sem inicializador ou initializing formal` |
| fábrica nomeada | `LLVM AOT ainda não suporta fábricas nomeadas (lowering nativo pendente)` |
| getter estático ou `@Native` em estático | `LLVM AOT ainda não suporta getters estáticos e @Native em membro estático` |

## 4. Operador ternário

`cond ? a : b` baixa para grafo de controle: `br i1` sobre a condição, um bloco
`then`, um bloco `else` e um `phi` no bloco de junção. **Exatamente um operando é
avaliado, exatamente uma vez**, e a ordem de avaliação do Dart é preservada: a
condição primeiro, depois só o ramo escolhido. Os predecessores do `phi` são os
blocos reais em que cada ramo terminou, não os rótulos em que começaram, o que
mantém o `phi` correto com ternários aninhados.

A unificação dos tipos dos dois ramos segue a ABI interna: ramos do mesmo tipo
produzem esse tipo; um ramo `null` promove o outro à forma anulável, com
`insertvalue` no ramo não nulo para montar `{ i1, payload }`; duas classes
compatíveis produzem a mais geral. Tipos sem supertipo comum na ABI são recusados
com `LLVM AOT ainda não suporta tipos incompatíveis no operador condicional`.

O teste `ternario_avalia_cada_operando_uma_unica_vez` confere uma chamada por
ramo, um único `br i1` e um único `phi`; o diferencial confere a saída contra o
JavaScript e contra o Dart 3.6.2.

## 5. Genéricos no rebaixamento

O frontend **apaga** o parâmetro de tipo para o seu bound antes da HIR: em
`class Box<T extends Base>`, `T` chega ao backend nativo já como `Base`. O
apagamento é seguro nesse caso porque o parâmetro vira o handle da classe do
bound e **nenhuma operação do subconjunto nativo observa o argumento de tipo**.
Um `Box<T extends Base>` com campo `T v` e getter `T get valor` compila e executa
no AOT.

Onde o apagamento seria observável, o backend **recusa** em vez de responder
errado:

| Forma | Mensagem exata |
| --- | --- |
| `x is T`, `x as T`, qualquer teste ou cast | `LLVM AOT ainda não suporta testes e casts de tipos reificados` |
| função genérica de topo | `LLVM AOT ainda não suporta funções genéricas` |
| chamada com argumentos de tipo escritos | `LLVM AOT ainda não suporta chamadas genéricas` |
| argumento de tipo reificado registrado pela análise | `LLVM AOT ainda não suporta argumentos de tipo reificados (o apagamento seria observável)` |
| bound `Object`/`Object?`, parâmetro de tipo anulável | `LLVM AOT ainda não suporta Object e parâmetros de tipo reificados` |
| `T` que chega sem apagamento em campo ou método | `LLVM AOT ainda não suporta classes genéricas (parâmetros de tipo em campos ou métodos)` |
| `List<T>`, `Map`, tipos de função | `LLVM AOT ainda não suporta coleções e funções como valores (lowering nativo pendente)` |

O backend JavaScript responde `is`/`as` sobre genéricos com descritores em
execução; o nativo não os tem. Essa é a única assimetria de recurso relevante
desta frente, e ela aparece como recusa do compilador, não como resposta
diferente em execução.

## Forma do IR e o JIT

`crates/jit` consome o mesmo LLVM IR que o AOT. Esta frente mudou a **estrutura**
da emissão de construtores: o par `df_new_{id}` + `df_ctorbody_{id}` virou a
tripla `df_new_` / `df_init_` / `df_ctorbody_`, com sufixo `_{índice}` para
construtores nomeados. Os símbolos são todos internos ao módulo e nenhum símbolo
novo do runtime foi declarado — o conjunto de `RUNTIME_SYMBOLS` que o JIT
registra por endereço absoluto continua o mesmo. O IR passou a ter um global
`@df_statics` e a chamar `dartforge_object_new` com classe `-1` no prólogo da
entrada; as duas coisas usam recursos que o IR já tinha (`private constant` para
strings e o próprio `dartforge_object_new`).

## Verificação

```
cargo test -p dartforge-llvm
cargo test -p dartforge-compiler --test nativo_producao
cargo test -p dartforge-compiler --test nativo_producao -- --include-ignored
```

Os testes ignorados por padrão exigem Node.js e `clang` LLVM 17+ com `rustc`
(`DARTFORGE_CLANG`/`DARTFORGE_RUSTC`). O diferencial nativo roda em `-O0` e `-O2`
com `DARTFORGE_GC_STRESS=1`, que força coleta em cada alocação e prova que a área
de estáticos e os handles construídos permanecem enraizados.
