# Plano do backend nativo — exceções, medição e o que vem depois

Este documento é o plano exigido pelo `docs/BRIEF-NATIVO.md` §5: **qual
mecanismo de exceção** o compilador nativo usa e **por quê**, com o custo no
caminho feliz; **como `finally` com `return` dentro se comporta**; e a **ordem
de trabalho**. Ele descreve o que está implementado e o que falta, não uma
intenção: o que está escrito como "hoje" foi lido no código e, onde diz
"verificado", foi medido contra o oráculo (`dart run --enable-asserts`).

---

## 0. Ponto de partida desta sessão

A branch `nativo-excecoes-aot` começou num commit de preservação que **não
compilava**. Os defeitos, corrigidos antes de qualquer coisa nova:

1. `crates/emit_native/src/lower/mod.rs` — o `for` das funções ficou sem
   fechar e a propagação de `to_string_symbol` caiu dentro dele. A propagação
   lê `module.classes` já preenchido, então o lugar dela é **depois** do laço.
2. `crates/emit_native/src/lower/fn_builder.rs` — `v_elem.initializer` não
   existe: `VariableElement` guarda um `VariableRef` que aponta para o AST da
   unidade declarante, não a expressão. Ver §5.1.
3. `crates/runtime/src/runtime_main.rs` —
   `dartforge_concurrent_modification_error_new` era chamado com a coleção
   modificada mas declarado sem parâmetros; o runtime é compilado por `rustc`
   fora do `cargo`, então `cargo check` não via o erro. Ver §5.2 e §5.3.
4. `crates/cli/src/nativo.rs` — `dartforge aot` chamava
   `dartforge_compiler::compile_path_llvm_*`, da trilha velha, que já não é
   dependência do CLI: **o CLI não compilava com a feature `nativo`**. `aot`
   passou a ser o apelido de produção do mesmo caminho do `compile-native`
   (`emit_native`), para existir uma trilha nativa só.

---

## 1. O mecanismo de exceção: retorno por valor com verificação

### 1.1 A escolha

O compilador usa **exceção pendente no runtime + verificação depois de cada
chamada** (`emit_call_with_check` em `lower/fn_builder.rs`), **não** os
`invoke`/`landingpad` do LLVM.

Como funciona:

* O runtime guarda uma exceção pendente por isolate
  (`dartforge_exception_throw`, `_pending`, `_peek_bits`, `_peek_tag`,
  `_clear` em `crates/runtime/src/runtime_main.rs`).
* `throw e` grava a exceção e desvia para o alvo de exceção corrente.
* **Toda chamada** que possa lançar é seguida de
  `dartforge_exception_pending()` e um desvio condicional: se há exceção
  pendente, vai para o `exception_target` do topo da pilha (o despacho de
  `catch`, ou a entrada do `finally`, ou o `exception_target` do `try`
  externo, ou o retorno da função); senão segue o caminho normal.
* Uma função que sai por exceção retorna o valor-padrão do seu tipo de
  retorno; quem chamou descobre pela bandeira, não pelo valor.

### 1.2 Por que não landing pads

Os `landingpad` do LLVM são mais baratos no caminho feliz — **zero
instruções**: a tabela de desenrolamento é consultada só quando algo lança.
Ainda assim não são a escolha aqui, por três razões que valem mais do que a
diferença:

1. **Dependência de plataforma.** `landingpad` exige um modelo de
   personalidade do alvo. No Windows MSVC, que é o alvo do proprietário, é
   SEH (`__CxxFrameHandler3`/`__C_specific_handler`) com `catchpad`/
   `cleanuppad` e *funclets* — um dialeto de IR diferente do Itanium ABI do
   Linux/macOS. Adotar `invoke` significa **dois** lowerings de exceção, e o
   de funclets restringe o que pode existir dentro de um bloco de limpeza.
   O modelo por valor é um só, em toda plataforma.
2. **O runtime é nosso e está em Rust.** A fronteira `extern "C"` entre o IR e
   `crates/runtime` não desenrola: `panic` atravessando `extern "C"` é
   definido como abortar. Uma exceção lançada *de dentro* do runtime (índice
   fora de faixa, divisão por zero, modificação concorrente — a maior parte
   das exceções de um programa Dart real) não tem como virar um desenrolamento
   sem um mecanismo próprio de qualquer jeito. A bandeira pendente **é** esse
   mecanismo, e ela já atende os dois lados.
3. **O alvo é `async`.** O `await` de uma função `async` é um ponto de
   suspensão: a pilha de máquina é desmontada e remontada por uma máquina de
   estados. Exceção que atravessa `await` não pode depender do desenrolamento
   da pilha nativa, porque a pilha nativa do `throw` não é a do `catch`. Uma
   exceção **como valor** atravessa a máquina de estados sem caso especial;
   um `landingpad` exigiria um segundo caminho só para funções `async` — de
   novo, dois lowerings.

