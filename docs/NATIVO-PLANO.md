# Plano do backend nativo — exceções, medição e o que vem depois

Este documento é o plano exigido pelo `docs/historico/briefs/BRIEF-NATIVO.md` §5: **qual
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
| começo do contrato (§6) | 7/214 | 71 — `panic` de handle |
| passo 1 (N) | 26/214 | 17 — saída diferente; o resto, construtos não suportados |
| passo 2 (R) | 48/214 | 8 — `switch` como expressão (não suportado) |
| passos 3–4 (E, G), corpus de 222 | 50/222 (e 50 sob `--gc-stress`) | 9 — chamada de valor de função (não suportado) |

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
* **R10 (rodada 2, pré-requisito do P5).** `Ref` com `Smi` etiquetado: um
  `Ref` é `0` (null), um handle **par** (`(índice + 1) << 1`) ou um `int`
  pequeno **ímpar**, `(v << 1) | 1`, para `v` em `[-2^62, 2^62)`. O `Box` de
  um `int` nessa faixa não aloca; fora dela vai para o heap como o `_Mint`
  da VM (`Value::BoxedInt`). A forma é canônica — o que cabe no `Smi` nunca é
  encaixotado —, então `identical(1, 1)` é igualdade de bits. O bit é o
  inverso do da VM (lá `kSmiTag = 0`), para o `0` continuar sendo null sem
  mudar o código gerado. **O coletor nunca segue um `Smi`**: raiz, campo
  `Ref`, global e exceção pendente podem conter um, e a marcação pula as
  arestas ímpares; as coleções normalizam o `Smi` para o escalar (R8). Um
  `Smi` desreferenciado como objeto é o quinto erro de N4 ("Smi usado como
  handle"). Código: `runtime/src/heap.rs`, módulo `smi`.

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
  já coletado (raiz faltando). Nenhuma delas devolve valor padrão. Com R10
  há uma quinta: `Smi` (ímpar) lido como objeto.
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
* **G8. Tabela de efeitos das externs.** Toda extern do runtime é
  declarada num lugar só — `crates/emit_native/src/llvm/externs.rs` — com a
  declaração LLVM e os efeitos `{aloca, lança, chama código Dart}`, no molde
  das entradas LEAF da VM (`runtime_entry.cc:778`) e do `gc-leaf-function`
  do Dartino. Nesta etapa todas estão marcadas de forma **conservadora**
  (aloca, lança): a tabela existe para o passo 6 só trocar valores, e cada
  marca "não aloca" só pode entrar com um teste que roda a extern sob
  `DARTFORGE_GC_STRESS=1` e exige zero coletas — uma marca otimista errada é
  o defeito dos stack maps errados do lado de cá. O uso previsto (passo 6):
  extern que não aloca não é ponto de coleta, então valor `Ref` cuja vida não
  cruza ponto de coleta não precisa de slot e função sem ponto de coleta não
  precisa de quadro; e "não aloca ⇒ não lança" dispensa o
  `exception_pending()` depois dela (`docs/PESQUISA-LLVM-DART-AOT.md` §2.4).
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
É deliberadamente o mais simples que é correto. O passo 6, só com medição
antes e depois (sem raízes com `DARTFORGE_GC_OFF=1`, raiz por chamada, raiz
por `store`), tem três partes, nesta ordem: (1) usar a tabela de efeitos
(G8) — raiz e quadro só onde há ponto de coleta, e sem `exception_pending()`
depois de extern que não lança; (2) raiz como `store` simples num quadro
`alloca` da função, encadeado numa lista do isolate (a pilha-sombra de
Henderson, ISMM 2002, que é a estratégia `shadow-stack` do LLVM), em vez de
chamada opaca — o LLVM não apaga o `store` (o quadro escapa para o runtime),
mas deixa de tratar cada raiz como barreira; (3) slots por *liveness* em vez
de um por definição.

---

## 7. Rodada 2: o SDK da fonte, `async` e o executor de macros

O plano detalhado da rodada (passos P0–P9, aceite de cada um, riscos) foi
aprovado em 2026-09-23. Aqui ficam as decisões, que valem para todo o
trabalho seguinte, e o mapa de módulos que o P0 deixou pronto para o trabalho
em paralelo.

### 7.1 Decisões do proprietário (2026-09-23)

1. **O `dart:core` do nativo vem da fonte do SDK 3.6.2.** `core`, `async`,
   `collection`, `convert`, `math`, `_internal`, `_compact_hash` e depois
   `typed_data` são compilados da seção `vm` com os patches dela; uma camada
   fina de patches **nossos** substitui só o que depende da máquina interna da
   VM (`_SuspendState`, a ligação de `Timer`/microtarefas com
   `dart:isolate`/`dart:io`, finalizadores). Em Rust ficam só os natives
   (`vm:external-name`) e os intrínsecos (`vm:recognized`). Consequências:
   * o `Smi` etiquetado (R10: `Ref` = `(h << 1)` ou `(v << 1) | 1`) é
     pré-requisito do P5;
   * os membros do SDK casados pelo nome
     (`crates/emit_native/src/lower/sdk_por_nome.rs`) estão **congelados desde
     já**: nenhum caso novo nesse mecanismo; o arquivo é apagado em P5d.
2. **Executor de macros: JIT ORCv2 persistente, com um isolado novo por
   execução.** O processo guarda o grupo (código do runtime, objetos do SDK e
   das macros); cada aplicação × fase roda num isolado com heap vazio e globais
   zerados, descartado no fim. O AOT fica como verificação e *fallback*. Os
   globais `@dfg_*` do módulo LLVM passam para a tabela de slots do isolado,
   indexada por id estável (P8). Se o oráculo mostrar macro real dependendo de
   estado estático entre aplicações, o agendador passa a guardar um isolado por
   (biblioteca de macro, fase) — muda o agendador, não o runtime.
3. **`RegExp`: a crate `regress`** (semântica ECMAScript), compilada dentro do
   runtime atrás de uma interface que permita trocá-la por um motor próprio
   depois. O corpus de regex é o oráculo. Isso tira o runtime do `rustc`
   avulso sem dependências: ele passa a ser compilado com as dependências dele
   (uma `staticlib` do cargo), decisão que entra junto com o `RegExp` (P9).
4. **Heap por isolado agora.** A especificação decide: *Dart Programming
   Language Specification* §6.3, "An isolate is a unit of concurrency. It has
   its own memory and its own thread of control". Heap por grupo (a VM desde
   2.15) é otimização de implementação, não semântica: nada observável pelo
   programa depende dele. `Isolate.exit` passa a **copiar** a mensagem em vez
   de transferi-la — só custo de desempenho, nunca de comportamento. O grupo
   compartilha **código** e uma região permanente, somente leitura, de
   constantes canônicas. Isto resolve o conflito entre PLANO.md ("isolates com
   heap por isolate") e a nota de `references/NOTAS-ARTIGOS.md` §4 ("heap por
   grupo", que descreve a VM, não o requisito): vale o heap por isolado; a troca
   para grupo só se justificaria com `Isolate.spawn` + `Isolate.exit` sem cópia
   medidos, e não mudaria o código gerado (o isolado continua dono das raízes).
5. **Strings UTF-16 antes do P5.** O runtime guarda `String` como UTF-8 do
   Rust, mas a semântica do Dart é de unidades de código UTF-16 (`length`,
   índices, `codeUnitAt`, `substring` no meio de um par substituto), e o código
   do SDK da fonte depende disso. A representação muda **antes** do P5, na
   forma da VM: `_OneByteString` (Latin-1) e `_TwoByteString` (UTF-16). Aceite:
   o corpus de strings, com o programa de pares substitutos (04). O
   `docs/NATIVO.md` já descreve a representação atual (UTF-8) como ela é.

### 7.2 O P0: diagnóstico honesto

* **Construto não suportado é erro de compilação.** `emitir_ir` e `compilar`
  (`crates/emit_native/src/lib.rs`) devolvem `Err` com **todos** os
  diagnósticos quando o lowering produziu algum; nenhum IR é emitido. Saiu o
  módulo de erro que gerava um executável só para imprimir os diagnósticos e
  sair com 254, e saiu do runtime a `dartforge_erro_de_compilacao`. A primeira
  linha do erro é `erro de compilação: <primeiro diagnóstico sem a posição>` —
  a chave de agrupamento do relatório; as seguintes, um diagnóstico cada.
* **O relatório mostra tudo o que falta.** Cada falha nativa lista os
  construtos do programa (todos, não só o primeiro), e o relatório termina com
  a seção **"construtos (todos os diagnósticos)"**: por construto, quantos
  programas o usam e quantas ocorrências somam, e a lista dos programas
  bloqueados **só** por ele. O modo `determinismo --nativo` (só IR, segundos)
  imprime a mesma seção. `dartforge_emit_native::construtos_do_erro` é o único
  leitor do formato, ao lado de quem o escreve.

**A matriz exata (223 programas, `determinismo --nativo`, só IR).** 55
programas compilam; 165 têm diagnóstico de construto; 3 não carregam
(`dart:js*`). São 871 construtos distintos — o nome do membro faz parte do
construto (``chamada `where` ``, ``membro `hashCode` ``), e cada identificador
não resolvido também. Só 14 programas estão bloqueados por **um** construto
só: a primeira linha escondia que quase todo programa precisa de várias
coisas ao mesmo tempo. As 20 primeiras, por número de programas:

| programas | ocorrências | construto |
| ---: | ---: | --- |
| 37 | 215 | closure |
| 35 | 196 | operador sobre num/dynamic/objeto |
| 33 | 60 | chamada `where` |
| 25 | 41 | chamada `f` (valor de função) |
| 20 | 72 | cascata |
| 19 | 42 | construtor de classe do SDK (`List`) |
| 18 | 107 | chamada `toStringAsFixed` |
| 17 | 43 | chamada `fold` |
| 16 | 210 | await |
| 16 | 47 | membro `hashCode` |
| 16 | 29 | chamada `map` |
| 15 | 54 | chamada `reduce` |
| 14 | 146 | chamada estática `parse` |
| 14 | 104 | constante de enum |
| 14 | 41 | chamada de valor de função |
| 13 | 57 | chamada de membro sem implementação compilada |
| 12 | 51 | expressão switch |
| 12 | 23 | membro `entries` |
| 12 | 21 | chamada `containsKey` |
| 12 | 20 | chamada `toSet` |

A contagem é um **piso** para os construtos aninhados: o lowering diagnostica
a expressão que não sabe baixar e não desce nos filhos dela, então a closure
de `lista.where((x) => …)` aparece como ``chamada `where` ``, não como
`closure`. P1 (closures) e P5 (membros do SDK pela fonte) atacam as duas
metades da mesma lista.

**Placar do P0 no CI (Pesado 35836380647):** nativo **50/223** (o mesmo
de antes), JIT 50/223 com zero divergências JIT × AOT, determinismo do IR
idêntico com 1, 4 e 8 trabalhadores, JS 223/223 nos dois perfis.

### 7.3 Mapa de módulos e donos

A divisão do P0 é mecânica: nenhum comportamento mudou. Prova: o LLVM IR de
cada programa do corpus é byte a byte o mesmo antes e depois
(`dartforge-diferencial determinismo --nativo`, 223 programas, mesmo resumo
FNV-128 programa a programa), e os itens do runtime são os mesmos (cada função,
`thread_local!` e `use` movido inteiro, conferido por script).

**`crates/emit_native/src/lower/`** — o antigo `fn_builder.rs` (3.258 linhas)
virou cinco arquivos, todos `impl FnBuilder`:

| arquivo | conteúdo | dono (rodada 2) |
| --- | --- | --- |
| `fn_builder.rs` | o `struct FnBuilder`, blocos, `emit`/`terminate`, coerção (R4), rotas de `return`/`break`/`continue`, `emit_call_with_check`, `throw`, testes de tipo | compartilhado: campo novo no `struct` só por acréscimo, no fim, com comentário |
| `expressoes.rs` | `lower_expr` e o `match` de expressões: literais, identificadores, operadores, curto-circuito, condicional, cadeia `?.`, propriedade, índice, coleções, `is`/`as` | α (P1 closures, P4 `super`/cascata/extensões) e γ (P3 `switch` expressão); cada construto novo entra como **uma linha** no braço, que delega a um método no arquivo do dono |
| `comandos.rs` | `lower_stmt`, `try`/`catch`/`finally`, `assert` | γ (P3 `switch` comando) |
| `chamadas.rs` | chamadas a função de topo, local e estática, construtores e métodos do usuário | α (P1: chamada de valor de função) e, depois do merge de P1–P4, δ (P5d) |
| `sdk_por_nome.rs` | os membros do SDK casados pelo nome (`print`, `length`, `add`, `substring`, `Exception(…)`…) | **congelado**; δ apaga em P5d |
| `membros.rs`, `operadores.rs`, `mod.rs` | chamada de membro e despacho, operadores, ids e símbolos | β (P2) |
| `atribuicao.rs`, `locais.rs`, `verificador.rs` | atribuição, locais (R6), verificador da HIR (E3) | α (P1: captura e `Cell`) |
| novos | `captura.rs`, `closures.rs` (α); `padroes.rs`, `rti.rs` (γ); `async_sm.rs` (ε); `nativos.rs`, `sdk_modulo.rs` (δ) | |

**`crates/runtime/src/`** — o antigo `runtime_main.rs` (2.999 linhas) virou
sete **fragmentos**. Não são módulos Rust: são pedaços de um programa só, que
se enxergam sem `use`. A lista, na ordem de concatenação, é `FRAGMENTOS` em
`crates/runtime/build.rs` — a única. O `build.rs` grava o texto concatenado
(que o AOT compila com `rustc` avulso, `RUNTIME_MAIN`), o corpo do módulo
`abi` do JIT (um `include!` por fragmento, na mesma ordem) e a tabela de
símbolos, varrendo os `#[unsafe(no_mangle)]` de **todos** os fragmentos. Um
`.rs` em `src/` fora da lista derruba o build (`tests/fonte_unica.rs` confere
que o texto do AOT é o `heap.rs` seguido dos fragmentos, sem alteração).

| fragmento | conteúdo | dono (rodada 2) |
| --- | --- | --- |
| `nucleo.rs` | `main` C e `finalizar_programa`, `thread_local!` do heap e das classes, registro de classes e subtipos, objetos, igualdade, identidade, caixas (R3), `value_class`, records | δ (R10 `Smi`: caixas); ζ tira o estado para `isolado.rs` em P8 |
| `gc_raizes.rs` | frames e raízes (G), globais, coleta | δ |
| `excecoes.rs` | exceção pendente, `StackTrace`, construtores e acessores das classes de erro | δ (as classes de erro vêm da fonte em P5); os demais só acrescentam |
| `saida.rs` | `print`, `toString` e `describe_handle` dos valores do runtime | δ (P5d: `print` da fonte) |
| `strings.rs` | `String`, `StringBuffer`, `RegExp`, `parse` | δ (decisão 5: One/TwoByte) |
| `colecoes.rs` | listas, mapas, conjuntos, iterações ativas | δ |
| `closures.rs` | células, ambientes, closures | α (P1) |
| novos | `despacho.rs` (β), `tipos.rs` (γ), `eventos.rs` (ε), `isolado.rs` (ζ), `nativos/*` (δ) | quem cria acrescenta o nome a `FRAGMENTOS` (só acréscimo) |

**Arquivos compartilhados** (só acréscimo, cada agente num bloco com
comentário de cabeçalho): `llvm/externs.rs`, `FRAGMENTOS` em
`crates/runtime/build.rs`, o `struct FnBuilder`. `llvm/mod.rs`: α (closures) e
ζ (globais) mexem em funções diferentes; β põe o despacho em `llvm/despacho.rs`.
**Ordem de merge:** P0 → (P1, P2, P3, P5a–c) → P4 → P5d → (P6, P8) → P7 → P9.

### 7.4 δ: strings, `Smi` e P5a/P5b (medido)

**Decisão 5 — strings UTF-16 (a702ff0).** `Texto` (`runtime/src/heap.rs`):
`_OneByteString`/`_TwoByteString` canônicos; `length`, índices, busca,
`split`/`replaceAll`/`splitMapJoin` (os algoritmos do `_StringBase`) por
unidade; `print` troca o surrogate solto por U+FFFD, como o `Utf8::Encode` da
VM (medido: `EF BF BD`). Pesado 35849542217: nativo **50/223** (o programa 04
de surrogates passa), JIT 50/223 com 0 divergências, IR determinístico, JS
223/223; `--gc-stress` 50/223 (35849593802).

**R10 — `Smi` (4f0b236).** Pesado 35852784596: nativo 50/223, JIT 50/223 com 0
divergências; `--gc-stress` 50/223 (35852795093). Alocações, contadas exatas
pelo contador novo `smi_caixas_evitadas` do `DARTFORGE_GC_STATS` (antes =
`allocations` + evitadas), numa amostra de 12 aprovados: 45 833 → 45 800
(−0,1%; o código do usuário do corpus é tipado, quase não encaixota); num laço
de 100 000 `Object o = i`, 100 000 alocações a menos. O ganho grande é com o
SDK da fonte, genérico (`E`, `Object?`).

**P5a — sobreposição.** `sdk_nativo/libraries.json` (alvo
`dartforge_nativo`, base `vm`) troca por **conteúdo** quatro arquivos:
`async_patch.dart` (o modelo do dart2js sem `_SuspendState`),
`schedule_microtask_patch.dart` e `timer_patch.dart` (natives
`DartForge_scheduleImmediate`/`DartForge_Timer_*`) e `finalizer_patch.dart`
(validação da VM, callback nunca rodado — a especificação permite). O caminho
lógico continua o do SDK, então os `part` resolvem ao lado do original
(`SdkLayout::load_com_sobreposicao`, `elements/src/load.rs` lê o substituto;
o cache do SDK inclui as trocas na chave). O programa carrega sem diagnóstico.

**Medição de 5a** (`sdk_modulo::medir_inferencia_do_sdk`, com o pedido a
`crates/types` aplicado só localmente — NATIVO-PEDIDOS): **4 447
diagnósticos** de inferência nos corpos das sete bibliotecas da fonte —
`_internal` 635, `core` 2 294, `_compact_hash` 111, `collection` 420, `math` 71,
`convert` 391, `async` 525. Os mais comuns: "Nome indefinido" (1 381), argumento
não atribuível (763), retorno não atribuível (616), método/getter não definido
para o tipo. O aceite de 5a (zero) é trabalho da inferência, não do nativo; a
lista por código está no teste ignorado `medir_inferencia_das_bibliotecas_da_fonte`.

**Achado do inventário (P5b).** `nativos::inventario` percorre os `external`
sem patch das sete bibliotecas: **137 natives distintos**, 57 intrínsecos
(`vm:recognized`) e **43 `external` cujo patch não foi ligado** pelo
carregador (`patched_by` vazio): membros de classe com `@patch` —
`Object.==`/`hashCode`/`toString`, `Timer._createTimer` (que a sobreposição
patcheia), `_AsyncRun._scheduleImmediate`, `String.fromCharCodes`,
`double.parse`, `identical`… O `patched_by` só é preenchido para uma parte dos
patches; o lowering de P5d precisa dele para todos (é do dono de
`crates/elements`).

**P5b — natives.** `emit_native/src/nativos.rs`: os 137 natives em ordem, com
estado (`Runtime`/`Pendente`) e efeitos G8 conservadores; o teste recusa native
da fonte sem entrada e entrada que deixou de ser native. 45 já no runtime
(fragmentos `nativos_numeros` e `nativos_strings`, `dartforge_nativo_<Nome>`):
os `Integer_*FromInteger` (operandos trocados como na VM; deslocamento com a
regra do `ShiftOperationHelper`), `Smi`/`Mint_bitLength`/`bitNegate`, a
aritmética e as comparações de `Double`, `Double_toString` (o `ToShortest` da
VM: `1e+21`, `1e-7`, `100000000000000000000.0` — conferido contra a VM) e os de
`String` sobre o `Texto` (`String_getHashCode` é o `StringHasher` da VM,
conferido). Os de lista, mapa, `Object`, `RegExp` e tipos ficam `Pendente`:
dependem do layout de `_List`/`_GrowableList` (R11) e da RTI (P4).

**Portão de custo (§2.3 do plano).** Nada do caminho do programa mudou ainda:
o nativo carrega a seção `vm` sem a sobreposição até P5d, e nenhum objeto do
SDK é compilado. Linha de base para o portão (Pesado 35852784596, job JIT ×
AOT, 54 programas que terminam): AOT Clang + ligação **p50 142,4 ms** (p95
156,8), JIT até executar p50 42,2 ms, AOT total p50 167,0 ms. O custo a frio
do SDK fica para quando P5c compilar a primeira biblioteca.

**Não feito nesta rodada, e por quê.** P5c (objeto por biblioteca em cache
por blake3) e P5d (a troca, apagando `lower/sdk_por_nome.rs`) precisam que o
lowering compile os corpos do SDK — closures (P1), despacho (P2), `switch`
(P3), `super`/mixins/RTI (P4) —, que estão com α/β/γ; P5d é, pelo mapa, depois
do merge de P1–P4. O `RegExp` com `regress` exige tirar o runtime do `rustc`
avulso para uma `staticlib` do cargo (decisão 3), o que muda a distribuição do
runtime do AOT; fica com P9, como o plano já previa, e o casador atual está
isolado em `regexp_casa_em`/`regexp_proxima` (`runtime/src/strings.rs`).
### 7.5 P1–P4 (α): closures, símbolos estáveis, despacho, padrões, herança

O que entrou, as decisões e o porquê de cada uma. O placar medido fica em
§7.6.

**Closures (P1).** `lower/captura.rs` decide, antes de baixar cada função
(a de topo e cada closure, na hora dela), quais variáveis declaradas **nela**
moram numa `Cell`: as capturadas por uma função aninhada **e** atribuídas em
qualquer ponto (fora ou dentro de uma closure, antes ou depois da captura; o
nome de uma função local conta como atribuído — é ligado depois de a closure
existir, e é assim que ela chama a si mesma). Capturada e nunca atribuída, a
variável é copiada para o ambiente. É a divisão do `dartdevc`/`dart2js`, e é
conservadora (uma variável atribuída só antes da captura também vai para a
célula): a análise fina fica para quando a medição pedir. A variável de um
`for` clássico que mora numa célula ganha uma célula nova a cada volta,
copiada da anterior, antes das atualizações (a especificação do `for`); a
do `for-in` e as do corpo de um laço já nascem por volta.

A **convenção uniforme** de chamada de um valor função é a de chamada
dinâmica da VM: `i64 @<entrada>(i64 closure, ptr args, ptr desc)`, com os
argumentos todos `Ref` num vetor na pilha do chamador (posicionais e depois
nomeados, na ordem do descritor) e o descritor `[n_posicionais, n_nomeados,
hash(nome)…]` (FNV-1a de 64 bits do nome, com os nomes ordenados: o mesmo
em qualquer módulo). A entrada confere a aridade contra a assinatura da
função (`dartforge_args_casam`), preenche os padrões dos opcionais ausentes e
chama o **corpo** `i64 @<símbolo>(i64 env, i64 p0, …)`. O código de uma
closure é o índice da entrada em `@df_code_table` (o índice 0 é a entrada que
só retorna: `dartforge_closure_entry` deixa `NoSuchMethodError` pendente
quando o valor não é closure). O tear-off de função de topo ou estática é
canônico (`TearOff`, `identical(f, f)`); o de método de instância é uma
closure nova com o receptor no ambiente, e a entrada dele chama o membro com
o despacho do receptor. Os corpos de closure ainda não são inferidos
(`crates/types`, pedido em `docs/NATIVO-PEDIDOS.md`): neles tudo é `dynamic`,
e os nomes são refeitos pelo escopo léxico no lowering (`resolver_por_nome`:
local, membro da classe envolvente pela linearização, topo da biblioteca).

**Símbolos estáveis (P2).** O símbolo vem do **caminho** da declaração, nunca
de índices do `crates/elements`: `df.<biblioteca>.<dono>.<membro>`, cada
parte escapada (letras, dígitos e `_` ficam; o resto vira `$` e dois dígitos
hexadecimais por byte UTF-8 — o `.` separa, então o símbolo é injetivo). A
biblioteca é a URI (`dart:core`, `package:a/b.dart`) ou, para `file:`, o
caminho relativo ao diretório da biblioteca de entrada (o mesmo programa tem
os mesmos símbolos em qualquer checkout). O dono é a classe, `ext:<nome>`
para uma extensão, e vazio para o topo; o membro é o nome, o setter termina
em `=`, o construtor é `new` ou `new:<nome>`. Closures:
`<função>$clo<k>` (k conta as anônimas na ordem do texto) e
`<função>$<nome>` para as funções locais; as entradas somam `$ent`, `$tear`
(função) e `$tearm` (método). Globais: o getter preguiçoso é
`df.<caminho>`, o valor `dfg.<caminho>` e a bandeira `dfg.<caminho>$ok`.
`main` da entrada continua `dart_main`. O teste `t_id_simbolos_estaveis`
(`crates/emit_native/src/lib.rs`) prova que inserir uma função e uma classe
não muda nenhum outro símbolo e que outro diretório dá o mesmo IR. Os ids de
classe do runtime são a ordem desse caminho (a partir de 1, pulando a faixa
1000–1012 das classes de erro do runtime, que saem em P5d).

**Despacho (P2).** A chamada de membro com um só alvo continua direta; com
vários, o `switch` sobre a classe do receptor (mundo fechado). O receptor sem
tipo útil (`dynamic`, `Object`, corpo de closure) usa o **despacho por nome**
(`lower/despacho.rs`): as classes do programa que têm o membro (pela
linearização, com campos, getters, setters e os campos implícitos de enum)
viram casos do `switch`; o resto vai ao membro do SDK casado pelo nome
(congelado) ou, se ele não existe, a `NoSuchMethodError` em tempo de
execução. Os operadores sobre `num`/`dynamic` vão ao `operator` da classe do
programa ou a `dartforge_dyn_op` (tapa-buraco com a semântica da VM, marcado
para sair em P5 — `%` euclidiano, `~/` truncado, `/` sempre `double`), e o
`==` sobre referências chama o `operator ==` do programa (com um lado null
vale a identidade, §17.26). **A tabela global de despacho** (`@df.sel.*`,
deslocamento por seletor) **fica para P5c**: ela só é necessária quando
código compilado à parte (o módulo do SDK) chama um membro do programa; até
lá o `switch` em mundo fechado tem a mesma semântica e nenhum consumidor a
mais.

**`switch`, padrões e enums (P3).** `lower/padroes.rs` casa um padrão como
árvore de decisão sobre os testes que já existiam (tipo, `==` da constante,
comparação), com as variáveis ligadas à medida que o casamento avança;
`switch` como comando (casos vazios compartilham o corpo, `continue
rótulo`, `break`) e como expressão, `if-case`, declaração e atribuição por
padrão, `for-in` com padrão. Record com campo nomeado (literal e padrão) é
diagnóstico: o runtime não tem a forma (pedido a δ). O valor de um enum é um
objeto canônico num global preguiçoso (`index` e `_name` nas posições 0 e 1,
depois os campos declarados, criado pelo construtor que o valor escolhe);
`values`, `index`, `name` e o `toString()` `Enum.valor`.

**Herança e cascata (P4).** Cascata (`..`, `?..`) avalia o alvo uma vez; as
seções vão pelo despacho dinâmico (o alvo implícito ainda não tem tipo).
`super.m()`/`super.x`/`super.x = v`/`super op e` é chamada direta a partir do
que vem depois da classe na **linearização** (`membros::linearizacao`: a
classe, os mixins do último para o primeiro, a superclasse). Mixins **sem
cópia**: o código do mixin é compilado uma vez; os campos dele ficam no
layout de cada classe que o aplica, entre os da superclasse e os da classe, e
o índice é escolhido pela classe dinâmica de `this` (`switch`) — o mesmo
mecanismo do despacho, sem gerar uma cópia por aplicação. `super` dentro de
um mixin ainda é diagnóstico. Membros de extensão: o receptor é o primeiro
parâmetro, na representação do tipo `on`. Fábrica redirecionadora, campo
`late` escalar com inicializador (em caixa: null é "não inicializado") e a
chamada de valor função (`f()`, `obj.campo()`, `f.call()`, `(e)(…)`).

**Correções de caminho.** `try/finally` sem `catch`: as exceções do corpo
iam direto ao `finally` sem registrar a entrada do phi dele (o Clang recusava
o módulo); agora passam por um bloco de pouso que registra, e as exceções
dentro de um `catch` também passam pelo `finally` antes de subir. A exceção
que sai do `finally` sobe para o tratador mais interno (antes ia ao
`finally` de fora pulando o `catch` de fora).

**Também em P3/P4.** `const` canônico (`lower/constantes.rs`): a chave de uma
constante é o valor estrutural dela escrito como texto (`o:<construtor>(…)`,
`l<tipo>:[…]`, `i:2`…, com o tipo estático nas coleções — `const <int>[]` e
`const <String>[]` são objetos diferentes); cada chave é um global
preguiçoso `dfc.<hash>` e as coleções constantes saem imutáveis. Contexto
constante: inicializador `const` (de topo, estático ou local), valor padrão
de parâmetro e argumentos de `const C(…)`. Literais de coleção com `...`,
`...?`, `?e`, `if`, `for` e `for-in`, e o literal de conjunto (antes `{a, b}`
virava um mapa vazio). Num padrão de casamento, um nome solto é padrão
constante (`case base:`), e `const (e)` é a expressão. Records com campo
nomeado (`lower/registros.rs`): cada forma do programa é uma "classe" com
`toString` e `==` estrutural gerados. O `for-in` e o espalhamento leem lista
ou conjunto (`dartforge_iteravel_get_*`). `break`/`continue` (com rótulo)
que atravessam um `finally` passam por ele e continuam o salto; um salto para
um laço dentro do próprio `try` não passa. O corpo do `finally` roda com a
exceção guardada fora da pendência (antes a primeira chamada dele desviava).
Um global cujo inicializador lança volta a não inicializado.

**RTI ainda não.** `x is List<int>`, `case <int>[…]` e `List<int>()` num
padrão são **diagnóstico** ("teste de tipo genérico (RTI)"): responder pela
classe daria a resposta errada. É o item de P4 que falta, com o `super`
dentro de um mixin.

### 7.6 Placar da rodada 2 (α), medido no CI

| passo | commit | Pesado (run) | nativo | JIT | JIT × AOT |
| --- | --- | --- | ---: | ---: | --- |
| P0 (base) | 87be22b | 35836380647 | 50/223 | 50/223 | 0 divergentes |
| P1–P2 | b156dd2 | 35861971349 | 68/223 | 68/223 | — |
| P1–P4 parcial | b60a219 | 35863053512 | 74/223 | 74/223 | 0 divergentes |
| P3 (const, padrões) | 8c313a9 | 35866264097 | 81/223 | 81/223 | 0 divergentes |
| P4 (RTI como diagnóstico, cast pela classe) | a276d6c | 35871381320 | 81/223 | 81/223 | job verde |

Os 81 passam também com `--gc-stress` (coleta antes de toda alocação),
rodado localmente programa a programa sobre a lista do CI. O determinismo do
IR é idêntico com 1, 4 e 8 trabalhadores (88 programas com IR). JS 223/223
nos dois perfis.

O que sobra, pelo relatório de construtos: quase tudo é membro do SDK sem
implementação (`where`, `map`, `fold`, `toStringAsFixed`, `sort`,
`List.filled`/`List.generate`, `parse`, `hashCode`…) — P5, o SDK da fonte —,
`await`/`yield` (P6/P7) e o `toString()` de objeto do programa dentro de uma
coleção impressa (o runtime não chama código Dart; também P5).

### 7.7 P6 (`async`/`await`), RTI e `super` em mixin

O que entrou, as decisões e o porquê. O placar medido fica em §7.8.

**O `dart:async` vem da fonte (`crates/emit_native/src/fonte.rs`).** P6 é a
primeira parte da decisão 1 posta em prática: o programa que usa `dart:async`
(função `async`/gerador, `await`, o nome `Future`/`Stream`/`FutureOr` ou
`import 'dart:async'`) compila o `dart:async` da seção `vm` com a sobreposição
`sdk_nativo/` — e o `dart:_internal` (de onde ele usa `unsafeCast`,
`IterableElementError`…) e a `Duration` do `dart:core` (o `Timer` a recebe; o
runtime em Rust nunca a teve) —, **com os corpos inferidos**. Nenhuma classe
dessas bibliotecas tem representação própria no runtime: são objetos comuns
do heap, com layout, id de classe e símbolos estáveis (`df.dart$3aasync.…`).
Como a inferência pula toda biblioteca `is_sdk`, o nativo desliga `is_sdk`
delas na sua cópia do `Program` antes da inferência — o único efeito de
`is_sdk` no `crates/types` é esse (pedido ao dono em NATIVO-PEDIDOS: um
parâmetro que faça o mesmo). A `Duration` sai do `dart:core` para uma
biblioteca de mesmo URI e escopo (`separar_partes_do_core`): o resto do
`dart:core` continua sendo o do runtime. Quem não usa `dart:async` emite o
mesmo IR de antes (a regra de custo zero).

*Poda.* As bibliotecas da fonte entram inteiras (o mundo aberto de uma
biblioteca do SDK, como no módulo por biblioteca de P5c) e o módulo é podado a
partir do programa: fica a função que o programa alcança por chamada, closure,
tear-off ou `toString` de classe. *Construto que falta no código da fonte* vira
`UnsupportedError` em tempo de execução com o texto do diagnóstico
(`FnBuilder::nao_suportado`): a poda é conservadora (todo `toString`, todo
alvo de um despacho), e o programa que não passa por ali compila; o que passa
falha alto, nunca em silêncio. No código do programa continua sendo erro de
compilação (N1). `DARTFORGE_FONTE_NAO_SUPORTADO=1` lista esses pontos na
compilação.

*Natives e intrínsecos* (`lower/externos.rs`): o `external` da fonte com
`@pragma("vm:external-name", N)` tem por corpo a chamada a
`dartforge_nativo_N` (a tabela de δ, `nativos.rs`); `unsafeCast` é intrínseco
(o valor). As classes de erro que o runtime representa (1000–1012) e o
`List.filled` são construídos pelas externs que o runtime já usa
(`lower/erros_do_runtime.rs`) — o objeto tem de ser o do runtime, que é o que
`on ArgumentError` testa; saem com a faixa em P5d.

*Um defeito do carregador achado aqui* (`crates/elements`, corrigido em
commit próprio): a parte de um arquivo de patch (`core_patch.dart` →
`part "errors_patch.dart"`, a `timer_patch.dart` da sobreposição) era
carregada como parte comum, e a classe `@patch` dela virava outra classe com o
mesmo nome, que tomava o lugar da original — `Error` sem `throwWithStackTrace`,
`Timer` sem `periodic`. Eram os 43 `external` com `patched_by` vazio que δ
mediu.

**O corpo `async` (`lower/async_sm.rs`)** segue o dart2js
(`rewrite_async.dart`) e os apoios do `async_patch` de δ: o stub cria o
quadro (um objeto do heap), o `Completer`
(`_makeAsyncAwaitCompleter<T>`, `T` o tipo do valor do `Future`), a closure
do corpo registrada na zona (`_envolverCorpo`) e começa por
`_asyncStartSync`. O corpo `f$async(env, código, resultado)` salta pelo estado
guardado no quadro; `await e` grava o estado, chama `_asyncAwait(e, corpo)` e
retorna, e o bloco de retomada só é alcançado pelo `switch` da entrada. Com
`código == _ERRO` o erro é lançado no ponto do `await` com o rastro dele e
segue o caminho de exceção pendente de sempre (os `try`/`catch`/`finally` em
volta são os do lowering). Exceção que chega ao topo vai a `_asyncRethrow`;
`return v` vai a `_asyncReturn` (o gancho está em `FnBuilder::terminate`).

*O que atravessa um `await` mora no quadro*, por duas passadas sobre a HIR do
corpo pronto, que não dependem de como cada construto foi baixado: todo
`alloca` vira posição do quadro; todo valor SSA vivo na entrada de uma
retomada (vivacidade com a aresta virtual suspensão → retomada) é gravado no
quadro logo depois de definido e relido antes de cada uso — o que o LLVM faz
no *coroutine frame* (`CoroSplit`), sem reconstruir o SSA. O quadro é
alcançado pelas arestas do heap (contrato G): a pilha-sombra é desmontada a
cada suspensão.

**O laço de eventos (`crates/runtime/src/eventos.rs`)** roda depois do
`main` (o `dartforge_entry` o chama só quando o programa usa `dart:async`):
microtarefas (as closures de `DartForge_scheduleImmediate`, que o
`_startMicrotaskLoop` da fonte agenda) todas antes de qualquer timer; timers
na ordem da VM (`timer_impl.dart`): prazo `agora` para duração 0 e
`agora + 1 + ms` para as outras, desempate pela sequência de agendamento,
periódico reagendado em `prazo + ms` depois do callback. É o único ponto em
que o runtime chama Dart, por uma função do código gerado que chama uma
closure sem argumentos (`dartforge_chamar_dart0`; G8 marca
`dartforge_laco_de_eventos` com `chama_dart`). Exceção que sai de uma
microtarefa já passou pela `Zone` (`_rootHandleError` a relança): o laço para
e `finalizar_programa` a relata. O estado é por thread (um isolado por
thread; P8 o junta em `isolado.rs`), e as closures que esperam são raízes do
coletor (globais de raiz de id negativo).

**RTI (`crates/runtime/src/tipos.rs`, `lower/rti.rs`)**, no desenho do dart2js
(`rti.dart`): um universo canônico de tipos por isolado (hash-consing: o mesmo
tipo tem o mesmo id); o compilador descreve cada tipo por uma **receita**
(texto curto) com um global preguiçoso por receita; variáveis `P<i>`
(parâmetro de tipo da classe do código corrente, lido do tipo de `this` visto
como a classe declarante) e `M<i>` (argumento de tipo da função corrente) são
trocadas no ambiente por `dartforge_rti_avaliar`; as regras de supertipo de
cada classe citada (o fecho) são registradas na entrada. O tipo de cada
objeto genérico, coleção com tipo de elemento e closure (a assinatura, o
`$signature` do dart2js) mora num metadado por slot do heap (o `metadata_ptr`
reservado do cabeçalho). A função genérica e a fábrica de classe genérica
recebem a tupla dos argumentos de tipo no último parâmetro; na chamada, a
tupla vem dos argumentos escritos ou, na falta deles, casando o retorno
declarado com o tipo estático da chamada (e cada parâmetro com o argumento) —
a inferência não grava os argumentos inferidos (pedido ao dono). `is`/`as`,
`on T`, padrões de tipo e de coleção com argumentos vão pelo RTI quando a
anotação precisa (argumentos não triviais, variável de tipo, `FutureOr`, tipo
de função ou de record, typedef); a classe sem argumentos continua pelo teste
de classe. **Saiu o atalho do `as`**: o cast confere o tipo inteiro e lança
`TypeError` com a mensagem da VM. `Type` é um objeto canônico por tipo
(literal de tipo e `runtimeType`), com o texto da VM.

**`super` dentro de mixin (`lower/heranca.rs`).** O alvo é o que vem depois
do mixin na linearização da classe **dinâmica** de `this` (especificação
§12.3: cada aplicação tem a sua superclasse); como o código do mixin é
compilado uma vez, o alvo é escolhido pela classe (`switch`), o mesmo
mecanismo dos campos de mixin.

**`--gc-stress` no CI.** Um job novo do `pesado.yml` roda o corpus nativo
inteiro sob `DARTFORGE_GC_STRESS=1`, e o `nativo-placar` reprova a rodada se um
programa passa sem estresse e falha com ele (o portão G7).

**O que ficou de fora.** Streams (84) e o `Future.forEach` (88) param no
protocolo de `Iterable` das coleções do runtime (`iterator` de uma lista do
runtime): é o SDK da fonte das coleções (P5d). Os geradores (P7: 85, 86, 87,
110, 119, 185) precisam da API de `Iterable` do `dart:core` sobre o iterável
do `sync*` — também P5d. `forEach`/`map`/`fold`… sobre listas do runtime no
código do programa (89b, 212) continuam P5.

### 7.8 Placar de P6/RTI, medido no CI

| passo | commit | CI | Pesado | nativo | JIT | `--gc-stress` |
| --- | --- | --- | --- | ---: | ---: | ---: |
| base (main) | 48248d5 | — | 35890465508 | 81/223 | 81/223 | 81 (local) |
| P6 + RTI + `super` em mixin | fe14953 | 35899797501 (vermelho: `sdk_cache`) | 35899797550 | 90/223 | 90/223 | 90/223 |
| cache do SDK com o papel de patch | 7f226e9 | 35901304602 | 35901304619 | 91/223 | 91/223 | 91/223 |
| `TypeError` com mensagem, testes | c259fc5 | 35904857783 | 35904857766 | **91/223** | **91/223** | **91/223** |

Nenhuma regressão contra o main (conjunto de falhas comparado programa a
programa); passam a mais: 80, 81, 82, 83, 89, 181 (`async`), 214 e 219 (RTI),
06 e 105 (a correção dos patches). JIT × AOT sem divergência, determinismo do
IR idêntico com 1, 4 e 8 trabalhadores, JS 223/223 nos dois perfis, o portão
de custo zero verde. O job novo `--gc-stress` roda o corpus inteiro (31 s no
runner) e o `nativo-placar` confirma: nenhum programa que passa sem estresse
falha com ele. IR de um programa `async` típico (83): 1,2 MB, a maior parte do
`dart:async` alcançado.

### 7.9 δ: o SDK da fonte compilado (P5c) e a troca (P5d), medido

**Como o SDK entra.** Com `DARTFORGE_SDK_DA_FONTE=1` (opt-in até o placar
do SDK da fonte passar o de hoje), as oito bibliotecas da fonte (`core`,
`async`, `collection`, `convert`, `math`, `_internal`, `_compact_hash`,
`typed_data`, com os patches `vm` e a sobreposição `sdk_nativo/`) são
baixadas **cada uma no seu módulo**, em mundo aberto, com os símbolos
estáveis `df.<biblioteca>.<dono>.<membro>`, e compiladas uma vez por
conteúdo: a chave é o blake3 das fontes do SDK e da sobreposição, das fontes
do compilador (`crates/emit_native/build.rs`: emit_native, types, elements,
frontend, runtime), da identidade do Clang e das bandeiras
(`native_cache/sdk/<chave>/`, `recusados.tsv` ao lado). O programa é
compilado como sempre (os corpos do SDK **não** são inferidos nem baixados
por programa) e referencia os símbolos do SDK.

**Dois perfis** (exigência do proprietário, 2026-09-23):

* **desenvolvimento, teste, CI e JIT** — o SDK e o runtime numa **DLL em
  cache** (`dfsdk_<chave>.dll`, com a biblioteca de importação); o
  executável liga só o objeto do programa e a importação, e a DLL vai ao
  lado dele por ligação física. Motivo, medido: ligar os objetos do SDK
  (~12 MB) em cada executável custava ~450 ms por programa, contra ~60 ms da
  ligação de antes; com a DLL a ligação pareada ficou abaixo da de antes
  (132 ms × 198 ms na mesma máquina, mesma hora);
* **produção** (`optimize`, `dartforge aot --optimize`) — **um executável
  autocontido**, como o `dart compile exe`: o SDK em bitcode ThinLTO `-O2`
  (outra entrada do mesmo cache por blake3, compilada uma vez), o programa
  em bitcode ThinLTO, o runtime estático, ligados pelo `lld` com ThinLTO e
  `/OPT:REF`. O teste `producao_e_um_executavel_autocontido` copia só o
  `.exe` para uma pasta vazia e o executa (sem nenhuma DLL do DartForge).

**Poda (mundo fechado pela ligação).** A tabela de métodos de uma classe é
registrada **na primeira alocação** de um objeto dela
(`dartforge_object_new_t` recebe a função `df.mt.<biblioteca>.<Classe>`
que devolve a tabela); só as classes dos valores do runtime (`_Smi`,
`_OneByteString`, `_GrowableList`…) são registradas pela entrada. Uma
classe que o programa nunca instancia não tem a tabela alcançada, e o
ligador tira a tabela, os adaptadores e os métodos que só ela alcançava.

**Tamanho do executável de produção** (medido; o mesmo programa com o
`dart compile exe` do SDK 3.6.2):

| programa | DartForge (produção, ThinLTO) | `dart compile exe` |
| --- | ---: | ---: |
| hello world (`print('Olá, mundo!')`) | 2 486 784 B | 5 796 864 B |
| médio (corpus `64_map_ordem_insercao`) | 2 528 768 B | 5 839 360 B |

**Despacho: tabela por classe, não a tabela global.** A *global dispatch
table* da VM AOT escolhe os deslocamentos vendo todas as classes; com o SDK
compilado antes do programa, os deslocamentos do SDK não podem depender das
classes do programa, e cada programa teria de redefinir um global por
seletor que o SDK usa (milhares) — ou o COFF teria de resolver símbolos
fracos, que ele não resolve como o ELF. É a solução do JIT da VM: cada
classe tem a tabela (hash FNV-1a de 64 bits do seletor → entrada uniforme),
e cada ponto de chamada tem um cache (id de classe, entrada). O seletor é
`c:m` (chamar), `g:x` (ler), `s:x` (gravar), com `@<biblioteca>` nos nomes
privados. A chamada é direta quando só a biblioteca da classe pode
sobrescrever o membro e ele tem uma implementação que é método
(`sdk_fonte::implementacoes`).

A entrada uniforme guarda a tupla RTI do método genérico num slot oculto
depois dos argumentos posicionais e nomeados; o descritor de aridade continua
contando só os argumentos Dart. O adaptador passa essa tupla à função real.
O `T` de uma classe genérica, por sua vez, é avaliado pelo RTI do receptor.
Essa combinação permite que `Iterable.whereType<T>` filtre por `T` mesmo
quando a chamada e `WhereTypeIterator<T>.moveNext` passam por seletores.

**O que é nosso na sobreposição** (além do de 7.4): `print_patch.dart`
(`printToConsole` como native, e os erros que o runtime lança construídos
como os objetos da fonte, `_dartforge*`), `string_buffer_patch.dart` (o
`StringBuffer` por partes; o da VM usa `Uint16List` e um native de criação),
`compact_hash.dart` (os campos que a VM injeta em `_HashVMBase` como campos
de verdade; o resto é o arquivo do SDK), `regexp_patch.dart` (o `_RegExp`
sobre o motor do runtime, `crates/runtime/src/regexp.rs`) e
`isolate_patch.dart` (portas e capacidades sobre `crates/runtime/src/
portas.rs`).

**`typed_data` é o da VM, sem sobreposição.** As listas tipadas internas
(`_Uint8List`…) e as visões (`_Uint8ArrayView`, `_ByteDataView`…) são formas
do heap (`Value::TypedData`/`TypedView`): bytes no endian do hospedeiro, um
byte por elemento de `Uint8List`. Os intrínsecos (`vm:recognized`) — as
fábricas, `_getX`/`_setX`, `[]`, `_memMoveN` — e os natives `TypedData*` são
funções de `crates/runtime/src/typed_data.rs`; a fábrica registra a tabela
de métodos da classe na primeira alocação. `ByteBuffer`, `ByteData`, visões
e visões não modificáveis funcionam; o SIMD (`Float32x4`…) ainda é recusado
por membro. O nome em `Type.toString()` segue o `Class::UserVisibleName`
da VM (`_Uint8List` → `Uint8List`, `_GrowableList` → `List`).

**`dart:io` é o da VM.** Os patches de `_internal/vm/bin` compilam sem
mudança; a sobreposição só troca `common_patch.dart` (o arquivo da VM mais
as funções `_dartforge*` que o runtime chama para criar `OSError`, as
entradas da listagem síncrona e o preparo da partida) e
`nativewrappers.dart` (o campo nativo de `NativeFieldWrapperClass1` vira um
campo declarado, o primeiro do layout, que os natives leem e gravam). Os
natives seguem o contrato de `runtime/bin/*.cc` — devolvem o valor ou um
`OSError`, com os mesmos códigos de erro (`SetErrno` de `file_linux.cc`) —
e estão em `crates/runtime/src/io_arquivos.rs` (arquivos, o `File` com
contagem de referências e finalizador no coletor), `io_diretorios.rs`
(diretórios e a listagem, síncrona e em lotes), `io_servico.rs` (o
IOService: uma porta nativa atendida por até 32 threads, com os pedidos e as
respostas do `IO_SERVICE_REQUEST_LIST`) e `io_plataforma.rs` (`Platform`,
`Stdin`/`Stdout`, `exit`, `exitCode`, `sleep`, bytes aleatórios e o preparo
que o embedder faz antes do `main`: `Platform.script` e `Uri.base`). Cada
native tem a versão Unix (Linux e macOS) e a do Windows. `dart:developer`
segue o perfil de produção da VM (`nativos_desenvolvedor.rs`).

O manipulador de eventos segue o da VM, com um backend próprio por sistema,
sem fingir que um é o outro. O contrato comum é o do `_NativeSocket`: a
máscara de eventos (`kInEvent`, `kOutEvent`, `kCloseEvent`, `kErrorEvent`,
`kDestroyedEvent`) postada na porta do soquete, as fichas de controle de
fluxo (`InfoDeDescritor`, 16 por soquete e 4 por porta num soquete de escuta
compartilhado), a leitura e a escrita parciais (0 = nada agora) e o
fechamento em dois tempos (o comando de fechar, depois `kDestroyedEvent`).

* **Linux e macOS** (`io_eventos.rs`): uma thread com epoll (Linux) ou
  kqueue (macOS), por prontidão e com disparo por borda; os natives leem e
  escrevem direto no descritor não bloqueante (`io_soquetes_unix.rs`).
* **Windows** (`io_windows_eventos.rs`): uma porta de conclusão (IOCP)
  atendida por uma thread, com E/S sobreposta, como o `eventhandler_win.cc`
  da VM. O "descritor" é um `Manipulador` (o `Handle` da VM); ele mantém uma
  leitura emitida (`WSARecv`, `WSARecvFrom`, `ReadFile`) e cinco `AcceptEx`
  num soquete de escuta; o que chegou fica no manipulador até o Dart ler, e
  a leitura que esvazia o buffer emite a seguinte. A escrita copia até 64
  KiB e emite `WSASend`/`WriteFile`; enquanto ela não conclui, o soquete
  escreve 0 (`Socket_HasPendingWrite` = verdadeiro). A conexão sai por
  `ConnectEx` e o fechamento de um cliente por `DisconnectEx`. Os comandos
  do Dart chegam pela mesma porta (`PostQueuedCompletionStatus`). A única
  thread auxiliar é a da entrada padrão, que não aceita E/S sobreposta
  (console): ela faz o `ReadFile` síncrono e posta a conclusão.
  Os soquetes são criados com `WSA_FLAG_NO_HANDLE_INHERIT`.

**Posse no Windows.** Cada operação emitida é uma `Operacao` no heap (o
`OVERLAPPED` é o primeiro campo) que carrega o buffer e uma referência
(`Arc`) do `Manipulador`; ela passa ao sistema na emissão e volta na
conclusão, que é sempre entregue pela porta — também quando a chamada
conclui na hora e quando o fechamento a aborta (`ERROR_OPERATION_ABORTED`).
Assim buffer e manipulador vivem até a última conclusão, mesmo depois de o
objeto Dart fechar ou ser coletado; o `kDestroyedEvent` só sai quando não há
operação pendente. O objeto Dart (`SoqueteNativo`) é dono de uma referência
do manipulador (soquetes de escuta compartilhados: uma por objeto), solta no
fechamento (`CloseFd`) ou com o objeto.

**Soquete de escuta compartilhado.** Só o último objeto fecha o soquete do
sistema (`CloseSafe` do registro); os outros soltam a própria referência.
Regressão em `crates/cli/tests/io_regressao.rs`.

**Sinais.** No Unix, cada inscrição é um pipe **não bloqueante** nos dois
lados: com o pipe cheio o tratador descarta o byte (a notificação se funde
às pendentes, como o sistema funde sinais), sem nunca travar a thread
interrompida — a VM do Dart trava nesse caso, por isso a regressão é da CLI e
não do corpus diferencial. O tratador preserva o `errno` e só usa atômicos;
o cancelamento espera os `write` em andamento (`em_uso`) antes do `close`,
e fechar o soquete do sinal desfaz a inscrição (`ClearSignalHandlerByFd`).
No Windows os sinais são os eventos de console (`SetConsoleCtrlHandler`:
SIGINT = Ctrl+C, SIGHUP = fechamento), escritos num pipe sobreposto.

Sobre isso, `io_soquetes.rs` (os natives de TCP, UDP, domínio Unix,
`InternetAddress` e a resolução de nomes do IOService — o que basta para
`HttpServer` e `HttpClient` —, iguais nos três sistemas) e `io_processos.rs`
(no Unix, `fork` + `execvp` com os pipes e o pipe de controle da VM, a
thread que espera os filhos, `runSync`, `killPid`, os sinais e
`ProcessInfo`). No Windows (`io_windows_processos.rs`), `CreateProcessW`
com só os três handles de E/S herdados (`PROC_THREAD_ATTRIBUTE_HANDLE_LIST`),
pipes nomeados sobrepostos do lado do pai e síncronos do lado do filho, e o
código de saída pelo pool do sistema (`RegisterWaitForSingleObject`). O
Windows não tem soquetes de domínio Unix no `dart:io` (o `OSError` da VM).
O `FileSystemWatcher`, o SIMD e as mensagens de controle de soquete
(`SCM_RIGHTS`) são recusados por membro. O corpus `corpus/nativo/` (só VM ×
nativo) cobre o `dart:io`: arquivos, diretórios, processos, TCP, HTTP, UDP,
sinais e escuta compartilhada.

**Validação por sistema.** `cargo check --target` só filtra erros de
compilação; o que vale é o CI de cada sistema (build, testes do emissor e do
JIT, `io_regressao` e o corpus nativo). A camada Windows ainda precisa do
corpus `corpus/nativo` rodando no runner Windows e de medições sob carga
(vazão, latência, CPU, memória e threads com muitas conexões, consumidores
lentos e processos com muita saída).

**Recusa por membro.** O membro do SDK que não baixa (construto não
suportado, native pendente, intrínseco da VM sem entrada, teste de tipo sobre
parâmetro de tipo antes da RTI) vira uma função que avisa em tempo de
execução — `erro: membro do SDK não suportado no backend nativo: <símbolo>
(<motivo>)`, código 254 — e entra em `recusados.tsv`. Nunca uma saída
errada.