### 1.3 O custo no caminho feliz, e o que o paga

O custo é real e é este, por chamada que possa lançar:

```llvm
  %r = call i64 @alguma_funcao(...)
  %p = call i8  @dartforge_exception_pending()
  %c = icmp ne i8 %p, 0
  br i1 %c, label %trata_excecao, label %segue
```

Três instruções e um desvio quase sempre não tomado.
`dartforge_exception_pending` é uma leitura de uma variável do isolate; com
`-O2` o Clang **não** consegue eliminá-la hoje porque é uma chamada externa
opaca — o compilador tem de supor que qualquer chamada a escreve.

Duas saídas, nesta ordem de prioridade, e **medidas antes de adotadas**:

* **Não emitir a verificação onde ela não pode disparar.** A HIR sabe quando
  uma chamada é para uma função folha do runtime que nunca lança
  (`dartforge_string_length`, aritmética já verificada, `print`). Cada
  verificação que não é emitida é a redução mais barata que existe, e não muda
  o modelo.
* **Trocar a chamada por uma leitura de campo do isolate.** Se o ponteiro do
  isolate for um parâmetro implícito (ou um `thread_local` exposto como um
  global `@dartforge_pending_flag`), a verificação vira `load`+`icmp`+`br`,
  que o LLVM funde, iça para fora de laços e prediz corretamente. É a
  otimização que torna o modelo competitivo com `landingpad` em código sem
  exceção.

A medição exigida pelo brief ("meça as duas em programas sem exceção") fica
**pendente** e precisa dos dois itens acima primeiro: medir o modelo por valor
na forma ingênua contra um `landingpad` que ainda não existe compararia a pior
versão de um com a melhor versão do outro. O que decide é o modelo por valor
**com** a verificação como `load` e **sem** as verificações inúteis.

### 1.4 `finally` — o modelo do discriminador de razão

`finally` é baixado como um bloco com um **`phi` de razão** (`reason_phi`) e um
**`phi` de valor de retorno** (`ret_val_phi`), e termina num `switch` sobre a
razão. Cada caminho que sai do `try`/`catch` registra por onde saiu:

| razão | significado | para onde o `switch` manda |
| --- | --- | --- |
| 0 | o `try` (ou o `catch`) terminou normalmente | bloco de junção, segue o código |
| 1 | havia um `return` pendente | refaz o `return` com `ret_val_phi` |
| 2 | há exceção pendente | repropaga para o `finally`/`catch` externo, ou sai da função |
| 3 | havia um `break` pendente | refaz o `break` |
| 4 | havia um `continue` pendente | refaz o `continue` |

Isto é o `finally` como sub-rotina com continuação explícita. `break`,
`continue` e `return` dentro de um `try` **não** desviam direto: eles empilham
`(bloco, razão, valor)` no `FinallyScope` e desviam para o `finally`, que
executa e então refaz a saída. `route_return`/`route_break`/`route_continue`
fazem isso, e aninham: um `finally` que sai com razão 2 e tem outro
`finally_scope` acima repassa a razão para o de cima em vez de sair da função.

### 1.5 `return` **dentro** do `finally`

Regra do Dart: **um `return` dentro do `finally` vence** — ele descarta o
`return` pendente e, mais importante, **descarta a exceção pendente**.
Verificado no oráculo (`dart run --enable-asserts`):

```dart
int a() { try { return 1; } finally { return 2; } }   // 2
int b() { try { throw 'x'; } finally { return 3; } }  // 3, sem exceção
```

O lowering acerta isso **por construção**, e vale registrar por quê: quando o
corpo do `finally` é baixado, o `FinallyScope` dele **já foi tirado da pilha**
(`self.finally_scopes.pop()` acontece antes de `lower_stmt(fin_stmt)`). Então
o `return` dentro do `finally` cai em `route_return` com um escopo a menos:
ele chama `dartforge_exception_clear()` — é isto que faz a exceção sumir — e
termina o bloco com um `Return` de verdade (ou, se houver um `finally` ainda
mais externo, roteia para ele com razão 1, que é o comportamento certo). Como
o bloco ficou terminado, o `switch` da razão **nunca chega a ser emitido**, e
não existe caminho pelo qual a razão 2 sobreviva.

A ordem de `catch`/`finally` aninhados também foi verificada no oráculo: o
`finally` interno roda **antes** do `catch` externo que vai pegar a exceção,
e o `finally` externo depois do corpo do `catch`.

---

## 2. Como medir — o placar nativo é um comando

Sem placar automático o número do corpus é medido no olho. O harness
`crates/diferencial` roda o corpus pelo backend nativo:

```powershell
cargo build --release -p dartforge-diferencial
target\release\dartforge-diferencial.exe --nativo
# um só programa:
target\release\dartforge-diferencial.exe --nativo --filtro 76_erros
```

No modo `--nativo` ele compara **`dart run --enable-asserts` contra o
executável nativo**, byte a byte no `stdout` (o `dartdevc` não participa: não
há contrato de DDC a cumprir aqui). O oráculo Dart vem do mesmo cache por hash
de conteúdo do modo JS, então a comparação custa só a compilação nativa.

O relatório agrupa as falhas **pela primeira linha do `stderr`**, em ordem de
tamanho do grupo: é a lista de trabalho, do defeito que desbloqueia mais
programas para o que desbloqueia menos.

**Limites, e por que eles existem.** Rodando o corpus nativo com `--jobs 6`,
programas gerados chegaram a 1,9 GB cada e a máquina foi a 96% de memória.
Tempo-limite não resolve isso — um laço que aloca chega a gigabytes antes de o
relógio disparar. Três tetos, todos ligados por padrão:

* **Heap do runtime: 256 MiB** (`DARTFORGE_HEAP_MAX_MB` ajusta, `0` desliga).
  Ao estourar, o processo imprime uma linha e sai com 255 — vira uma falha
  legível no placar em vez de um travamento.
* **`--jobs` no modo nativo: 2 por padrão.** Cada programa é um processo com
  heap próprio; seis em paralelo multiplicam o estrago. Suba só depois que o
  backend estiver estável.
* **`--limite-exec`: 5 s** para executar o binário, separado do `--limite` do
  oráculo `dart run` (que precisa de mais só para subir).

O harness também guarda no máximo 4 MiB de `stdout`/`stderr` por processo, e
descarta o resto continuando a ler — senão um programa em laço que imprime
enche a memória do próprio harness, e parar de ler travaria o filho antes de
o tempo-limite poder matá-lo.

Pré-requisitos no disco: `clang` (`DARTFORGE_CLANG`, padrão
`D:/LLVM/22.1.8/bin/clang.exe`), `rustc` no `PATH` (compila `crates/runtime`
para `.lib`, cacheado por hash em `target/native_cache/`) e o SDK 3.6.2 em
`C:/tools/dartsdk-3.6.2/lib` para a seção `vm`.

---

## 3. Placar e o que ele diz

| momento | corpus nativo | maior grupo de falha |
| --- | --- | --- |
| início da sessão (commit de preservação, já compilando) | **3/214** | 172 — Clang recusa o IR |
| depois da coerção de tipos no emissor | 6/214 | 56 — Clang recusa o IR |
| depois de símbolo inexistente + `phi` | 6/214 | 33 — saída diferente da VM |

O que a tabela diz é que o gargalo **mudou de natureza**: saiu de "o módulo
nem compila" para "o programa roda e imprime outra coisa". O primeiro é um
defeito só, repetido; o segundo é uma lista de formas a acertar, e é o
trabalho que continua.

---

## 4. Ordem de trabalho

A ordem é a do brief, que é a que desbloqueia mais programas por unidade de
trabalho:

1. **Deixar compilando** o que o agente anterior deixou. *Feito* (§0).
2. **Placar nativo reproduzível** — `--nativo` no harness, com os tetos de
   memória e tempo. *Feito* (§2).
3. **Fechar o que o placar aponta no IR** — o emissor tem de produzir IR
   válido para todo o corpus antes de qualquer forma nova; enquanto o Clang
   recusa o módulo, nenhuma outra correção é observável. *Em andamento.*
4. **Exceções** — `throw`/`try`/`catch`/`finally`/`rethrow`. O lowering está
   escrito (§1); falta acertar os textos de `toString` dos erros do
   `dart:core`, que o corpus compara byte a byte.
5. **`async`/`await` e laço de eventos** — a HIR já faz o desugaring em
   máquina de estados; falta `Future`/`Completer`/`Zone` e a fila de
   microtarefas no runtime. A **ordem** das microtarefas é observável e o
   corpus a compara.
6. **Genéricos reificados** — `metadata_ptr` já está reservado no cabeçalho do
   objeto (`docs/NATIVO.md` §2) e não é usado. O aviso que vem do backend JS:
   a receita de rti tem de ser `raw|Caixa<@>`, nunca `raw|Caixa`.
7. **`dart:core` da seção `vm` a partir da fonte** — é o que tira `int`,
   `String` e as coleções de serem casos especiais do compilador. Hoje 28
   programas do corpus nem carregam, por falta de `dart:math`, `dart:convert`,
   `dart:typed_data`, `dart:collection` e da parte de `dart:async`.
8. **`dart:io`**, **isolates**, **cache de módulo**.

Alvo governante: compilar e executar `package:analyzer` no nosso runtime.

---

## 5. Decisões pequenas, registradas porque custam a redescobrir

### 5.1 Inicializador de campo vem do AST, não do elemento

`VariableElement` não guarda a expressão inicializadora; guarda um
`VariableRef` (`Field { unit, member, index }` ou `TopLevel { unit, decl,
index }`) que endereça o AST da unidade declarante.
`FnBuilder::variable_initializer` resolve isso e devolve `None` quando a
unidade é **outra**: `lower_expr` recebe o `ast` da unidade corrente, e uma
`ExprId` de outra unidade indexaria a árvore errada — daria código
silenciosamente errado em vez de um erro. Quando o `late` de outra unidade
importar, a saída é baixar o inicializador como uma função própria na unidade
dele, não passar a `ExprId` para cá.

### 5.2 `ConcurrentModificationError` guarda o objeto modificado

`dart:core` (`errors.dart`) define
`ConcurrentModificationError([this.modifiedObject])`, e o `toString` usa
`Error.safeToString(modifiedObject)` quando ele não é nulo.
`Error.safeToString` existe justamente para **não** chamar o `toString` do
objeto — um `toString` que lança durante a construção de uma mensagem de erro
esconderia o erro original. Por isso a VM imprime a forma de identidade:

```
Concurrent modification during iteration: Instance(length:4) of '_GrowableList'.
Concurrent modification during iteration: _Map len:2.
Concurrent modification during iteration: _Set len:3.
```

O runtime reproduz essas três formas (`safe_to_string` em `runtime_main.rs`),
porque o corpus compara `stdout` byte a byte contra a VM.

### 5.3 O runtime não passa pelo `cargo check`

`crates/runtime/src/runtime_main.rs` é concatenado a `heap.rs` numa constante
(`dartforge_runtime::RUNTIME_MAIN`) e compilado por `rustc` avulso em
`cache.rs`. Consequência prática: **`cargo check -p dartforge-runtime` não
verifica esse arquivo**. Erro ali só aparece quando o primeiro programa é
compilado, como "compilação do runtime nativo com rustc falhou". A
verificação rápida é compilar um programa qualquer:

```powershell
target\release\dartforge-diferencial.exe --nativo --filtro 01_print
```

### 5.4 O emissor de LLVM precisa saber o tipo de cada valor

O emissor imprime o tipo LLVM fixo em cada posição (`ret i64`, `add i64`,
`br i1`, o tipo declarado de cada argumento do runtime) e imprimia o operando
como estava, sem olhar o tipo dele. Como a HIR carrega `bool` ora como `i1`
(resultado de `icmp`), ora como `i8` (a fronteira com o Rust), saía
`ret i64 %v8` para um `%v8` que era `i1`, e o Clang **recusava o módulo
inteiro** — não só aquela função. Era a maior causa isolada de falha do corpus
nativo: 172 dos 214 programas.

Hoje o emissor monta uma tabela de tipos por função e converte o operando na
posição. Duas regras não óbvias:

* Entre `double` e `i64` a conversão é `bitcast`, **não** `sitofp`/`fptosi`.
  A HIR tem nós próprios (`IntToDouble`/`DoubleToInt`) para a conversão
  numérica; um `double` ocupando um slot `i64` só pode ser o padrão de bits —
  é assim que `Const(Double)` é emitido e como os doubles atravessam o runtime.
* A entrada de um `phi` **não** pode ser convertida onde o `phi` está: `phi`
  tem de ser a primeira instrução do bloco. A conversão pertence ao bloco de
  **origem** daquela entrada, emitida logo antes do terminador dele.

E literal `double` sai na forma hexadecimal do LLVM: `format!("{d}")` imprime
`1` para `1.0`, e o LLVM recusa `double 1`.

### 5.5 Não chamar símbolo que o módulo não emite

`lower_program` só emite funções cujo `node` é `FunctionRef::Function`. Um
acessor implícito de campo (um `final int codigo;` gera um elemento getter
`codigo`) tem `node: FunctionRef::None` e nunca vira símbolo. A busca de método
por nome não filtrava por isso, então `e.codigo` virava
`call @df_fn_N_codigo` — "use of undefined value", e de novo o módulo inteiro
recusado. `metodo_de_instancia`/`funcao_de_topo` exigem o `node`, e a busca cai
no acesso a campo que já estava escrito logo abaixo.

---

## 6. Contrato de representação e raízes

### 6.1 Por que um contrato, e não correções

71 dos 214 programas do corpus nativo morriam em `panic` do runtime com
"handle inválido (NegOverflow)" (27) ou "handle não vivo" (44). Lido no
código, não são 71 defeitos: é a falta de um contrato entre o lowering, o
emissor e o runtime sobre **o que um `i64` significa**. Hoje:

* `this` cai no `_ => Const(Int(0))` de `lower_expr`, e `this.x` vira
  `object_get(0, …)`;
* atribuição a propriedade ou índice não grava nada (`Assign` só trata
  identificador);
* o construtor aloca só os campos da própria classe, grava os argumentos
  **por posição** e não roda inicializadores, lista de inicialização, `super`
  nem corpo;
* `Foo? x = null` passa pela checagem de tipo da declaração, que chama
  `dartforge_value_class(0)`;
* `Type::Ref` carrega inteiro cru (`int?`, `num`, `Object`, `dynamic`, `T`), e
  o tipo se perde em locais `alloca i64`, leituras de campo `i64` e
  `is_ref = 0` fixo em `AllocObject`/`SetField`;
* o código gerado nunca registra raízes no GC; o heap só coleta quando há um
  frame aberto, e os únicos frames são os internos do runtime — depois de
  ~256 alocações o próximo literal de coleção coleta com só ele mesmo como
  raiz.

O contrato abaixo tem quatro partes. Cada passo de código cita o invariante
que implementa; a ordem é **N → R → E → G**, e **G nunca antes de R**:
`set_root` valida o handle com `Heap::get`, então enraizar antes de a
representação ser honesta transforma cada escalar em posição `Ref` num
`panic`.

### 6.2 R — representação

* **R1.** Todo valor da HIR tem exatamente uma representação, derivada do
  tipo estático Dart: `I64` = `int` não anulável; `F64` = `double` não
  anulável; `I1` = `bool` não anulável; `Ref` = todo o resto — `String`,
  objetos, coleções, closures, records, `Object`, `dynamic`, `num`, `Null`,
  parâmetros de tipo `T`, e **qualquer tipo anulável, inclusive `int?`,
  `double?` e `bool?`**. `Void` só existe como retorno. `I8` só existe na
  fronteira com o runtime (o `bool` da ABI C) e nunca é representação de um
  valor Dart.
* **R2.** Um `Ref` é `0` (null) ou um handle vivo. Nunca um escalar cru.
* **R3.** `int`/`double`/`bool` que entram numa posição `Ref` passam por
  `Box` (no heap, `Value::BoxedInt`/`Value::BoxedDouble`; `bool` são dois
  singletons permanentes) e voltam por `Unbox`. `Box`/`Unbox` são instruções
  da HIR; o emissor as baixa para chamadas do runtime.
* **R4.** Uma função só, `FnBuilder::coagir(op, destino)`, faz toda
  conversão de representação, e é chamada em **toda** fronteira: argumento
  → parâmetro, valor → retorno, valor → local, entrada de `phi` (no bloco de
  origem, antes do terminador), valor → campo, valor → elemento de coleção,
  valor → `throw`. O emissor só alarga/estreita larguras (`i1`/`i8`/`i64`) e
  faz `bitcast` de bits; ele não muda representação.
* **R5.** `lower_expr(e)` devolve o operando na representação de
  `repr(tipo estático de e)`. Quem produz um valor a partir do heap ou do
  runtime escolhe o **acessor** pela representação de destino (`*_get_ref`
  encaixota escalares; `*_get_bits` devolve os bits para `I64`/`F64`/`I1`) —
  nunca lê bits como `I64` para depois "coagir" a `Ref`, o que encaixotaria
  um handle. Duas exceções medidas no passo 2: (a) expressão de tipo
  `dynamic` (ou sem tipo — corpo que a inferência ainda não visita) não é
  forçada a `Ref`; ela fica na representação em que foi produzida, e as
  fronteiras coagem pela representação **real** do operando; (b) a
  inferência ainda não grava a promoção de fluxo no tipo da leitura
  (`if (x != null) x + 1` lê `x` como `int?`), então o operando `Ref` de um
  operador aritmético cujo tipo estático é `int?`/`double?` volta ao escalar
  por `Unbox` ali — num programa válido ele não é null nesse ponto.
* **R6.** Cada variável local declarada tem um `alloca` no tipo da sua
  representação, criado no bloco de entrada da função; leitura é `Load` no
  mesmo tipo, escrita é `Store` depois de `coagir`. Parâmetros também moram
  num `alloca`. Locais são resolvidos por escopo léxico (pilha de escopos no
  `FnBuilder`), não por um mapa único por nome — um bloco que sombreia
  `current` não pode sobrescrever o `current` de fora.
* **R7.** O layout de um objeto é a lista dos campos **de instância** de toda
  a cadeia de superclasses, da raiz para a folha, sem os estáticos
  (`ClassElement::fields` mistura os dois). O índice de um campo vem do
  elemento resolvido (`VariableId`), procurado nesse layout; o de um método,
  do `FunctionElementId` resolvido — nunca do "primeiro membro com esse nome
  no programa". Campo é lido e gravado na representação do seu tipo
  declarado.
* **R8.** Coleções não guardam caixas: o runtime normaliza um `Ref` que
  aponta para `BoxedInt`/`BoxedDouble`/bool para o `TaggedValue` escalar na
  entrada, e a leitura escolhe o acessor pela representação de destino (R5).
  Assim `[1, 2]` e `<Object>[1, 2]` têm o mesmo conteúdo, e `1` como chave de
  mapa é a mesma chave vindo encaixotada ou não.
* **R9.** Igualdade e identidade de caixas são por valor: `identical(1, 1)` e
  `==` entre um `int` encaixotado e outro, como na VM.

### 6.3 E — arestas do heap

* **E1.** Toda gravação no heap — campo, elemento de lista, entrada de mapa,
  elemento de conjunto, célula, ambiente, record, literal de coleção — leva
  `is_ref`/tag **derivado da representação do operando**: `Ref` → tag 3 e
  `is_ref = 1`; `I64` → 1; `I1` → 2; `F64` → 4. Não existe `i8 0` literal
  para um valor `Ref`.
* **E2.** Chave de mapa com a tag real do operando (hoje `map_get_bits` usa
  3 fixo).
* **E3.** Um verificador da HIR roda antes de `emit_function` e recusa, com
  diagnóstico: tag de gravação incoerente com a representação do operando;
  constante inteira em posição `Ref` (a única constante `Ref` é `Null`); e
  instrução que o emissor não sabe baixar (o antigo `; inst pendente`).

### 6.4 N — null e o que não é suportado

* **N1.** O lowering nunca produz `0` como substituto de um valor. `this` é o
  parâmetro `this` da função. Construto que o lowering não sabe baixar é
  **erro de compilação** com o nome do construto e a posição — um programa
  que "passava" porque o `0` por acaso dava a saída certa passa a aparecer
  no placar como "não compila: <construto>", que é a verdade.
* **N2.** Acesso a membro sobre um receptor `Ref` que pode ser null e não
  usa `?.` emite `CheckNotNull`, que lança o erro Dart
  (`Null check operator used on a null value`), em vez de desreferenciar 0.
* **N3.** `?.` tem *null-shorting*: se o receptor é null, a cadeia inteira
  (`a?.b.c()`) vale null e nada à direita é avaliado.
* **N4.** No runtime, `Heap::get`/`get_mut`/`set` distinguem quatro falhas
  com mensagens próprias: handle `0` (null desreferenciado — bug do
  compilador), negativo, além da tabela (escalar usado como handle) e slot
  já coletado (raiz faltando). Nenhuma delas devolve valor padrão.
* **N5.** Construtor generativo é uma função da HIR,
  `df_ctor_<id>(this, parâmetros…)`; `C(args)` é `object_new(classe,
  |layout|)` seguido da chamada. A função executa, nesta ordem (a do
  `dartdevc`, que é a da especificação): inicializadores de campo da
  **própria** classe na ordem de declaração; parâmetros `this.x`; a lista de
  inicialização na ordem escrita (campos e `assert`); `super(…)` —
  explícito, implícito sem argumentos, ou com os `super.x` —, que roda
  recursivamente os inicializadores e o corpo da superclasse; e por fim o
  corpo. `: this(…)` delega para o outro construtor. Argumentos são casados
  com parâmetros por posição e por nome, com os valores padrão da
  declaração.
* **N6.** Variáveis de topo e campos estáticos são globais do módulo com
  inicialização preguiçosa (uma bandeira por global) e, quando `Ref`, raiz
  permanente no runtime.

### 6.5 G — raízes

* **G1.** O emissor abre `frame = dartforge_gc_push_frame(N)` na entrada de
  toda função que tem algum valor `Ref`, e dá a cada um deles um **slot
  fixo**: cada valor SSA `Ref` (parâmetro, resultado de chamada, `Load`,
  `phi`, alocação) recebe `set_root(frame, slot, v)` logo depois de ser
  definido — o parâmetro logo depois do `push_frame`, o `phi` depois do
  último `phi` do bloco.
* **G2.** Cada `alloca` de tipo `Ref` também tem um slot, e **todo `Store`
  nele é seguido de `set_root`** com o valor gravado. O plano original
  enraizava só valores SSA; isso não basta, porque um local vive mais que o
  SSA que o gravou: em `if (i == 0) saved = current;` dentro de um laço, a
  segunda volta redefine o SSA de `current` e sobrescreve o slot dele —
  sem o slot do `alloca`, `saved` ficaria sem raiz.
* **G3.** `dartforge_gc_pop_frame(frame)` antes de **todo** `ret`, inclusive
  as saídas por exceção (a função que sai com exceção pendente retorna o
  valor padrão pelo mesmo `ret`).
* **G4.** Slot fixo por SSA é correto: numa ativação há no máximo uma
  instância viva de cada valor SSA; quando a definição reexecuta num laço, a
  anterior já está morta — o que precisa sobreviver está num `alloca` (G2),
  num `phi` (que tem slot próprio) ou no heap (E1).
* **G5.** Entre o `pop_frame` do chamado e o `set_root` do resultado no
  chamador não há alocação (`Heap::pop_frame`); o mesmo vale para o valor que
  uma extern do runtime devolve.
* **G6.** Runtime: coleta em **qualquer** alocação que passe do limiar (sai
  o `!frames.is_empty()` de `Heap::allocate`); a exceção pendente, o rastro
  corrente e os globais `Ref` são raízes; extern que aloca mais de uma vez
  enraíza os temporários (frame local ou `allocate_linked`); as tabelas
  laterais indexadas por handle (`IMMUTABLE_COLLECTIONS`,
  `ACTIVE_ITERATIONS`, `ORIGIN_COLLECTIONS`) são purgadas dos handles mortos
  a cada coleta, senão um slot reutilizado herda a marca de outro objeto; e
  nenhuma extern chama outra com um `borrow_mut` do heap aberto.
* **G7.** `DARTFORGE_GC_STRESS=1` coleta antes de toda alocação; o harness
  ganha `--gc-stress`, e um programa só conta como aprovado nesse modo se
  passar também sob estresse.

### 6.6 Passo 0 — as 71 reclassificadas (medido)

Instrumentado só o runtime (N4 e `DARTFORGE_GC_OFF=1`), sem mudar o
comportamento, e executados os 214 programas compilados (sem oráculo) uma
vez, para achar e reclassificar os 71:

| mensagem nova | programas | coleta desligada |
| --- | --- | --- |
| handle além da tabela (escalar usado como handle) | 43 | 43, igual |
| handle null (0) desreferenciado | 26 | 26, igual |
| handle negativo | 1 (`41_classes_ctor_nomeado`) | igual |
| handle já coletado (raiz faltando) | 1 (`39_funcoes_recursivas_e_iteradores`) | passa a terminar |

O "handle não vivo" antigo (44) era quase todo **escalar positivo usado
como handle** (43), não objeto coletado; o "NegOverflow" (27) era quase todo
**null desreferenciado** (26). Só 1 dos 71 depende de raiz: 70 são de
representação (H3–H8). É o que a ordem N → R → E → G prevê — enquanto o
heap só coleta com frame aberto, a falta de raízes quase não aparece; ela
aparece quando G tirar o portão.

Os 7 que passavam: `01_print`, `03_strings_escapes`, `06_strings_metodos`,
`141_antigo_arithmetic`, `142_antigo_boolean`, `164_antigo_numeric_edges`,
`169_antigo_string_escapes`.

#### O que o passo 1 achou, e mudou no desenho

* **O backend nativo rodava sem `dart:core`.** A seção `vm` do
  `libraries.json` do SDK 3.6.2 só declara `cli`; `core`, `async`,
  `collection`… vêm de `"include": [{"target": "vm_common"}]`, e
  `SdkLayout::load` não seguia o `include`. O programa carregava sem o SDK,
  e todo tipo estático de primitivo (`int`, `String`, `bool`) era `dynamic`
  — `to_hir_type` devolvia `Ref` para quase tudo, e o lowering vivia de
  adivinhar pelo tipo do operando. R é impossível sem os tipos: o passo 1
  começa fazendo `SdkLayout::load` seguir o `include`. Com isso
  `Program::functions` passa a ter o `dart:core` inteiro, e o lowering só
  compila as funções do **usuário** (o runtime em Rust implementa o SDK);
  chamada a função do SDK sem implementação no runtime é diagnóstico (N1).
* **A inferência de corpos pulava comandos.** `infer_stmt` não tinha braço
  para `for-in` nem para rótulo: o corpo de todo `for (x in xs)` ficava sem
  tipos e sem resolução. Entraram os dois (o elemento vem de
  `Iterable<E>` pelos supertipos instanciados). Também: o acesso a um campo
  pelo getter implícito devolvia o **tipo de função** do getter, não o do
  campo (`a.next!.text` não resolvia `text`); corrigido em `scope.rs` e na
  leitura de getter de topo.
* **Membros de extensão** não têm lowering (o receptor implícito); eles não
  são compilados, e só o **uso** de um é diagnóstico — uma extensão
  declarada e nunca chamada não derruba o programa.
* **O diagnóstico de N1 chega ao placar pelo executável.** O certo é
  `compilar` devolver `Err`, mas `lib.rs` está congelado nesta sessão (outro
  trabalho separa a emissão de IR); o emissor produz, para um módulo com
  erros, só um `dartforge_entry` que imprime os diagnósticos e sai com 254.
  A primeira linha não tem a posição, para agrupar no harness.

### 6.7 Referências consultadas, e o que adotamos

* **Dartino (`references/dartino-llvm`, `src/vm/codegen_llvm.cc`,
  `gc_llvm.cc`).** Representação **uniforme**: todo valor é um
  `Object*`, e o inteiro pequeno é um Smi com a etiqueta no bit baixo; o
  código gerado marca os ponteiros do heap com `addrspace(1)`, roda
  `PlaceSafepoints` + `RewriteStatepointsForGC` e o GC (que move objetos)
  percorre a pilha pelos *stack maps* do LLVM (`.llvm_stackmaps`),
  atualizando pares base/derivado.
  *Não adotado agora:* os passes de statepoint precisam do pipeline do LLVM
  sob nosso controle (nós emitimos IR textual e chamamos o `clang`), e o
  leitor de stack maps teria de ler a seção no COFF do Windows — que o
  Dartino não precisava. Nosso heap não move objetos (handles indexam uma
  tabela), então não há ponteiro derivado a corrigir: a pilha-sombra
  explícita (G1–G3) é suficiente e portátil. *Adotado:* a separação
  "o GC só vê o que está marcado como referência" — no Dartino é o
  `addrspace(1)`, aqui é `Type::Ref` na HIR e o slot de raiz.
  *Registrado para depois:* Smi com etiqueta evitaria a caixa no heap para
  `int` em posição `Ref` (R3); exigiria que todo `Ref` pudesse ser um
  inteiro etiquetado, e o runtime distinguir por bit — é a otimização
  natural se o custo de `Box` aparecer na medição.
* **VM oficial (`references/dart-sdk/runtime/vm`).** `Smi` (63 bits
  etiquetado), `Mint` (inteiro de 64 bits em caixa) e `Double` em caixa; o
  código compilado tem *stack maps* compactados por ponto de segurança.
  `Instance::IsIdenticalTo` (`object.cc`) dá a semântica que R9 segue:
  mesmo ponteiro, ou dois inteiros de mesmo valor, ou dois `double`
  bit a bit iguais; `int` e `double` nunca são idênticos entre si, mas
  `1 == 1.0` é verdadeiro pelo `==` de `num`.
* **Backend LLVM do Dart VM AOT de linzj/Alibaba
  (`references/linzj-llvm-project/NOTAS.md` e `COMMITS.md`;
  `references/dart-sdk-llvm-mraleph/runtime/vm/compiler/backend/llvm`,
  `ir_translator.cc`, `stack_maps.cc`).** O tradutor monta cada chamada como
  `gc.statepoint` com os valores vivos (da própria análise de liveness) e lê
  os stack maps de volta. Cerca de 32 dos 122 commits do fork do LLVM dele
  corrigem stack maps, statepoints ou registradores salvos errados que
  derrubavam o GC ("stack maps marking uninitialized slots live, crashing
  GC", "Incorrect stack maps, missing one stack slot mark"…). É a
  confirmação empírica da escolha de G: raízes explícitas em slots, que o
  otimizador do LLVM não tem como invalidar, em vez de statepoints.
  Duas lições registradas para depois: (1) a VM não emite stack map em
  chamada a entrada de runtime LEAF (a barreira de escrita), que não pode
  disparar GC — o equivalente aqui é uma extern que comprovadamente não aloca
  não exigir os vivos enraizados antes dela; o emissor já conhece as externs
  pelo nome (a mesma tabela do verificador, `lower/verificador.rs`), e marcar
  as que não alocam é o gancho do passo 6, não desta etapa; (2) aquele
  backend **descartava** `AssertAssignable`/`AssertBoolean`/`AssertSubtype`
  — a equipe da VM chamou de violação da semântica do Dart. N vai no
  sentido oposto: construto não suportado é erro de compilação, e toda
  coerção implícita que pode falhar (`Unbox`, `as`, declaração com
  inicializador `dynamic`) é checada e lança `TypeError`.
* **`docs/PESQUISA-OTIMIZACAO.md` §2 e §16.** "Nenhum cache cresce sem
  política de descarte": as tabelas laterais por handle do runtime são
  purgadas a cada coleta (G6), e as caixas de `bool` são dois singletons
  fixos, não um cache. Arena não é ganho automático: o heap continua um
  vetor de slots reutilizáveis, sem mudança de alocador nesta etapa.

### 6.8 Custo aceito

`set_root` por definição é uma chamada externa com empréstimo do `RefCell`.
É deliberadamente o mais simples que é correto; slots por *liveness*, pular
valores mortos no ponto de coleta e pilha-sombra em memória (ou a estratégia
`shadow-stack` do LLVM) vêm depois, e só com medição antes e depois.
