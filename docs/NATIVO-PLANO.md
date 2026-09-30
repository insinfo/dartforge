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

* **G1.** O emissor abre, na entrada de toda função que tem algum valor
  `Ref`, um **quadro de raízes no stack dela** — `{ anterior, N, [N x i64] }`
  (a pilha-sombra do `ShadowStackGC` do LLVM), com os slots zerados — e o
  encadeia com `dartforge_gc_empilhar(quadro)`; a coleta percorre a cadeia
  da thread. Cada valor `Ref` tem um **slot fixo**: cada valor SSA `Ref`
  (parâmetro, resultado de chamada, `Load`, `phi`, alocação) é gravado no
  slot dele com um `store` comum logo depois de ser definido — o parâmetro
  logo depois do prólogo, o `phi` depois do último `phi` do bloco. Antes
  eram três chamadas ao runtime (`push_frame`, que alocava um vetor por
  ativação, `set_root` por valor e `pop_frame`); o `store` não impede o
  LLVM de tirar dos laços as funções puras do runtime (`typed_len`).
* **G2.** Cada `alloca` de tipo `Ref` também tem um slot, e **todo `Store`
  nele é seguido de `set_root`** com o valor gravado. O plano original
  enraizava só valores SSA; isso não basta, porque um local vive mais que o
  SSA que o gravou: em `if (i == 0) saved = current;` dentro de um laço, a
  segunda volta redefine o SSA de `current` e sobrescreve o slot dele —
  sem o slot do `alloca`, `saved` ficaria sem raiz.
* **G3.** `dartforge_gc_desempilhar(quadro)` antes de **todo** `ret`, inclusive
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
closure é o endereço da entrada uniforme (`ptrtoint`, emissão de
`AllocClosure` em `crates/emit_native/src/llvm/mod.rs`), para que uma closure
criada no módulo do SDK seja chamada do módulo do programa
(`docs/NATIVO-PEDIDOS.md`); quando o valor não é closure,
`dartforge_closure_entry` deixa `NoSuchMethodError` pendente e devolve 0, que
o ponto de chamada troca pela entrada `@df_clo_invalido`. *Histórico (até
2026-09-27):* o código era o índice na `@df_code_table`, com o índice 0 como
entrada que só retorna. O tear-off de função de topo ou estática é
canônico (`TearOff`, `identical(f, f)`); o de método de instância é uma
closure nova com o receptor no ambiente, e a entrada dele chama o membro com
o despacho do receptor. Os corpos de closure são inferidos pelo motor de
`crates/types/src/inferencia` (resposta ao pedido em `docs/NATIVO-PEDIDOS.md`,
«De α (P1–P4) para a inferência»); o `resolver_por_nome` do lowering
(`lower/closures.rs:791`: local, membro da classe envolvente pela
linearização, topo da biblioteca) continua como apoio para o nome sem
resolução gravada. *Histórico (até 2026-09-27):* aqui se lia que os corpos
de closure ainda não eram inferidos e que neles tudo era `dynamic`.

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

**Isolados** (`isolados.rs`, `portas.rs`). Um isolado é uma thread com o
próprio heap, área de globais, universo de tipos e laço de eventos. A
entrada gerada separa `df.preparar_isolado` (registros das bibliotecas,
RTI, embedder), que o runtime roda em cada thread nova, e registra a
chamada de closure (`dartforge_registrar_isolados`). `Isolate.spawn`
copia a entrada e a mensagem para grafos; na thread nova, o isolado
manda a mensagem de pronto (`[porta de controle, [pausa, término]]`) e
roda a entrada como a primeira mensagem. Numa mensagem para outro
isolado:

* constantes canônicas (`const`, valores de enum, globais `const`) vão
  pelo getter que as produz (`dartforge_marcar_constante`: o endereço do
  getter é o mesmo em todos os isolados) e tear-offs de topo pelo código —
  o destino recebe as dele, e `identical`/`switch` valem entre isolados,
  como no grupo de isolados da VM;
* os tipos (RTI) dos objetos vão numa tabela própria e são reinternados no
  destino (os ids de tipo são por isolado), inclusive os objetos `Type`;
* as tabelas de métodos das classes vão junto (o destino pode nunca ter
  criado um objeto da classe);
* `TransferableTypedData` é movido: a origem fica vazia.

A porta de controle recebe as mensagens OOB da VM (`pause`, `resume`,
`ping`, `kill`, ouvintes de saída e de erro, erros fatais), atendidas
entre eventos antes das comuns. Um erro não tratado vai aos ouvintes de
erro; sem ouvinte e com erros fatais, é impresso (`Unhandled exception:`)
e o isolado termina. `Isolate.exit` desenrola o isolado com uma exceção
que nenhum `catch` recebe (`dartforge_exception_capturavel`). Limites:
o `kill` imediato só é visto quando o isolado volta ao laço de eventos (a
VM interrompe na verificação de pilha); `Isolate.spawnUri` não existe
num programa compilado (a porta de pronto recebe o erro).

**dart:ffi** (`lower/ffi.rs`, `runtime/src/ffi.rs`, `sdk_nativo/ffi/`).
O que o transformador de FFI da VM faz no kernel, aqui é feito no
lowering, e a chamada nativa é LLVM puro — sem libffi e sem interpretar a
assinatura a cada chamada:

* **Tipos nativos.** Os primitivos (`Int8`…`Double`, `Bool`, `Void`), o
  `Pointer` e os inteiros específicos da ABI (`Long`, `IntPtr`, `Size`,
  `WChar`…, do SDK ou do programa) ganham um tipo C; os da ABI saem do
  `@AbiSpecificIntegerMapping` para o alvo (`linuxX64`, `windowsX64`,
  `macosArm64`…). A tabela vai ao runtime (`dartforge_ffi_registrar_tipo`)
  e serve a `sizeOf<T>()` e à chave do trampolim.
* **Trampolins.** Todo tipo de função feito só de tipos nativos que aparece
  no programa é uma assinatura: `lookupFunction<NS, DS>` e `asFunction`
  instanciam `NativeFunction<NS>` dentro do corpo genérico, então a
  assinatura concreta só existe como argumento de tipo. Cada uma ganha um
  trampolim na convenção uniforme das closures: confere a aridade, converte
  os argumentos (`int` truncado ao inteiro estreito com o `signext`/`zeroext`
  que a ABI C exige, `double` a `float`, `Pointer` ao endereço), chama o
  endereço guardado na closure (`Instruction::ChamadaNativa`) e converte o
  retorno. `asFunction` pede ao runtime a closure da chave da assinatura
  (letras dos tipos C, a mesma dos dois lados).
* **Memória.** `Pointer` guarda o endereço como `int`; as cargas e
  gravações (`_loadInt8`…`_storeDouble`, os da ABI, `_memCopy`) são natives
  do runtime que aceitam como base um `Pointer` ou uma lista tipada (com
  conferência de limites). `DynamicLibrary` usa `dlopen`/`dlsym` (Linux,
  macOS) e `LoadLibraryW`/`GetProcAddress` com busca nos módulos do
  processo (Windows).
* **Também baixados** (cada um com programa em `corpus/nativo/`, comparado
  com a VM pelo `pesado.yml`): layout de `Struct`/`Union` e `asTypedList`
  (`12_ffi_structs.dart`), `Pointer.fromFunction` e `NativeCallable`
  (`13_ffi_callbacks.dart`; redirecionamento em `lower/ffi.rs:390-409`),
  `NativeFinalizer` (`14_finalizadores.dart`), structs e unions por valor
  (`16_ffi_structs_por_valor.dart`), funções variádicas com `VarArgs`,
  inclusive com structs na parte variádica (`17_ffi_varargs.dart`), `Handle` (`18_ffi_handle.dart`) e `@Native` de
  função e de variável (`19_ffi_native_variaveis.dart`;
  `lower/ffi.rs:1283-1284`). O que continua recusado com o motivo, nunca com
  a ABI errada: inteiro específico da ABI sem mapeamento para o alvo, struct vazia por valor e assinatura nativa
  com parâmetros opcionais, nomeados ou genéricos (`lower/ffi.rs:104-190`).

  > **Histórico (até 2026-09-27).** Este item listava como pendentes structs
  > e unions por valor, `Pointer.fromFunction`, `NativeCallable`, `@Native`,
  > `asTypedList`, `NativeFinalizer`, `Handle` e funções variádicas; todos
  > entraram depois (evidências acima).

**Extensões genéricas.** Um membro de instância de extensão recebe a tupla
de argumentos de tipo `[os da extensão…, os do membro…]`: os da extensão
saem do tipo estático do receptor casado com o `on` (como na aplicação de
extensão da especificação), então `T` vale como expressão, em `is`/`as` e
dentro de closures. Setters, `[]`/`[]=` e operadores de extensão baixam
como chamadas diretas; o `@patch` de um membro de extensão do SDK
substitui o `external` da declaração. Uma variável de tipo como expressão
(`T` numa função ou classe genérica) produz o objeto `Type` do argumento
corrente.

**Tipos de extensão (3.3).** Apagados: o valor **é** o da representação
(`Id(3)` é o `int` 3), sem classe no heap. `apagamento.rs` calcula, antes do
contexto (a tabela de tipos ainda aceita tipos novos), o apagamento de cada
tipo da tabela, o tipo de `this` de cada tipo de extensão (`E<T…>`) e a
instância de cada supertipo de extensão (`Filho<int>` implementa
`Base<List<int>>`). O contexto só entrega tipos apagados (`get_type`,
`tipo_local`, `to_hir_type` e as receitas de RTI, também as das anotações):
representação, caminhos rápidos, `is`/`as` e `runtimeType` veem a
representação. A rota dos membros é a resolução da inferência
(`Resolved::Member` num tipo de extensão), sempre antes dos caminhos pelo
tipo apagado (um `length` do tipo de extensão não é o da lista que ele
representa; intrínsecos e SIMD também cedem): o membro de instância é uma
função com a representação como primeiro parâmetro, como o de uma extensão,
com os argumentos de tipo do tipo de extensão na tupla, tirados do tipo
estático **não apagado** do receptor (`get_type_bruto`), inclusive nos
tear-offs (a assinatura) e nos padrões de objeto (os argumentos escritos).
A representação é o próprio receptor; o construtor primário é o valor do
argumento; os demais (generativos com `this.v`, `v = e` ou `this(…)`, e as
fábricas, redirecionadoras inclusive) são funções que devolvem a
representação, chamadas como fábricas. `lower/tipos_de_extensao.rs`;
`corpus/nativo/36` e `corpus/js/230`–`232` iguais à VM.

**Recusa por membro.** O membro do SDK que não baixa (construto não
suportado, native pendente, intrínseco da VM sem entrada, teste de tipo sobre
parâmetro de tipo antes da RTI) vira uma função que avisa em tempo de
execução — `erro: membro do SDK não suportado no backend nativo: <símbolo>
(<motivo>)`, código 254 — e entra em `recusados.tsv`. Nunca uma saída
errada.

### 7.10 Distribuição sem Rust e sem LLVM na máquina de quem usa

Quem usa o dartforge instala só a distribuição. Três peças saíram da máquina
do usuário para o build do dartforge:

* **Runtime pré-compilado** (`crates/runtime_estatico`,
  `crates/emit_native/build.rs`). O runtime é a `staticlib` de um crate do
  Cargo — o módulo `abi` de `dartforge-runtime` no perfil `aot` (com o
  `main` C) ou `dll` (a biblioteca compartilhada do SDK da fonte) —, e por
  isso pode ter dependências (a TLS do `dart:io`) numa cópia só da
  biblioteca padrão do Rust. O `build.rs` do emit_native compila as duas
  variantes com um `cargo build` próprio (outro diretório de alvo), com
  `--remap-path-prefix` para as mensagens de pânico não levarem o caminho de
  quem compilou, e as publica como `dartforge_runtime_<blake3>`/
  `dartforge_rtdll_<blake3>` (o hash cobre o fonte do runtime, o
  `Cargo.lock`, o alvo e a variante). O AOT procura em `lib/` da
  distribuição (`<raiz>/bin/dartforge` → `<raiz>/lib/`, ou `DARTFORGE_LIB`),
  depois no diretório do build; não há mais compilação com o `rustc` da
  máquina. `DARTFORGE_RUNTIME_SEM_PRECOMPILAR=1` pula a etapa em `cargo
  check`/`clippy`.
* **Gerador de objetos embutido** (`crates/llvm`, `src/gerador.rs`, feature
  `llvm-embutido`, ligada pelo `jit` da CLI). O LLVM ligado ao dartforge lê
  o IR, roda o `default<O0>`/`default<O2>` do `PassBuilder` e emite o objeto
  com a CPU que o Clang assume sem `-march` (`x86-64`, `apple-m1`,
  `generic`) e código independente de posição fora do Windows — o que o
  `clang -x ir -c` fazia, sem processo por módulo e sem gravar o IR.
  `DARTFORGE_GERADOR=clang` volta ao Clang. A chave dos caches leva a
  identidade do gerador (versão do LLVM, triple, CPU).
  Produção: a API C do LLVM não escreve o resumo do ThinLTO, então o
  bitcode (`lto-pre-link<O2>`) vai para a **LTO completa** do `lld`, com a
  geração de código em partições paralelas (`--lto-partitions`,
  `/opt:lldltopartitions`). Medido no Linux x86-64 (programa pequeno com o
  SDK da fonte): ligação de produção 15,0 s contra 13,0 s do ThinLTO pelo
  Clang; SDK de desenvolvimento frio 4,7 s; corpus nativo 225/225 com os dois
  geradores, JIT × AOT 225/225.
* **Pendente**: a ligação ainda passa pelo driver do Clang (que acha o
  `crt1.o`, a libc e o SDK do sistema). O próximo passo é o `lld` da
  distribuição chamado direto, com as bibliotecas do sistema resolvidas pelo
  dartforge; os caminhos fixos de desenvolvimento (`C:/tools/dartsdk…`, o
  cache em `target/`) passam para a distribuição (o SDK Dart e o
  `sdk_nativo/` em `lib/`, o cache no diretório de cache do usuário).

### 7.11 TLS: `SecureSocket`, `SecurityContext`, `X509Certificate`, HTTPS

O runtime (`crates/runtime/src/tls.rs`) implementa o filtro da VM
(`runtime/bin/secure_socket_filter.cc`) sobre o **rustls** (provedor
`ring`); o `secure_socket.dart` e o `dart:_http` do SDK rodam intactos por
cima. A sobreposição troca só o patch da VM
(`sdk_nativo/io/secure_socket_patch.dart`) e o pedido `sslProcessFilter` do
IOService (`io_service_patch.dart`).

* **Os anéis.** Os quatro `_ExternalBuffer` (texto lido/a escrever, cifrado
  lido/a escrever) são `Uint8List` do heap; o `ProcessAllBuffers` da VM é
  reproduzido com as mesmas regras de anel (um byte sempre livre). O pedido
  é atendido na thread do isolado — o filtro só faz criptografia em
  memória; a E/S continua no `RawSocket` — e responde **num evento seguinte**
  do laço (como a porta do IOService): responder numa microtarefa deixava o
  `_tryFilter` girar sem ceder ao laço, e um servidor no mesmo isolado nunca
  completava o handshake.
* **Certificados.** O que as raízes do contexto não validam não derruba o
  handshake: ele para com o código 16 (`SSL_ERROR_WANT_CERTIFICATE_VERIFY`),
  o Dart chama o `onBadCertificate` / `badCertificateCallback` num evento
  seguinte e decide (`_decidir`); até lá nada do que o rustls produziu sai
  para o soquete. Recusado, o erro é o da VM:
  `HandshakeException: Handshake error in client (OS Error: \n\tCERTIFICATE_VERIFY_FAILED: unable to get local issuer certificate(handshake.cc:392))`
  (os motivos seguem os textos do BoringSSL). As raízes embutidas
  (`defaultContext`, `withTrustedRoots`) são as do sistema
  (`rustls-native-certs`: arquivos do Linux, chaveiro do macOS, repositório
  do Windows) e, sem nenhuma, as da Mozilla (`webpki-roots`).
* **Por que rustls e não BoringSSL.** O `boring` exige, no build, cmake, Go
  e bindgen (libclang), e o BoringSSL não tem ABI estável; o rustls com
  `ring` compila com o Cargo nos três sistemas e vai pré-compilado no
  runtime da distribuição. Diferenças documentadas em relação à VM:
  sem renegociação: `allowLegacyUnsafeRenegotiation` só é guardado, e um
  pedido de renegociação do servidor (TLS 1.2) é recusado com ou sem a
  opção — sem ela a conexão termina com `TlsException`, como na VM (o texto
  do erro é o do rustls, não o `NO_RENEGOTIATION` do BoringSSL); com ela a VM
  renegociaria (`tls_renegociacao.dart` em `io_regressao.rs`, contra o
  `openssl s_server -www`, que pede a renegociação ao receber `GET /reneg`:
  a entrada padrão do `s_server` não serve, porque no Windows ele bloqueia
  nela antes de atender o soquete).
  Os formatos de chave e certificado seguem a VM
  (`crates/runtime/src/tls_formatos.rs`): PEM, PEM cifrado legado, PKCS#8
  cifrado e PKCS#12 (PBES2 e as PBE legadas), com a mesma ordem de decisão
  (PKCS#12 só sem linha de início PEM; DER solto recusado) e os mesmos erros
  (`tls_formatos.dart` em `io_regressao.rs`). O resto — cadeia e chave,
  autoridades de cliente, ALPN, versão mínima 1.2/1.3, `keyLog`, `peerCertificate`, `selectedProtocol` — segue a
  VM.
* **Verificado** (`crates/cli/tests/io_regressao.rs`, `tls_e_https_como_a_vm`,
  saída idêntica à do `dart run`, também no JIT e com
  `DARTFORGE_GC_STRESS=1`): cliente e servidor no mesmo isolado, ALPN, os
  campos do `X509Certificate`, a recusa sem a CA, o `onBadCertificate`, 200 KB
  (mais que os anéis) e `HttpServer.bindSecure` + `HttpClient`.

Achado no caminho: a estimativa de bytes do heap estourava (subtraía, na
coleta, a capacidade de agora de valores cujo armazenamento natives como
`_GrowableList._setLength` trocaram no lugar); agora a coleta refaz a
estimativa a partir dos vivos. E o `main(List<String> args)` passou a
receber a linha de comando (AOT; no JIT, os argumentos depois do programa
em `dartforge run`, entregues também ao runtime da biblioteca do SDK).

### 7.12 Gatilho da coleta com histerese; `alloca` só na entrada

* **Gatilho × teto** (`heap.rs`, `recalcular_gatilhos`). Antes, passar da
  metade do teto (`DARTFORGE_HEAP_MAX_MB`) coletava em **toda** alocação:
  um programa com muito dado vivo parava de andar (medido: 9 000 listas de
  400 inteiros vivas e 300 000 temporárias, teto de 64 MB — mais de 120 s).
  Agora a próxima coleta vem quando se gasta metade da folga até o teto
  (no mínimo 256 KB; sem teto, o heap pode dobrar), e só a alocação que
  passaria do teto força uma última coleta antes da falta de memória. O
  mesmo programa: 2,8 s. Teste: `io_regressao::heap_perto_do_teto…`.
* **`alloca` só no bloco de entrada** (`llvm/mod.rs`,
  `emit_buffers_de_closure`). Os vetores dos literais de lista, mapa e
  record e os locais nasciam onde a instrução estava: num laço, cada volta
  reservava pilha nova, e 300 000 voltas de `<int>[i, i + 1, i + 2]`
  estouravam a pilha (a queda aparecia no RTI, a primeira chamada funda).

### 7.13 A distribuição (`dartforge empacotar`) e o SDK da fonte por padrão

* **Leiaute** (`crates/elements/src/distribuicao.rs`): `bin/dartforge`,
  `lib/dartforge-distribuicao.json` (a marca), `lib/runtime/`,
  `lib/sdk_nativo/`, `lib/dart-sdk/{lib,version}`, `lib/llvm/bin/` (Clang e
  lld). O SDK do Dart, a sobreposição, o Clang e o runtime são procurados
  primeiro ali (depois nas variáveis de ambiente e na árvore de
  desenvolvimento); o cache vai para o do usuário
  (`~/.cache/dartforge/nativo`, `~/Library/Caches/…`, `%LOCALAPPDATA%\…`).
  Os caminhos fixos da máquina de desenvolvimento saíram do código (o
  `scripts/env.ps1` os aponta por variável).
* **`dartforge empacotar <destino>`** monta a distribuição a partir da árvore
  que compilou o binário. Verificado no Linux x86-64 num ambiente limpo
  (`env -i PATH=/usr/bin:/bin`: sem Rust, sem LLVM, sem Dart): AOT de
  desenvolvimento e de produção (com TLS), `Isolate.run`, argumentos do
  `main` e o JIT.
* **Linux: ligação pelo `ld.lld` direto** (`ligador.rs`), sem o driver do
  Clang e sem GCC/`libc6-dev` na máquina: a distribuição leva um **sysroot
  de ligação** (`lib/sysroot/<triple>/`: `Scrt1.o`, `crti.o`,
  `crtbeginS.o`, `crtendS.o`, `crtn.o`, as `.so` da glibc e da libgcc,
  `libc_nonshared.a`, `libgcc.a`, o carregador), copiado pelo `empacotar`
  da máquina que monta a distribuição; numa árvore de desenvolvimento os
  mesmos arquivos vêm do sistema (`clang -print-file-name`). As `.so` são
  só entrada da ligação: o programa carrega as do sistema ao rodar. O
  executável, a biblioteca do SDK da fonte e a produção (LTO no lld, com
  `--lto-partitions`) passam por aí; o corpus continua 225/225. Tamanho da
  distribuição no Linux: 293 MB (129 MB do lld sem símbolos, 70 MB do SDK do
  Dart, 51 MB do dartforge). O CI monta a distribuição e a exercita num
  ambiente limpo (`env -i`). No macOS e no Windows a ligação ainda passa
  pelo driver do Clang (que a distribuição leva); o Windows precisa ainda das
  bibliotecas do MSVC/Windows SDK na máquina.
* **O SDK da fonte é o padrão** (a troca de P5d): o corpus nativo passa
  225/225 por ele e 99/225 pelo runtime por nome de antes, que fica atrás de
  `DARTFORGE_SDK_DA_FONTE=0`.

### 7.14 Depuração nativa (J05)

`dartforge compile-native <entrada.dart> -o <exe> --depuracao` (também
`aot … --depuracao`) gera as tabelas de linha para o depurador nativo:
DWARF 5 no Linux, DWARF 4 no macOS, CodeView no Windows.

* **Posições.** Com `CompileOptions::depuracao`, o contexto indexa as linhas
  das unidades; cada comando baixado marca a posição dele
  (`FnBuilder::marcar_posicao`, a de fora volta ao sair), e toda instrução
  emitida a registra por `ValueId` (`hir::DepuracaoDaFuncao`), assim como o
  terminador de cada bloco (o `return` e o `break` não emitem instrução). Os
  passos da HIR não mudam: a instrução que um passo cria herda a posição da
  anterior.
* **Emissão** (`llvm/depuracao.rs`). A função com posições ganha um
  `DISubprogram` com o nome Dart qualificado (`dobro`, `Conta.depositar`,
  `main`) e cada linha de instrução do texto dela um `!dbg` com a
  `DILocation` do comando — anotado no texto já emitido, a partir de
  marcadores `; df.pos`, o que cobre também as instruções auxiliares
  (coerções, quadro de raízes, conferência de exceção); o LLVM exige o
  `!dbg` em toda chamada de função com subprograma. Unidade `FullDebug` (só
  nela o LLVM emite o `DW_TAG_subprogram`); variáveis e tipos Dart não são
  descritos.
* **HIR.** Função com posições não é embutida pelo inliner da HIR (o corpo
  copiado perderia a posição e o breakpoint nela não pararia).
* **Ligação.** No Linux a ligação de desenvolvimento mantém as seções; a de
  produção (`--optimize`) não tira os símbolos com `--depuracao`. No
  Windows, `-g` na ligação (o `/DEBUG` do ligador: o PDB ao lado do
  executável). No macOS o mapa de depuração do executável aponta para o
  objeto do programa, que fica no cache de objetos.
* **Medido.** `crates/cli/tests/depuracao.rs` (CI do Linux, com o `gdb`):
  `break main.dart:3` para no `return` de `dobro` com a pilha
  `dobro main.dart:3 ← main main.dart:17`, e `break main.dart:9` em
  `Conta.depositar main.dart:9 ← main main.dart:20`; a saída do programa não
  muda. `crates/emit_native/tests/depuracao.rs` confere o IR (todo `!dbg`, o
  `ret` com a linha do `return`, nada sem a opção). CodeView conferido no
  objeto COFF do mesmo IR (`llvm-readobj --codeview`: `DisplayName`
  `dobro`/`Conta.depositar`/`main`, linhas de `main.dart`); o PDB e o macOS
  não têm teste no CI.
* **Fora deste passo** (decisão do proprietário sobre o escopo do perfil de
  desenvolvimento): o protocolo de serviço da VM (DevTools, breakpoints pelo
  IDE Dart), inspeção de variáveis Dart, `dart:developer`
  (`log`/`postEvent`/extensões de serviço continuam no estilo PRODUCT) e o
  registro do código do JIT no depurador (a interface GDB JIT do LLVM).

## 8. Eliminar trabalho antes do LLVM (otimizador da HIR)

O LLVM só otimiza o que recebe. Um objeto criado pelo runtime
(`dartforge_object_new`) e lido por chamadas opacas não some no LLVM; um
`i.toDouble()` que passa pelo despacho por seletor também não. O objetivo
aqui não é deixar o runtime fazer mais depressa o trabalho de sempre, é o
executável precisar de menos trabalho.

### 8.1 Os passes (`crates/emit_native/src/otimizar/`)

Rodam sobre o módulo HIR do programa, antes do emissor, em todo modo (AOT,
produção e JIT); `DARTFORGE_OTIMIZAR_HIR=0` desliga para comparar.

1. **`mem2reg`**: locais (`Alloca` lido e gravado só por `Load`/`Store`)
   viram valores SSA, com `phi` nas fronteiras de dominância (Cytron et al.;
   dominadores de Cooper, Harvey e Kennedy). Um local `Ref` promovido deixa
   de ter slot fixo no quadro de raízes: a vivacidade (`llvm/raizes.rs`)
   decide.
2. **Resumo de exceções** (`efeitos.rs`): quem não pode deixar exceção
   pendente, pelo maior ponto fixo (começa com todas as funções do módulo
   sem lançar). Em toda chamada o estado de exceção está limpo — cada
   operação que pode lançar é conferida em seguida —, então a conferência
   logo depois de uma chamada a uma função que não lança é sempre falsa e
   some com o desvio (`simplificar.rs`). Só vale numa função em que toda
   operação que lança é conferida logo depois (as externas casadas pelo
   nome, conferidas no fim do comando, desligam isso na função). As
   alocações simples do runtime estão marcadas sem lançar
   (`ALOCA_SEM_LANCAR`, conferidas no código de cada uma).
3. **Inlining** (`inline.rs`) das funções pequenas do módulo (até 40
   instruções): construtores, getters, operadores, **também as que lançam**
   (N12). Quem lança deixa a exceção pendente e retorna; a saída excepcional
   do corpo copiado é um `return` como os outros, que vira desvio para a
   continuação, e a conferência que seguia a chamada — que fica lá — leva a
   exceção ao `catch`/`finally` de quem chama, como antes; nada a religar.
   A cópia de quem pode lançar mantém o `dartforge_exception_clear` dos
   retornos (o do `finally`); o terminador `Throw` não é copiado. Medido
   contra o inlining só de quem não lança (AOT `--optimize`, duas rodadas
   alternadas): `mapa` 1,09–1,17×, `arvores` 1,03–1,11×, `conjunto_str`,
   `construir` e `lista_sort` ~1,05×; o resto no ruído.
4. **Substituição escalar** (`escape.rs`): um objeto criado na função cujo
   único uso é ler e gravar os próprios campos por índice constante não
   escapa — ninguém observa a identidade dele (nem `==`, nem `is`, nem o
   coletor). Cada campo vira um local, que o `mem2reg` promove. Qualquer
   outro uso (argumento, retorno, `phi`, gravação em outro objeto, tipo,
   RTI) conta como fuga.
5. Limpezas: dobra de comparações e desvios constantes, blocos
   inalcançáveis e encadeados, valores puros sem uso.

O critério do exemplo de referência foi cumprido:

```dart
double soma(double a, double b) {
  final p = Ponto(a, b);
  return p.x + p.y;
}
```

sai como um `fadd` dos dois argumentos: nenhuma alocação, nenhum acesso ao
heap, nenhuma raiz, nenhuma conferência de exceção.

### 8.2 O que mais saiu do caminho quente

* **`int`/`double` em linha** (`lower/intrinsecos.rs`): com o tipo estático
  exato, `isEven`, `isOdd`, `isNegative`, `toDouble`, `abs`, `isNaN`,
  `isInfinite`, `isFinite` e `toInt`/`truncate` (o membro do SDK no caminho
  frio, para NaN e infinito) não passam mais pelo seletor.
* **Contexto da thread** (`runtime/src/heap.rs`, `Contexto`): a exceção
  pendente e o topo da pilha-sombra num `#[repr(C)]` que o código gerado lê
  e grava direto; `dartforge_contexto()` uma vez por ativação no lugar de
  uma chamada por conferência e duas por quadro. O `exception_clear` de
  todo `return` não toca mais no heap quando não há exceção.
* **`for-in` sobre `List<E>`** (`lower/sdk_fonte.rs`): a lista do runtime é
  percorrida pelo índice, com a conferência de comprimento do
  `ListIterator` (`ConcurrentModificationError`) a cada volta; outra classe
  que implementa `List` segue pelo `Iterator` dela.
* **`List<bool>`** lida e gravada em linha, e a gravação direta confere o
  `E` reificado da lista (`dartforge_lista_len_gravavel`): um `List<Never>`
  ou um `List<int>` visto como `List<num>` fica com o `[]=` do SDK, que
  lança o `TypeError` da VM (`corpus/nativo/27`). Antes, a gravação direta
  de `int`/`double` não conferia a covariância.

* **Cabeçalho fixo de lista** (`runtime/src/heap.rs`, `Elementos` e
  `CabecalhoDeLista`): os elementos de `Value::List` ficam atrás de um
  `#[repr(C)]` num `Box` (endereço dos dados, comprimento lógico, bits de
  gravação conferida) que não muda de endereço enquanto a lista vive.
  `dartforge_lista_cabecalho` é pura do handle (`memory(none)`) e sai dos
  laços; o comprimento e os dados são lidos em linha a cada uso. Toda
  mudança de estrutura passa pela guarda de `Elementos::vetor_mut`, que
  ressincroniza o cabeçalho. A gravação direta é conferida uma vez por
  lista (modificável, `E` aceita o escalar) e marcada no cabeçalho;
  `set_metadado` e `marcar_imutavel` apagam a marca.
* **`List.filled` e `add`**: `_List.filled`/`_GrowableList.filled`
  (sobreposição de `array.dart`/`growable_array.dart`) preenchem no
  runtime; `lista.add(v)` de `List<int|double|bool>` com `v` do tipo exato
  acrescenta o escalar sem caixa (`dartforge_lista_add_escalar`, conferido
  uma vez por lista).
* **`sort()` de `List<int>`** (sobreposição de `internal/sort.dart` e
  `collection/list.dart`): o mesmo dual-pivot do SDK com `int` estático.
* **Despacho de poucos alvos** (`lower/sdk_fonte.rs`,
  `despacho_por_classe`): um membro fechado do SDK com 2 a 4
  implementações vira um `switch` pela classe com a chamada direta ou o
  acesso ao campo de cada uma; o seletor fica no `default`.
* **Closures**: o cabeçalho imutável (`CabecalhoDeClosure`, num `Box`) vem
  de uma chamada pura e a chamada tipada lê a ABI, o corpo e o ambiente em
  linha; as capturas são lidas em linha do vetor do ambiente
  (`dartforge_env_dados`, que não muda de tamanho).
* **Strings**: o `==` de `_StringBase` compara no runtime (sobreposição de
  `string_patch.dart`); o literal é achado pelo endereço da constante,
  conferido pelos bytes.
* **GC**: o gatilho por contagem acompanha também as posições percorridas
  pela última marcação (uma lista grande com poucos objetos vivos não
  coleta a cada 256 alocações).

### 8.3 Medido (Linux x86-64, AOT `--optimize`, `bench/desempenho`)

Tempo estável por núcleo, ms (mediana das rodadas depois da primeira):

| núcleo | antes | depois |
| --- | ---: | ---: |
| `objetos_temporarios/soma_ponto` | 555 | 4 |
| `objetos_temporarios/pontos` | 350 | 57 |
| `numerico/collatz` | 4600 | 55 |
| `chamadas/formas` | 700 | 15 (Dart AOT 6,3) |
| `chamadas/fib` | 11,6 | 2,3 |
| `colecoes/crivo` | 1041 | 26 (Dart AOT 31) |
| `colecoes/lista_leitura` | 122 | 12 (Dart AOT 10,5) |
| `colecoes/lista_add` | 135 | 29 (Dart AOT 17) |
| `colecoes/lista_sort` (1 milhão, com `_Mint`) | 84 000 | 240 (Dart AOT 410) |
| `colecoes/mapa` | 1437 | 560 (Dart AOT 92) |
| `colecoes/conjunto_str` | 540 | 185 (Dart AOT 23) |
| `chamadas/closures` | 291 | 40 (Dart AOT 21) |

A comparação com a VM e o `dart compile exe` é
`scripts/comparar-desempenho.py`. O que ainda pesa, pelo perfil: o código do
SDK compilado à parte (`List.filled`, `add`, `sort`, `Map`) passa pelo
despacho por seletor com a convenção uniforme e pelas conferências de RTI
(`dartforge_rti_como`), sem o inlining nem a especialização que o programa
já recebe. O mapa e o conjunto ainda pagam o despacho das funções de
igualdade e hash (`_equals`/`_hashCode` dos mixins) e a conferência de RTI
na entrada uniforme do `add`/`[]=`; os objetos, duas alocações cada (o
`Value` e o vetor de campos).

### 8.4 Os quatro executores (medido em 2026-09-27)

`scripts/comparar-desempenho.py --repeticoes 3`: o JIT e o AOT do DartForge
contra a VM (`dart run`) e o `dart compile exe`. O JIT do DartForge gera
código sem otimização (`CodeGenLevelNone`, o par do `clang -O0`: partida
rápida para o ciclo de edição e recarga); a VM otimiza as funções quentes em
segundo plano. A diferença nos laços apertados (`produto_f64`,
`lista_leitura`) é essa, e o que a fecha é um JIT em camadas (recompilar o
que esquenta com o nível do AOT), não o código gerado.

Máquina: Linux-6.18.44-fc-v37-x86_64-with-glibc2.39, 4 CPUs; Dart SDK version: 3.6.2 (stable) (Wed Jan 29 01:20:39 2025 -0800) on "linux_x64"; 3 repetições alternadas.

Tempo estável por núcleo (ms): mediana das rodadas depois da primeira, e entre colchetes a faixa (mínimo–máximo) de todas as repetições; menor é melhor. `esgotou` = passou do prazo. Razão = DartForge AOT / Dart AOT.

| núcleo | DartForge JIT | DartForge AOT | Dart VM (JIT) | Dart AOT | razão |
|---|---:|---:|---:|---:|---:|
| chamadas/formas | 58.2 [56.5–60.6] | 11.2 [11.0–11.8] | 2.5 [1.9–18.0] | 5.7 [5.7–5.9] | 1.95x |
| chamadas/closures | 206.8 [201.1–227.1] | 40.4 [40.1–46.2] | 14.9 [14.3–17.7] | 20.5 [19.3–21.3] | 1.97x |
| chamadas/fib | 6.6 [6.5–6.8] | 2.7 [2.7–5.0] | 7.3 [7.1–7.7] | 6.5 [6.4–7.1] | 0.42x |
| colecoes/crivo | 114.5 [102.2–136.7] | 27.4 [26.8–48.2] | 33.2 [29.8–49.5] | 32.2 [31.0–35.8] | 0.85x |
| colecoes/lista_add | 37.2 [34.9–39.7] | 30.2 [29.2–37.4] | 18.1 [17.1–37.9] | 18.7 [17.4–33.6] | 1.61x |
| colecoes/lista_leitura | 174.6 [170.4–183.4] | 12.3 [11.8–13.2] | 10.2 [9.0–16.6] | 10.8 [10.4–13.2] | 1.14x |
| colecoes/lista_sort | 672.1 [649.4–735.3] | 237.4 [233.1–259.4] | 210.6 [206.1–246.2] | 423.8 [401.6–461.3] | 0.56x |
| colecoes/mapa | 1251.4 [1193.9–1334.5] | 598.8 [569.0–710.4] | 84.5 [62.6–100.6] | 86.8 [78.7–104.1] | 6.90x |
| colecoes/conjunto_str | 316.3 [292.1–360.5] | 188.2 [178.3–265.6] | 23.8 [21.1–27.5] | 23.0 [22.5–26.3] | 8.18x |
| numerico/mandelbrot | 34.2 [33.3–36.5] | 12.0 [11.8–14.5] | 14.4 [14.0–19.4] | 13.9 [13.8–16.0] | 0.86x |
| numerico/collatz | 133.9 [129.9–183.7] | 52.2 [51.9–53.8] | 96.8 [96.0–107.8] | 84.5 [83.8–116.2] | 0.62x |
| objetos_escapam/arvores | 369.6 [364.0–418.9] | 244.8 [223.5–266.5] | 55.9 [54.8–63.7] | 49.6 [46.8–59.6] | 4.93x |
| objetos_escapam/lista_ligada | 127.8 [104.6–189.1] | 90.6 [72.3–148.1] | 14.6 [12.8–17.4] | 14.9 [13.9–19.0] | 6.09x |
| objetos_temporarios/pontos | 86.2 [84.5–95.4] | 57.3 [55.0–62.8] | 10.0 [8.5–19.0] | 13.3 [12.8–14.2] | 4.30x |
| objetos_temporarios/soma_ponto | 26.1 [26.0–26.4] | 4.1 [4.0–4.9] | 4.8 [4.5–18.0] | 26.4 [25.9–27.7] | 0.15x |
| textos/construir | 971.6 [914.4–1097.2] | 708.4 [660.6–807.1] | 91.2 [76.1–110.1] | 103.9 [79.7–126.1] | 6.82x |
| textos/hashes | 283.4 [278.3–336.4] | 249.5 [237.1–292.3] | 36.8 [33.5–57.3] | 38.9 [36.6–44.4] | 6.42x |
| tipados/produto_f64 | 178.9 [177.2–189.0] | 3.7 [3.6–4.7] | 3.7 [3.6–19.8] | 3.8 [3.8–4.1] | 0.97x |
| tipados/fnv_bytes | 196.4 [191.6–224.3] | 14.9 [14.8–15.0] | 19.4 [19.3–20.8] | 22.2 [22.2–22.5] | 0.67x |

Tempo total do processo (s, mediana; inclui início e, nos JITs, compilação):

| programa | DartForge JIT | DartForge AOT | Dart VM (JIT) | Dart AOT |
|---|---:|---:|---:|---:|
| chamadas | 1.81 | 0.34 | 0.57 | 0.20 |
| colecoes | 19.74 | 8.91 | 3.06 | 3.99 |
| numerico | 1.21 | 0.40 | 1.03 | 0.60 |
| objetos_escapam | 3.36 | 2.14 | 0.84 | 0.43 |
| objetos_temporarios | 0.87 | 0.37 | 0.48 | 0.25 |
| textos | 7.95 | 5.78 | 1.20 | 0.88 |
| tipados | 2.46 | 0.12 | 0.53 | 0.17 |


Os núcleos de `objetos_*` depois do espaço de objetos, da alocação em linha
e do coletor geracional (N19) estão em §8.6.

### 8.5 Mapas, conjuntos e strings (N18, medido em 2026-09-29)

Ponto de partida (§8.4): `colecoes/mapa` 6,90×, `colecoes/conjunto_str`
8,18×, `textos/construir` 6,82×, `textos/hashes` 6,42× o Dart AOT. O perfil
(callgrind, `valgrind` da máquina; o `perf` não está disponível) mostrou que
o `compact_hash.dart` já era o da VM (índice `Uint32List` e dados lado a
lado, sem nó por entrada): o tempo ia no **caminho até a tabela**. Um
`m[k] = v` de `Map<int, int>` custava cerca de 10 mil instruções: cada
`_data[i] = x` passava pelo seletor e pela entrada uniforme do `[]=` de
`_List`, com a conferência de covariância (`dartforge_rti_como_em`, que na
época ainda formatava a mensagem mesmo quando passava — corrigido à parte
em `tipos.rs`), e `_hashCode`/`_equals` iam ao despacho dinâmico. Na VM, o
compilador especializa `_hashCode`/`_equals` do
`_OperatorEqualsAndHashCode` para `int` e `String` e embute a sonda.

O que entrou, uma coisa de cada vez:

1. **Sonda do `_Map`/`_Set` no runtime** (`crates/runtime/src/nativos_hash.rs`,
   sobreposição de `compact_hash.dart`). `_Map.[]=`, `[]`, `containsKey` e
   `_Set.add`, `contains`, `lookup` chamam primeiro um native que percorre a
   MESMA tabela, com as contas de `_HashBase` (`_hashPattern`,
   `_firstProbe`, `_nextProbe`), e grava o que o Dart gravaria — então a
   ordem de inserção, as remoções, a iteração e o `_rehash` continuam os do
   SDK. Só para chave `int` (`Smi`/`_Mint`: `hashCode` é o valor) e `String`
   (`Texto::hash_vm`, igualdade por unidades), cujo `==` é conhecido sem
   chamar Dart; `int` diante de `double` (`1 == 1.0`), outra chave, tabela
   cheia ou forma inesperada devolvem "não sei" sem mudar nada, e o caminho
   do SDK faz a operação. `mapa` 2,6 → 0,93 bilhão de instruções (200 mil
   chaves, callgrind); 730 → 290 ms.
2. **Interpolação com `int` sem caixa** (`JuntarTextos`, `hir.rs`,
   `llvm/mod.rs`; `dartforge_string_juntar_tipado`, `strings.rs`): a parte
   `int` vai como par (espécie, bits) e os dígitos são escritos direto no
   texto junto (dois por divisão), sem a string intermediária de
   `dartforge_to_string_i64` — uma alocação a menos por `'k$i'`. Cada texto
   é lido do heap uma vez.
3. **Memória direta da RTI** (`tipos.rs`, `memo_e`/`memo_aval`): `v is t` e
   `dartforge_rti_avaliar` guardados por uma chave do valor (o tipo
   reificado gravado, ou a espécie/classe) numa tabela de 1024 vagas,
   esquecida quando uma regra, classe ou forma do runtime muda. As
   conferências de covariância da entrada `$c` (`K`/`V` do `[]=`, `E` do
   `add`) deixam de montar o tipo do valor e de consultar o cache de
   subtipos: `dartforge_rti_e` 235 → 112 instruções por chamada.
4. **Buscas de `String` e `split` no runtime** (`nativos_strings.rs`,
   sobreposição de `string_patch.dart`): `_substringMatches` (`startsWith`,
   `endsWith`), `indexOf`/`lastIndexOf`/`contains` por `String` e
   `_splitWithCharCode` eram laços de `codeUnitAt` — intrínseco em linha na
   VM, uma chamada ao runtime por unidade aqui.
5. **`StringBuffer` com acumulador do runtime** (sobreposição de
   `string_buffer_patch.dart`): as unidades escritas vão direto para um
   `Value::StringBuffer`, uma chamada por `write`, sem um objeto `String`
   por parte guardado numa lista (nem a conferência do `add` dela); o
   `toString` copia o acumulado. `sb.write('item $i;')` × 300 mil: 340 →
   75 ms (Dart AOT 90 ms), e o pico de memória do `textos` cai de ~190 MB
   (Dart AOT) para 68 MB.

Correção: `corpus/nativo/47_mapas_e_conjuntos_int_string.dart` e
`49_strings_busca_divisao_e_buffer.dart` iguais à VM no AOT, no JIT e com
`--gc-stress`; corpus nativo 52/52 (AOT e JIT; com `--gc-stress` 51/52, o
`50_receptor_int_e_objetos_do_espaco` de outro trabalho esgota o prazo de
5 s sob carga), `corpus/js` no nativo 235/235; testes de
`dartforge-runtime` e `dartforge-emit-native` verdes.

`scripts/comparar-desempenho.py --repeticoes 5 colecoes textos`, depois
(máquina com outros agentes compilando: o Dart AOT mediu ~2× o de §8.4, por
isso a razão é o número que compara):

| núcleo | DartForge AOT antes (§8.4) | DartForge AOT depois | Dart AOT (mesma rodada) | razão antes | razão depois |
|---|---:|---:|---:|---:|---:|
| colecoes/mapa | 598,8 | 308,7 [270,6–370,4] | 168,1 [113,0–215,6] | 6,90× | 1,84× |
| colecoes/conjunto_str | 188,2 | 121,1 [117,5–160,4] | 29,7 [28,4–36,3] | 8,18× | 4,08× |
| textos/construir | 708,4 | 252,2 [240,4–355,6] | 162,7 [111,3–221,2] | 6,82× | 1,55× |
| textos/hashes | 249,5 | 189,3 [168,7–219,3] | 55,0 [52,6–71,8] | 6,42× | 3,44× |

Tempo total do processo (s, mediana de 5): `colecoes` 5,17 contra 5,87 do
Dart AOT (antes 8,91 contra 3,99); `textos` 2,72 contra 1,39 (antes 5,78
contra 0,88). Pico de memória (MB): `colecoes` 147,9 contra 150,2 do Dart
AOT; `textos` 67,7 contra 189,2. (A medição de memória de antes não foi
registrada em §8.4.)

O que ainda pesa, pelo perfil, e é de outras frentes: a entrada uniforme
`$c` (vetor de argumentos, `dartforge_rti_avaliar` duas vezes por `[]=` —
cerca de 180 instruções cada, a maior parte no metadado do receptor), o
despacho por seletor e `dartforge_value_class`, a alocação de cada string
(`Heap::allocate` e a coleta), e o iterador de `where`/`ListIterator` (os
núcleos `where`+`endsWith` ainda 20× a VM). No que é desta frente: o
`hashCode` de `String` recalculado a cada chamada (a VM o guarda no objeto;
aqui exige um campo no `Texto`, `heap.rs`) e o literal de string achado
por um mapa por endereço a cada avaliação (`dartforge_string_new`, ~90
instruções).

### 8.6 Alocação e coleta dos objetos do usuário (N19, medido em 2026-09-29)

Ponto de partida (§8.4): `objetos_escapam/arvores` 4,93×,
`objetos_escapam/lista_ligada` 6,09×, `objetos_temporarios/pontos` 4,30× o
Dart AOT. O perfil (callgrind, `arvores(12)` + `lista(200000)`, 723 milhões
de instruções) mostrou onde iam as cerca de 850 instruções por objeto:

* alocação por chamada ao runtime: `dartforge_object_new`, `Heap::allocate`,
  o registro da tabela de métodos e um `calloc`/`free` para o vetor de campos
  (cerca de 250 instruções);
* cada leitura ou gravação de campo chamava `dartforge_object_campos` (TLS,
  tabela de handles, `match` no `Value`);
* coleta completa a cada ~1 MiB alocado, sempre marcando tudo o que vive (a
  árvore longa) e varrendo a tabela de slots inteira.

Como a VM faz (`runtime/vm/heap/scavenger.cc`, a alocação em linha de
`stub_code_compiler.cc`): *new space* com ponteiro de alocação em linha no
código (TLAB), scavenger de Cheney que só toca os sobreviventes, promoção
para o *old space*, barreira de escrita com *store buffer* (o velho que
recebe referência nova é lembrado). O DartForge não pode mover objetos: o
código gerado guarda handles em registradores entre pontos de coleta (a
pilha-sombra é só de raízes, não é relida), então o par aqui é **geracional
sem mover** (marcas "pegajosas"):

1. **Espaço de objetos** (`crates/runtime/src/heap.rs`: `Bloco`,
   `EspacoDeObjetos`). O objeto do usuário sai da tabela de slots: vive num
   bloco de página de 64 KiB com blocos de um só número de campos — um
   cabeçalho de 16 bytes (estado de coleta, número de campos, `hashCode` de
   identidade, metadado de RTI), o próprio `Value::Object` (`Value` passou a
   `#[repr(u8)]`, com o layout conferido em `conferir_layout`) e os campos. O
   handle é o endereço do `Value` mais 2 (`h & 3 == 2`); os handles da
   tabela passaram a múltiplos de 4 e o `Smi` continua ímpar. Listas livres
   por número de campos, refeitas a cada varredura; páginas vazias soltas
   além da folga (vivos, demanda do ciclo e um pico recente que decai). A
   validação de um handle (`bloco_de`) usa um mapa de páginas de dois
   níveis, sem hash.
2. **Campos em linha pelo objeto.** O código gerado lê o ponteiro dos
   campos em `h + 14` (uma carga; o que não é objeto lê
   `Contexto::vazios`, zeros, como antes). Sem chamada por acesso.
3. **Alocação em linha (TLAB).** `Contexto::tlab[n]` (n ≤ 16 campos) é uma
   lista de blocos já contados como alocados; o código tira o primeiro,
   grava estado e classe e segue (`llvm/mod.rs`, `emitir_alocacao_em_linha`,
   com o teste de registro da tabela de métodos da classe). Lista vazia vai
   ao runtime, que coleta se preciso e reabastece 64 blocos. No
   `--gc-stress` a TLAB fica vazia: toda alocação passa pelo runtime.
4. **Coletor geracional sem mover.** Um objeto nasce jovem; o que sobrevive
   a uma coleta vira velho. A coleta menor (a cada 4 MiB ou 256 mil
   alocações) marca só a partir das raízes e dos **lembrados**, e varre só a
   lista dos jovens; a completa vem quando o total passa do dobro do que
   sobreviveu à última (e pelo menos dois semiespaços jovens acima dele, com
   histerese de ¾ do gatilho anterior). A **barreira de escrita**: toda
   mutação do runtime passa por `Heap::get_mut`, que lembra o velho (objeto
   ou slot); o código gerado, depois de gravar um `Ref` num campo, testa o
   estado no cabeçalho (`h - 18`) e chama `dartforge_lembrar` se é velho.
   `DARTFORGE_GC_VERIFICAR=1` faz cada coleta menor conferir, por uma
   travessia completa, que nenhum jovem alcançável ficou sem marca (o
   corpus inteiro passa com `--gc-stress` e a verificação ligada);
   `DARTFORGE_GC_RASTRO=1` escreve uma linha por coleta.
5. **O que veio junto** (pedido da coordenação): o `hashCode` de `String`
   calculado uma vez por valor (`Heap::hash_de_texto`, usado por
   `String_getHashCode` e pela sonda de `nativos_hash.rs`); o handle de cada
   literal de texto num cache do ponto de uso, na área do isolado (a busca no
   runtime só na primeira avaliação; a recarga do JIT esvazia os caches); o
   receptor escalar de membro do SDK chamado direto vai em caixa
   (`chamar_membro_fonte`, `corpus/nativo/50`).

`scripts/comparar-desempenho.py` passou a medir também o pico de memória
residente (`ru_maxrss` pelo `wait4`, o mesmo do `/usr/bin/time -v`, que não
está instalado na máquina), aceita `--sem-jit` e `DARTFORGE_BIN`.

Medido (Linux x86-64, 4 CPUs com outros agentes compilando — carga 5 a 6,
por isso as faixas largas; razão = DartForge AOT / Dart AOT das mesmas
rodadas alternadas, mediana de 5):

| núcleo | antes (§8.4) | depois | Dart AOT | razão antes | razão depois |
|---|---:|---:|---:|---:|---:|
| objetos_escapam/arvores | 244,8 | 200,8 [169–279] | 50,4 | 4,93× | 3,98× |
| objetos_escapam/lista_ligada | 90,6 | 146,5 [95–187] | 17,1 | 6,09× | 8,58× |
| objetos_temporarios/pontos | 57,3 | 33,0 [30–74] | 17,3 | 4,30× | 1,91× |
| objetos_temporarios/soma_ponto | 4,1 | 6,3 | 32,1 | 0,15× | 0,20× |
| textos/hashes | 249,5 | 105,5 | 59,0 | 6,42× | 1,79× |
| colecoes/conjunto_str | 188,2 | 94,7 | 44,0 | 8,18× | 2,15× |

(`textos` e `colecoes` também têm o trabalho de mapas e strings do N18, §8.5;
aqui só o hash guardado e o cache de literal são deste passo.)

Pico de memória residente (MB, mediana; `objetos_escapam` inteiro):
DartForge AOT 179, Dart VM 182, Dart AOT 50. Em `objetos_temporarios`,
13,6 contra 13,0 do Dart AOT.

Em instruções (callgrind, o mesmo programa reduzido do perfil inicial): 723
milhões → 319 milhões depois dos passos 1–3 (a alocação saiu do perfil:
`dartforge_object_new` de 60 milhões para 1). Em `arvores(12)`, a coleta
passou de 40% das instruções para 25%, e o que resta é o código do programa
(`arvore` e `conta`: o `esq?.conta() ?? 0` encaixota e desencaixota o `int?`,
e cada ativação chama `dartforge_contexto` e monta o quadro da
pilha-sombra).

**O que não fechou, e por quê.** `lista_ligada` ficou pior: tudo o que ela
aloca sobrevive (um milhão de nós por rodada, refeitos a cada rodada), então
toda coleta menor promove tudo o que marcou, e as completas marcam a lista
inteira duas vezes por rodada. Por objeto, a marcação custa cerca de 60
instruções e a varredura 30. O nó ocupa 88 bytes (cabeçalho 16 + `Value` 40
+ 2 campos de 16) contra 24 na VM, e o custo de memória (faltas de página,
banda) domina o tempo. Na VM a marcação do *old space* é concorrente, em
outras threads. O que falta, pela ordem do ganho esperado:

* **campos de 8 bytes**: o `is_ref` de cada campo é estático por classe
  (`repr_do_campo`); um mapa de bits por classe no lugar do `bool` por campo
  deixa o nó da lista em 72 bytes e a marcação sem o teste por campo. A API
  `Campos: Deref<[(i64, bool)]>` do runtime (≈60 usos) muda junto;
* **`Value` de 32 bytes** (encaixotar `TypedView`, `class_id` de 32 bits em
  `TypedData`): o bloco perde mais 8 bytes;
* **página jovem com ponteiro de alocação** e reciclagem da página inteira
  quando nenhum objeto dela sobrevive (a varredura menor deixa de tocar cada
  morto, como o scavenger);
* **marcação da completa em paralelo**, ou concorrente (a VM marca o *old
  space* em outras threads);
* no código do programa: `dartforge_contexto` em linha (TLS direto) e `?.` +
  `??` sobre `int?` sem caixa.

### 8.7 Objetos de 8 bytes por campo (N19, segunda rodada, 2026-09-29)

O nó da lista custava 88 bytes (cabeçalho 16 + `Value` 40 + 2 campos de
16), e o `Value` embutido só existia para `Heap::get` devolver `&Value`.
Agora:

* **`Value::Object` saiu do enum.** O objeto do usuário é só o bloco:
  `Cabecalho` de 16 bytes (`estado`, `flags`, `n: u16`, `class_id: i32`,
  `mapa: u32`, `metadado: u32`) e os campos, **palavras de 8 bytes**. O
  runtime o lê por `Heap::objeto(h) -> Obj` (classe, `campo(i) -> (bits,
  é referência)`), grava por `Heap::definir_campo` (com a barreira) e cria
  por `Heap::novo_objeto`; `Heap::get` devolve o marcador `Value::Objeto`.
  Os cerca de 50 usos do runtime (erros, portas, RTI, FFI, saída) passaram
  à API nova.
* **Mapa de referências no cabeçalho**, como o *unboxed fields bitmap* da
  VM: um bit por campo (os 32 primeiros no cabeçalho, os demais em palavras
  depois dos campos). Cada gravação acende ou apaga o bit do campo (o
  código gerado faz a leitura-modificação-escrita junto com o `store`), então
  um campo que às vezes é `Ref` e às vezes escalar continua preciso. A
  marcação percorre só os bits acesos (`trailing_zeros`).
* **Corpo de fora.** O objeto que muda de número de campos depois de criado
  (o erro que ganha o rastro, a recarga do JIT que muda o layout da classe,
  J03) não pode mudar de endereço: os campos vão para um corpo de fora
  (bit `FORA` em `flags`, o endereço no lugar do primeiro campo), e o código
  gerado o segue com uma seleção sem desvio. Todo bloco tem pelo menos um
  campo (o encadeamento da lista livre também mora ali).
* **`hashCode` de identidade pelo endereço** (30 bits espalhados; o
  coletor não move).
* A alocação em linha grava o cabeçalho numa palavra (`1 | n << 16 |
  classe << 32`) e zera o primeiro campo (o encadeamento).

O nó da lista tem 32 bytes (a VM, 24). Validação: corpus/js 235/235 e
corpus/nativo 59/59 em AOT, JIT e `--gc-stress` com
`DARTFORGE_GC_VERIFICAR=1`; `bench/desempenho` inteiro com os resultados da
VM. Medido (3 rodadas alternadas, máquina carregada; razão contra o Dart
AOT das mesmas rodadas):

| núcleo | §8.4 | §8.6 | agora | Dart AOT (ms) |
|---|---:|---:|---:|---:|
| objetos_escapam/arvores | 4,93× | 3,98× | 1,88× (103 ms) | 55 |
| objetos_escapam/lista_ligada | 6,09× | 8,58× | 3,21× (52 ms) | 16 |
| objetos_temporarios/pontos | 4,30× | 1,91× | 1,09× (27 ms) | 25 |

Pico de memória residente de `objetos_escapam`: 179 MB → **75 MB** (Dart
AOT 49, VM 181).

## 9. Servidor HTTP (`dart:io`): medição e onde vai o tempo

### 9.1 O benchmark

`bench/http/servidor.dart` é um servidor `dart:io` de verdade: `HttpServer.bind`
no loopback, `GET /` responde `hello` (texto), `GET /json` responde o
`jsonEncode` de um objeto pequeno; keep-alive ligado (o padrão do
`HttpServer`), `autoCompress` desligado. `scripts/bench-http.py` compila o
servidor pelo AOT do DartForge (`--optimize`, ou `--df-exe` com um executável
pronto; `--df-antes` mede um segundo executável junto, para antes/depois) e
pelo `dart compile exe`, sobe também o `dart run`, e gera a carga com o `wrk`
(`apt-get install wrk`) com 1 e 64 conexões. Por medida: req/s, p50/p99 do
`wrk --latency`, a CPU do servidor por requisição (`utime + stime` de
`/proc/<pid>/stat` sobre as requisições do `wrk`) e a RSS (`VmRSS` em repouso,
depois de subir e responder; `VmHWM` ao fim da carga). As repetições alternam
os executores, com aquecimento antes de cada medida, e o relatório dá mediana
e faixa.

A máquina é disputada (outros agentes compilam ao mesmo tempo; carga 6–9 em 4
CPUs): req/s e latência variam 30–50% entre rodadas. Por isso a comparação
de mudanças é feita também por **instruções por requisição** no `callgrind`
(determinístico: duas rodadas do mesmo executável dão 1 333 696 e 1 333 837):
o servidor roda no `valgrind --tool=callgrind`, recebe 300 requisições de
aquecimento por uma conexão keep-alive, os contadores são zerados
(`callgrind_control -z`) e 500 requisições são contadas. Conta só o espaço de
usuário (as chamadas de sistema ficam de fora).

### 9.2 Medido (Linux x86-64, 4 CPUs, Dart 3.6.2, 2026-09-29)

"Antes" é o `main` de 02h44 (depois de o nome do parâmetro do `TypeError` sair
do caminho feliz, `lower/rti.rs`); "depois" inclui §9.4 e o que os outros
trabalhos mudaram no mesmo intervalo (o heap estava em obra: `try_get`,
`bloco_vivo`). 3 repetições alternadas, 5 s por medida:

| medida | DartForge antes | DartForge depois | Dart VM (JIT) | Dart AOT |
|---|---:|---:|---:|---:|
| req/s, `/`, 1 conexão | 1677 [1273–2045] | 1913 [1340–2217] | 4544 [2479–7640] | 6723 [3143–7057] |
| req/s, `/`, 64 conexões | 1974 [1524–2157] | 1826 [1143–2066] | 10024 [9754–10093] | 10752 [10536–11025] |
| req/s, `/json`, 1 conexão | 1640 [1271–1641] | 1438 [1359–1781] | 4893 [4781–5937] | 4648 [3116–5599] |
| req/s, `/json`, 64 conexões | 1702 [1542–1727] | 1977 [1256–2122] | 8142 [8027–9234] | 10180 [9345–10685] |
| p50 `/`, 1 conexão, ms | 0,395 | 0,340 | 0,188 | 0,088 |
| p50 `/`, 64 conexões, ms | 27,5 | 33,9 | 5,6 | 5,1 |
| p99 `/json`, 64 conexões, ms | 118 | 97 | 33 | 26 |
| CPU/req `/`, 1 conexão, µs | 387 [344–520] | 335 [316–405] | 133 | 75 |
| CPU/req `/json`, 1 conexão, µs | 442 [437–462] | 412 [393–412] | 113 | 128 |
| CPU/req `/json`, 64 conexões, µs | 477 | 473 | 106 | 85 |
| instruções/req (callgrind, `/`) | 1 531 683 | 1 333 696 | — | 103 983 |
| RSS em repouso, MB | 13,3 | 14,7 | 151 | 7,3 |
| RSS de pico, MB | 15,2 | 16,2 | 167 | 18,1 |

A primeira medida da sessão (o `dartforge` de 28/09, antes da mudança do
nome do parâmetro) dava 1 668 req/s e p50 de 0,51 ms com 1 conexão, e
1 565 req/s e p50 de 37,6 ms com 64.

Leitura: o DartForge fica em 3–5× a CPU por requisição do Dart AOT e
executa ~13× as instruções de usuário; a memória é o ponto forte (RSS de pico
menor que a do `dart compile exe` e ~10× menor que a da VM). Com 64 conexões
o servidor não escala (um isolado, CPU no teto): a latência é a fila.

### 9.3 Onde vai o tempo

**Não é a E/S.** `strace -c` sob carga, por requisição: ~2 `write`, ~2
`ioctl(FIONREAD)`, ~1,3 `read`, ~1 `epoll_wait` e ~3,5 `futex` (o
`dart compile exe` faz o mesmo número de `write`/`read`/`ioctl` e mais `futex`
e `rt_sigprocmask`). No `perf`, a thread do manipulador de eventos é 3% das
amostras; os natives de soquete (`Socket_Read`, `Socket_WriteList`,
`Socket_Available`) e o laço de eventos somam ~1% das instruções de usuário
(`_NativeSocket.write` 25 mil, `read` 19 mil, `_RawReceivePort._handleMessage`
12 mil instruções por requisição, de 1,33 milhão). O `write` do soquete é
~10% do tempo, quase todo no TCP do kernel.

**É o código do `dart:_http` compilado.** Por categoria (self, callgrind,
depois de §9.4):

| categoria | instruções/req | % |
|---|---:|---:|
| heap: acesso ao slot (`try_get`, `indice_vivo`, `bloco_vivo`), alocação, raízes e coleta | 516 641 | 38,7 |
| `malloc`/`free`/`memset` da libc (vetores de campos, ambientes, caixas) | 249 723 | 18,7 |
| código Dart compilado | 172 810 | 13,0 |
| RTI e conferências de argumento (`rti_e`, `tipo_do_valor`, `args_casam`, `rti_definir`) | 141 641 | 10,6 |
| outros (exceções, laço, libc) | 93 422 | 7,0 |
| despacho (classe do receptor, seletor) | 90 410 | 6,8 |
| natives do SDK (strings, listas tipadas) | 69 758 | 5,2 |

Contados por uprobe (`perf stat -e uprobes:…`), por requisição, antes de §9.4:
444 alocações do heap (102 objetos, 103 closures, 103 ambientes, 51 células),
874 `dartforge_rti_e`, 1 123 `dartforge_seletor`, 115 `dartforge_string_new`
(literais: sem alocação, mas uma busca por endereço cada), uma coleta a cada
2,6 requisições. Pelo `dart:_http` (inclusivo):

* `res.write` → `_HttpOutboundMessage.encoding` relê o `content-type` a cada
  escrita (`ContentType.parse` → `_HeaderValue._parse`): 213 mil instruções
  por requisição (16%). É a semântica do SDK (a VM faz o mesmo parse), mas o
  `_parse` define seis funções locais por chamada — 6 closures, 6 ambientes e
  6 células alocados a cada vez — e compara cada caractere com um literal de
  um caractere (`char == " "`).
* `writeHeaders` 189 mil (14%): `_CopyingBytesBuilder.add(nome.codeUnits)`
  copia byte a byte pelo `CodeUnits.[]` (149 chamadas por requisição, cada uma
  pelo seletor e pela entrada uniforme `$c`, que confere o `int` do índice no
  RTI) e cada acesso ao `Uint8List` chama `dartforge_typed_len` e
  `dartforge_typed_ptr` (~100 instruções cada).

### 9.4 O que entrou

* **Classes fechadas pelos modificadores** (`lower/sdk_fonte.rs`,
  `classe_fechada_por_modificador`): uma classe `final` ou `sealed` cujos
  subtipos no programa são todos da mesma biblioteca e `final`, `sealed` ou
  privados não pode ser estendida nem implementada de fora — é o caso de
  `String`, `int`, `double`, `num` e `bool`. O membro dela passa a valer como
  fechado (`membro_fechado`), e a chamada com receptor de tipo estático
  `String` (`s.codeUnitAt(i)`, `s.substring`…) vai direto ao alvo, ou ao
  `switch` pela classe com o seletor no `default`, em vez do seletor e da
  entrada uniforme com a conferência dos argumentos. `rti_e` por requisição:
  874 → 590; seletor: 1 123 → 671.
* **`==` com o lado esquerdo de classe fechada** (`igualdade_de_classe_fechada`,
  chamado de `lower/expressoes.rs`): a regra do null de sempre e, com os dois
  lados não nulos, o `==` pelo mesmo despacho. O `char == " "` deixa de passar
  pelo seletor `c:==` e pelo `_OneByteString.==$c` (650 instruções por
  comparação, 123 comparações por requisição).
* **A classe do receptor numa consulta só** (`dartforge_value_class`,
  `nucleo.rs`; `cid_do_valor_do_runtime`, `seletores.rs`): strings, listas,
  caixas e listas tipadas tinham duas ou três consultas ao slot (e dois
  empréstimos do heap) por pergunta; agora uma. `cid_do_runtime` 50 mil →
  9 mil instruções por requisição, mais 21 mil do `cid_do_valor_do_runtime`
  novo: −19 mil.

Instruções por requisição: 1,53 milhão → 1,33 milhão (−13%; a parte medida
destas três mudanças: `cid_do_runtime` −19 mil líquido, `tipo_do_valor` −31 mil,
`==$c` −12 mil, `Universo::interface`/`sub` −25 mil, com o heap mudando por
baixo no mesmo intervalo).

### 9.5 O que falta (pela ordem do ganho medido)

1. **Acesso ao slot e alocação do heap** (38,7% + 18,7% de `malloc`): cada
   consulta de valor passa por `try_get` + `indice_vivo`/`bloco_vivo`; cada
   objeto é um `Value` mais um vetor de campos no `malloc`. Dono: heap/GC.
2. ~~**Funções locais que não escapam**~~: feito em §9.7 (chamadas diretas,
   −20% de instruções).
3. ~~**Conferência de argumento na entrada uniforme** para chamadas de tipo
   estático conhecido~~: feito em §9.6 (entrada `$tc`, −5% de instruções).
4. ~~**`Uint8List` em linha**~~: feito em §9.8 (cabeçalho de endereço
   fixo, −3% de instruções).
5. **Literais de string por ponto de uso**: o handle do literal num global
   do isolado, sem a busca por endereço (115 por requisição).

### 9.6 Entrada tipada: só os parâmetros covariantes (item 3 de §9.5, medido em 2026-09-29)

**A regra.** Numa chamada com o receptor de tipo estático conhecido, o
analisador garante cada argumento contra o parâmetro da interface — menos
os *covariantes*: os escritos `covariant` (ou que herdam a palavra de um
membro sobrescrito) e os que mencionam um parâmetro de tipo da classe em
posição covariante (`E`, `Iterable<E>`, `FutureOr<T>`, `V Function()`; o
`isGenericCovariantImpl` que o CFE propaga pela sobrescrita). A VM compila
duas entradas por método — a *checked entry*, dos encaminhadores `dyn:`, e a
*unchecked entry*, das chamadas tipadas —, e o argumento `dynamic` de uma
chamada tipada ganha do CFE um cast implícito em volta dele, logo depois de
avaliado (a mensagem sem " of 'nome'").

**O que entrou** (`crates/emit_native/src/lower/entrada_tipada.rs`):

* a chamada por seletor de um membro do SDK com o receptor tipado e todos os
  argumentos garantidos usa o seletor `t` + o de sempre (`tc:[]`, `ts:x`);
  a tabela da classe o liga à entrada `$tc`/`$ts`, gerada só quando a `$c`
  confere algum parâmetro que a tipada dispensa, e o runtime
  (`dartforge_seletor`) cai no seletor sem o `t` quando a classe não a tem
  (sem o que dispensar, encaminhador de `noSuchMethod`, classe do
  programa). A `$tc` também não confere a aridade;
* a conferência de aridade da entrada uniforme sem nomeados vai em linha
  (`n_req <= posicionais <= n_pos`, nenhum nomeado), sem chamar
  `dartforge_args_casam`;
* o cast implícito do argumento `dynamic` no ponto de chamada, com o tipo do
  parâmetro na invocação (o tipo estático do alvo, ou o do construtor com os
  argumentos de tipo da criação), para funções, construtores, fábricas,
  métodos, índices (`[]`, `[]=`) e operadores; e no `for-in` com o
  elemento `dynamic` numa variável tipada (`for (E e in elementos)` do
  `List.from`). Eram os dois defeitos de N14: `f(<int?>[null] as dynamic)`
  para `Iterable<int>` passava, e o erro do `List.from` saía do `add` com
  " of 'value'";
* os parâmetros covariantes de membro do programa conferidos no despacho
  direto (`chamar_membro`), por implementação e com o receptor, antes de
  converter à representação — antes `A a = B(); a.m(1.5)` com
  `B.m(covariant int x)` truncava para 1; e o tear-off de método confere os
  argumentos como a entrada uniforme (os nomes do parâmetro da
  implementação).

Correção: `corpus/nativo/52_chamada_tipada_e_covariancia.dart`
(covariância de lista, mapa e conjunto pela interface; `covariant` escrito,
nomeado e herdado; genérico da classe pela interface e sobrescrita que
alarga; classe do programa sobre `ListBase`; argumentos `dynamic` para
função, nomeado, tipo de função, construtor, fábrica, construtor genérico,
membros do SDK, índice, operador; a ordem de avaliação do cast; `for-in`,
`List.from`; chamada dinâmica e tear-off) igual à VM no AOT, no JIT e com
`--gc-stress`; corpus nativo 56/56, `corpus/js` pelo nativo 235/235; testes
de `dartforge-emit-native` e `dartforge-runtime` verdes. Fica de fora (já
era assim): a mensagem do `NoSuchMethodError` de aridade errada numa chamada
dinâmica (`dartforge_nsm_chamada` não sabe o receptor nem o nome).

Instruções por requisição (callgrind, `/`, §9.1): **1 225 725 → 1 164 449**
(−5,0%). `dartforge_rti_e` 31,7 → 6,3 mil instruções/req; `args_casam`
14,8 mil → fora do perfil. O `CodeUnits.[]` do `writeHeaders` (149 por
requisição) passa pela `$tc` (sem conferir o índice).

`scripts/comparar-desempenho.py --sem-jit --repeticoes 3 colecoes chamadas
textos` (máquina disputada; a razão contra o Dart AOT é o que compara; os
números incluem o que as outras frentes mudaram desde §8.5):

| núcleo | DartForge AOT | Dart AOT | razão (§8.5) | razão agora |
|---|---:|---:|---:|---:|
| colecoes/mapa | 338,5 | 251,8 | 1,84× | 1,34× |
| colecoes/conjunto_str | 110,6 | 34,6 | 4,08× | 3,20× |
| colecoes/lista_add | 51,2 | 33,0 | — | 1,55× |
| textos/construir | 300,3 | 184,8 | 1,55× | 1,62× |
| textos/hashes | 80,9 | 54,4 | 3,44× | 1,49× |
| chamadas/closures | 85,1 | 36,2 | — | 2,35× |

A avaliação do tipo do receptor duas vezes no `[]=` de mapa (`K` e `V`,
ambos covariantes: a VM também confere os dois) continua: cada
`dartforge_rti_avaliar` passa pela chave do receptor
(`chave_do_valor`, ~100 instruções).

### 9.7 Funções locais que não escapam, chamadas diretas (item 2 de §9.5, medido em 2026-09-29)

**O problema.** Uma função local era sempre um valor: a cada execução da
declaração, um ambiente com as capturas, a closure e — porque o nome conta
como atribuído (a recursão) — uma célula para o nome; toda chamada ia pela
entrada uniforme. O `_HeaderValue._parse` declara oito por chamada (seis mais
as duas de dentro de `parseParameters`), todas lendo e gravando o mesmo
`index`, que por isso também morava numa célula.

**A regra** (o que o grafo de fluxo da VM faz com as closures que não
escapam, e o que o CFE garante sobre o nome de uma função local): uma função
local cujo nome só aparece como alvo de chamada `f(…)` — nunca lido como
valor, passado, guardado ou devolvido — e só de dentro da função que a
declara ou de outras funções diretas não pode ser chamada depois de a função
de fora retornar. A análise é a de `captura.rs` (`analisar_com`): as
referências a cada função local, a forma (sem parâmetros de tipo próprios,
síncrona, só posicionais obrigatórios, sem ler um `late` de fora) e o ponto
fixo — uma função que deixa de ser direta vira closure, e o que ela chama
passa a ser chamado de dentro de algo que escapa. A genérica fica closure (a
tupla `$tipos` vem da entrada uniforme); um corpo `async`/gerador também não
tem diretas (os `alloca` dele viram posições do quadro).

**O código** (`crates/emit_native/src/lower/funcoes_diretas.rs`). A função
direta vira uma função do módulo, `<de fora>$<nome>$d<impressão>`, com os
parâmetros `[this] capturas… [tupla de fora] parâmetros…`, e a chamada, um
`call` estático — sem closure, ambiente nem célula do nome. Cada captura vai
de um de três jeitos, decididos na declaração: **valor** (a variável nunca é
atribuída; lido na hora da chamada), **endereço** (atribuída, escalar, e
nenhuma closure que escapa a captura: o endereço do `alloca` de quem declara,
que a função lê e grava — o `index` do `_parse`) ou **célula** (atribuída e
`Ref`, porque o endereço de um `Ref` fora do quadro de raízes o esconderia do
coletor; ou capturada também por uma closure que escapa). Uma direta que
chama outra declarada fora dela recebe as capturas dela e as repassa: a
chamada nunca procura uma captura pelo nome, que um bloco de dentro pode ter
sombreado.

Correção: `corpus/nativo/54_funcoes_locais_diretas.dart` (o molde do
`_parse` com várias funções sobre o mesmo índice, recursão, captura
modificada antes e depois, `Ref` atribuído, sombreamento, chamada de dentro
de uma closure que escapa, captura compartilhada com uma closure que escapa,
laços com variável por volta, aninhadas, genérica de fora e local genérica,
exceção e `finally`, tear-off, passada adiante, devolvida, `late`, `async`,
`sync*`, `this`) igual à VM no AOT, no JIT e com `--gc-stress`; corpus nativo
58/58 nos três, `corpus/js` pelo nativo 235/235, testes de
`dartforge-emit-native` e `dartforge-runtime` verdes.

Instruções por requisição (callgrind, `/`, §9.1): **1 164 449 → 927 176**
(−20,4%). O `_parse` não cria mais closure nem ambiente; `dartforge_env_new`
inclusivo 112 → 95 mil instruções/req e `dartforge_closure_new_tipada` 130
→ 71 mil (o que resta são closures que escapam de fato: os
`onData`/`onDone` de `_HttpOutgoing.addStream`, os `then` do
`_AsyncCompleter`, as dos `_StreamController`). No `bench/desempenho` nenhum
núcleo tem função local que não escapa: `chamadas/closures` (closures
guardadas numa lista) fica igual (84 ms, 2,7× o Dart AOT, 5 repetições).

### 9.8 Listas tipadas pelo cabeçalho de endereço fixo (item 4 de §9.5, medido em 2026-09-29)

**O problema.** Cada `a[i]`, `a[i] = v` e `a.length` de uma lista de
`dart:typed_data` chamava `dartforge_typed_len` e `dartforge_typed_ptr`, cada
uma com a busca do slot e a resolução da visão (~100 instruções). Num laço
cujo corpo não chama nada o LLVM as tirava do laço (são puras), mas o
`_CopyingBytesBuilder.add` do `writeHeaders` relê `_buffer` (um campo) a
cada volta e as pagava por byte copiado: 170 de cada por requisição.

**O que entrou.** Os bytes de uma lista tipada interna moram atrás de um
cabeçalho de endereço fixo, como o `CabecalhoDeLista` das listas (N13):
`heap::Armazenamento` passou a ser um `Box<CabecalhoTipado>` com o endereço
do primeiro byte (palavra 0) e o tamanho em bytes (palavra 1), além do `Vec`
próprio ou da memória externa de `asTypedList`; nenhum `&mut Vec` sai dele
(só a fatia), então o endereço não muda enquanto a lista vive. O código
gerado (`lower/tipados.rs`, `dados_e_comprimento_tipados`) chama uma vez
`dartforge_typed_cabecalho(h, tipo)` — pura do handle — e lê em linha o
endereço e o tamanho (`>> log2` do elemento); as leituras levam
`!invariant.load` (`llvm/mod.rs`), e o LLVM as tira dos laços e junta as
iguais, como fazia com as duas chamadas puras. A visão não tem cabeçalho
próprio: o runtime devolve o `CABECALHO_TIPADO_VAZIO` (endereço nulo), e o
código volta a `dartforge_typed_len`/`dartforge_typed_ptr`, que resolvem a
base, o deslocamento e a imutabilidade. O acesso guardado pelo teste de
limites usa o endereço lido para ele, sem ler de novo.

Correção: `corpus/nativo/55_listas_tipadas_cabecalho.dart` (os onze tipos
numéricos com os extremos, `Uint8ClampedList`, a vazia, índices fora da
faixa na leitura e na escrita, visões com deslocamento e de outro tipo,
`sublistView`, as não modificáveis, `List<int>` estático sobre a lista
tipada, a cópia byte a byte de `codeUnits` e o construtor que cresce com
`setRange`, SIMD e visão SIMD, 2 000 listas com o coletor, `Isolate.run`,
`ByteData`) igual à VM no AOT, no JIT e com `--gc-stress`; corpus nativo
59/59 (com `--gc-stress`, o `40_listas_compactas` passa do prazo de 5 s sob a
carga da máquina: 6,5 s com `--limite-exec 60`, igual à VM), `corpus/js` pelo
nativo 235/235, testes de `dartforge-emit-native` e `dartforge-runtime`
verdes.

Instruções por requisição (callgrind, `/`, §9.1): **927 176 → 898 112**
(−3,1%; o heap de N19 mudava na mesma janela — a parte desta mudança, pelas
funções: `dartforge_typed_len` + `dartforge_typed_ptr` 50,8 mil
instruções/req → `dartforge_typed_cabecalho` 12,6 mil + as visões 4,4 mil).
`scripts/comparar-desempenho.py --sem-jit --repeticoes 5 tipados`:
`produto_f64` 7,4 ms (Dart AOT 8,1; 0,91×, era 0,97× em §8.4) e `fnv_bytes`
13,6 ms (Dart AOT 20,1; 0,68×, era 0,67×) — os laços continuam vetorizados
(sem o `!invariant.load` o `produto_f64` ia a 16 ms: as leituras do cabeçalho
não saíam do laço). A cópia de `codeUnits` do `writeHeaders` segue byte a
byte pelo `CodeUnits.[]` (agora a `$tc`, §9.6): é a semântica do SDK
(`_CopyingBytesBuilder.add` com uma `List<int>` que não é `Uint8List`).

Medido de novo com o heap de N19 estável (2a6ef80e, objetos com campos de
8 bytes e mapa de referências): **902 989 instruções/req** — o total das
três mudanças de §9.6–§9.8 sobre o ponto de partida fica em −26,3%.

### 9.9 Rodada 2: constantes, closures, cabeçalhos e o servidor que caía (medido em 2026-09-29)

Ponto de partida: o `main` depois da rodada de caches inline de despacho
(`df.classe`/`df.seletor` em linha, `==` de `String` numa chamada, a
string canônica de um caractere do `s[i]`, a limpeza da exceção só quando
pendente — `corpus/nativo/56_despacho_em_linha_e_igualdade_de_string.dart`,
completo e igual à VM) e do heap novo de N19: **593 029 instruções/req**
(callgrind, `/`, §9.1). Cada item abaixo foi medido sozinho, na ordem:

| mudança | instruções/req |
|---|---:|
| ponto de partida | 593 029 |
| `const` primitiva vira a constante (`lower/const_primitiva.rs`) | 561 006 |
| closure de ambiente direto (`lower/closures.rs`) | 529 500 |
| `String.fromCharCodes` de lista em Latin-1 numa cópia; cache do cabeçalho tipado | 513 029 |
| Dart AOT (`dart compile exe`, o mesmo roteiro) | 107 613 |

Na rota `/json`: 606 103 (Dart AOT 125 788).

* **`const` primitiva** (`lower/const_primitiva.rs`, chamado de
  `ler_global`): a leitura de uma `const` `int`/`double`/`bool` cujo
  inicializador é literal, negação ou aritmética de inteiros sobre outras
  dessas constantes vira a constante — o que o CFE faz com toda constante.
  Antes cada `_State.X`/`_CharCode.LF` era o getter preguiçoso do global (a
  bandeira, a carga, às vezes uma chamada): o `switch (_state)` do
  `_HttpParser._doParse` comparava o estado com uma dezena de globais por
  byte da requisição; agora é uma tabela de saltos (o `_doParse` foi de 26
  para 13 mil instruções/req). A chave estrutural de `const [topo]` passa a
  ser a de `const [7]` (antes as duas constantes eram objetos diferentes).
  `corpus/nativo/59_constantes_primitivas_e_switch.dart`.
* **Closure de ambiente direto** (`lower/closures.rs`): com um valor só no
  ambiente — o `this` sem capturas, ou uma captura `Ref` (ou a célula
  dela) sem `this` —, a closure guarda o valor no lugar do ambiente e o
  corpo o recebe como o parâmetro `env` (`dartforge_closure_nova_direta`,
  `AllocClosureTipada { direto }`): uma alocação a menos por closure (59
  ambientes por requisição → 30). Fora: corpo `async`/gerador, genérica,
  tupla de tipos, `late`. O `==` de duas closures da mesma expressão sobre
  o mesmo objeto passa a ser `false`, como na VM (o runtime as igualava
  quando o ambiente tinha um valor só).
  `corpus/nativo/60_closures_de_ambiente_direto.dart`.
* **`String.fromCharCodes` de lista do runtime**
  (`sdk_nativo/core/string_patch.dart`, `_deCodigos`;
  `DartForge_string_de_codigos`): a lista toda em Latin-1 vira a string
  numa cópia só, em vez do `_scanCodeUnits` e de um `_setAt` (uma chamada ao
  runtime) por unidade — os nomes e valores de cabeçalho do `_HttpParser`.
  De passagem, dois defeitos antigos dos natives de string: o
  `_allocateFromTwoByteList` com `Uint16List` (pânico "lista esperada") e o
  `ArgumentError` do `_createFromCodePoints` acima de U+10FFFF.
  `corpus/nativo/61_string_de_codigos.dart`.
* **Cache do cabeçalho tipado** (`lower/tipados.rs`, `cabecalho_tipado`):
  a lista tipada relida de um campo a cada acesso (`_buffer![_index++]`,
  `_buffer[_length + i] = bytes[i]`) passa por um cache por função e tipo
  de elemento: o último handle, num local `Ref` (enraizado: a lista não
  morre e o handle não é reusado enquanto está no cache), e o cabeçalho
  dele. A falha chama `dartforge_typed_cabecalho_na_falha`, declarada sem
  `speculatable` (com a função pura, o LLVM chamava antes do teste e o
  cache virava um `select`). Lista de parâmetro ou de local fica com a
  chamada pura, que o LLVM tira do laço e vetoriza (`tipados/produto_f64`
  6,3 ms e `fnv_bytes` 13,2 ms, iguais). `typed_cabecalho` por requisição:
  394 → 58 chamadas. `corpus/nativo/62_cache_de_cabecalho_tipado.dart`.

**O servidor caía quando o cliente fechava a conexão** (defeito de §9.7,
achado aqui): um cliente que mandava a requisição e fechava antes da
resposta derrubava o processo (SIGSEGV no `_Future._propagateToListeners`,
pelo `_asyncCompleteError`); o `wrk` fecha as conexões ao fim de cada
medida, então nenhuma medida de §9.2 em diante sobrevivia à segunda
rodada. A função local direta `handleError` grava `listenerHasError`
(`bool`, capturado por endereço) com `store i64` num `alloca i1` do
chamador: os 7 bytes a mais zeravam o ponteiro da área de globais guardado
ao lado. O `store` por um ponteiro que não é `alloca` da função agora tem a
largura do valor (`llvm/mod.rs`).
`corpus/nativo/63_funcao_direta_grava_bool_por_endereco.dart`.

Correção de tudo: corpus nativo inteiro igual à VM no AOT e com
`--gc-stress` (fora os programas em obra de outras frentes).

**Medido com o `wrk`** (`scripts/bench-http.py`, 3 repetições, 5 s; carga
da máquina 6–8 em 4 CPUs):

| medida | DartForge (§9.2) | DartForge agora | Dart VM (JIT) | Dart AOT |
|---|---:|---:|---:|---:|
| req/s, `/`, 1 conexão | 1913 | 3991 [3143–4263] | 5489 | 9372 |
| req/s, `/`, 64 conexões | 1826 | 3608 [3173–3626] | 9484 | 12193 |
| req/s, `/json`, 1 conexão | 1438 | 3396 [3308–3511] | 5443 | 6828 |
| req/s, `/json`, 64 conexões | 1977 | 3422 [3094–3430] | 7968 | 10700 |
| CPU/req `/`, 1 conexão, µs | 335 | 216 | 146 | 84 |
| CPU/req `/json`, 64 conexões, µs | 473 | 286 | 117 | 92 |
| p50 `/`, 64 conexões, ms | 33,9 | 17,2 | 6,4 | 4,9 |
| RSS em repouso, MB | 14,7 | 17,0 | 150 | 7,3 |
| RSS de pico, MB | 16,2 | 28,1 | 166 | 17,7 |

O ganho de req/s desde §9.2 soma o heap de N19, a rodada de despacho em
linha e esta. A RSS de pico subiu (16 → 28 MB) no mesmo intervalo: o
padrão de coleta do heap novo (as duas `Uint8List` de 8 KB por
requisição contam para o gatilho por bytes), a conferir com a frente do
heap.

**Onde vai agora** (self, `/`, 513 mil instruções/req): código Dart
compilado 26,8%; heap (slots, alocação, coleta) 20,9%; `malloc`/`free`/
`memset` da libc 18,3%; runtime e natives 16,7%; RTI 11,8%; classe do
receptor no despacho 5,2%. Os maiores, pela ordem:

1. **Alocação e coleta** (heap + libc, ~39%): cada `Uint8List(8192)` é um
   `calloc` (o `memset` de 21 mil instruções/req é dele), a coleta
   (`coletar`, 43 mil/req inclusivo) é disparada pelos bytes dessas
   listas; cada closure/ambiente/lista é um `Value` num slot mais
   `malloc`. `list_push` custa ~380 instruções por `add` (o
   `_headerField.add(byte)` do `_doParse`, 46 por requisição): duas
   `estimated_bytes`, `get` e `get_mut`. Dono: heap.
2. **Natives de string** (~40 mil/req): `codeUnitAt` (334 por requisição,
   48 instruções cada), `length` (242), `==`, `[]` — cada um uma consulta
   ao slot. Um cabeçalho de endereço fixo para o `Texto`, como o das listas
   tipadas (N17), deixaria o código gerado ler em linha e tirar dos laços;
   depende da representação no heap.
3. **RTI** (60 mil/req): `rti_avaliar` (180 por requisição, ~107 cada) e
   `rti_definir` (121, ~260 cada nas listas do runtime: `cid_do_runtime`,
   `set_metadado`, `ajustar_forma_da_lista`). Um cache no ponto de uso de
   `P<i>` pela classe e o metadado do cabeçalho do objeto (lidos em linha)
   tiraria a maioria das chamadas. Dono: `lower/rti.rs`/`tipos.rs`.
4. **A classe do receptor que não é objeto do espaço** (`value_class` do
   runtime, 245 por requisição, ~100 cada): strings, listas e listas
   tipadas no cache inline do seletor. Depende de a classe estar no handle
   ou num cabeçalho.
5. **Por chamada de função Dart** (1 185 por requisição): prólogo com
   `dartforge_contexto`, a área de globais e o quadro de raízes. Os
   getters e acessos de uma linha não entram em linha quando a chamada vai
   pelo seletor (`CodeUnits.[]`, 149 por requisição no `writeHeaders`).

### 9.10 Rodada 3 (N17): alocação, RTI no ponto de uso e o prólogo (medido em 2026-09-29, Windows)

**Máquina e método.** Windows 11, Intel i3-1215U de notebook (2 núcleos P,
4 E, 7,7 GB), com outros agentes compilando na mesma máquina. Sem `wrk` nem
`callgrind`. A carga vem de um cliente em Dart compilado com
`dart compile exe`: HTTP/1.1 com keep-alive, 1 ou 64 conexões, cada uma
manda a requisição seguinte ao receber a resposta inteira (`Content-Length`
ou *chunked*). Ele mede req/s e p50/p99, com 0,5 s de aquecimento e 2–3 s
de medida. Um roteiro em PowerShell sobe cada servidor e mede:

* afinidade fixa: servidor no processador lógico 2 (núcleo P), cliente no
  6, os dois com prioridade alta;
* CPU do servidor por requisição (`TotalProcessorTime`);
* RSS em repouso (`WorkingSet64` depois de subir e responder) e de pico
  (`PeakWorkingSet64` ao fim das duas cargas);
* os executores alternam a cada repetição, e a repetição inteira é
  descartada e refeita se um `rustc`/`clang`/`lld-link`/`dartforge` rodou
  durante ela.

Mesmo assim o ruído é de ±15–30% entre repetições (frequência variável do
notebook, a máquina disputada). Por isso a comparação de uma mudança é a
**razão por par** (os dois executores vizinhos na mesma repetição), com
mediana e faixa sobre 20 pares. Para separar o efeito de cada mudança, as
do emissor têm uma chave de medida na compilação (`DARTFORGE_SEM_CACHE_RTI=1`,
`DARTFORGE_SEM_QUADRO_NA_ENTRADA=1`). As variáveis `DARTFORGE_SEM_*` entram
na chave do SDK em cache (`sdk_modulo.rs`). A do alocador é de execução
(`DARTFORGE_ALOCADOR_SEM_CACHE=1`).

O perfil, sem `perf`: um amostrador em Python suspende a thread do isolado
a cada ~1–4 ms e percorre a pilha com o `StackWalk64` da `dbghelp`. A
simbolização usa o mapa do `lld-link` (`/map`, com os símbolos estáticos, por
um invólucro do `lld-link` num diretório de ferramentas à parte) e a tabela
de exportação das DLLs do sistema. O PDB de `--depuracao` não serve para
isso: só tem os símbolos públicos do runtime (o SDK vem em cache, sem
depuração), e as funções internas saíam com o nome da pública vizinha.

**Onde ia o tempo no Windows** (self, thread do isolado, `/`, 1 conexão,
servidor do `main` de partida), em % da CPU ativa (fora a espera no laço):

| categoria | % |
|---|---:|
| código Dart compilado | 31 |
| chamadas de sistema de E/S (`WSASend`, `ZwDeviceIoControlFile`) | 24 |
| `malloc`/`free` (`RtlAllocateHeap`/`RtlFreeHeap`/`GetProcessHeap`) | 10,7 |
| heap e coleta (`tirar_da_regiao`, `allocate`, `coletar`, `list_push`) | 8,8 |
| RTI (`rti_avaliar`, `rti_definir`, `rti_e`, `is_subclass`) | 6,3 |
| natives de string e lista | 6,1 |
| classe do receptor (`value_class`, `cid_do_valor_do_runtime`) | 2,9 |
| `dartforge_contexto` + `__chkstk` | 2,5 |

A diferença mais clara para o §9.9 é o `malloc` do Windows: o `System` do
Rust é o `HeapAlloc` do heap do processo, com o `GetProcessHeap` e a trava do
heap a cada closure, ambiente, vetor de lista ou texto.

**O que entrou** (cada item medido sozinho, razão por par, 20 pares):

* **Alocador com lista livre por thread** (`crates/runtime/src/alocador.rs`,
  fragmento novo). É o `#[global_allocator]` das `staticlib` do executável
  (`aot` e `dll`); o JIT e os testes do crate continuam com o do processo.
  Pedidos de até 512 bytes com alinhamento ≤ 16 vão a classes de 16 em 16
  bytes, cada uma com a sua lista livre por thread (até 64 KiB por classe).
  Todo bloco de classe sai do `System` com o tamanho da classe, então pode
  voltar a qualquer lista ou ao sistema. As listas só valem nas threads de
  isolado (`dartforge_iniciar` e o `Isolate.spawn` as ligam; o fim da thread
  do isolado as esvazia). No perfil, o `malloc`/`free` vai de 10,7% para ~3%
  da CPU ativa. CPU/req com 1 conexão: razão **0,94** [0,73–1,22];
  req/s 1,00 [0,79–1,35].
* **`P<i>` com cache no ponto de uso** (`llvm/mod.rs`,
  `emitir_rti_avaliar_em_cache`; `tipos.rs`, `dartforge_rti_avaliar_cache`).
  Serve ao `dartforge_rti_avaliar` sem tupla e com `this`: duas palavras na
  área do isolado (a mesma dos caches de seletor, zerada pela recarga)
  guardam a chave do tipo de `this` e o resultado. A chave é o metadado do
  cabeçalho do objeto, ou `-16 - classe` sem metadado, a mesma da memória do
  runtime (`chave_do_valor`). Com um objeto do espaço e a chave igual, a
  resposta sai com cargas, sem chamada; senão o runtime responde e grava.
  CPU/req: razão **0,89** [0,56–1,45]; req/s 1,08 [0,72–1,55]. Continua
  indo ao runtime: o receptor que não é objeto do espaço, a tupla de
  função (`M<i>`) e os pontos em que o tipo de `this` alterna, como o
  `_Future._propagateToListeners` com `Future`s de `T` diferentes, onde o
  cache de uma entrada troca a cada vez.
* **Memória direta no `rti_definir` das coleções** (`tipos.rs`,
  `memo_definir`). O `List<E>` de um literal visto como `_GrowableList<E>`
  (e o mesmo para `Map` e `Set`) passava por `como_supertipo`, `substituir`,
  um `Vec` novo e o `internar` a cada coleção criada (~1,2% das amostras).
  Agora é uma vaga de `[classe, tipo pedido, tipo gravado]`, limpa com as
  outras memórias. Não foi medido sozinho (vai junto com o item anterior no
  mesmo runtime).
* **`list_push` com uma consulta ao slot** (`heap.rs`). Eram `get`,
  `get_mut` e outro `get` para a estimativa; agora a diferença da capacidade
  reservada do vetor, e o `push` que não realoca sai sem mexer nos
  contadores (salvo no `--gc-stress`, que continua coletando). Não foi
  medido sozinho: menor que o ruído.
* **O quadro de raízes no topo do bloco de entrada** (`llvm/mod.rs`, o
  prólogo). O `alloca` do quadro saía depois do `df.obter_area`, que é
  `alwaysinline` e tem desvios. Com isso ele deixava de estar no bloco de
  entrada e virava **alocação dinâmica**: `__chkstk` e o ajuste de pilha a
  cada chamada da função, e o inliner não o levava para o bloco de entrada
  de quem chama. O `__chkstk` sai do perfil (era 0,6%) e o executável do
  servidor cai de 15,1 para 13,7 MB. CPU/req: razão 1,05 [0,75–3,7]; req/s
  0,97 [0,27–1,17]. **Sem ganho mensurável** com este ruído; fica porque
  remove as alocações dinâmicas e o código delas.

Correção: `corpus/nativo/70_rti_cache_no_ponto_de_uso.dart` (o mesmo ponto
de uso com receptores de outros argumentos de tipo, outras classes,
subclasses que fixam o argumento, sem tipo gravado; as coleções criadas em
métodos genéricos, com `is`, `runtimeType` e a covariância do `add`/`[]=`;
20 000 objetos; `Isolate.run`) é igual à VM no AOT, no JIT, com
`--gc-stress` e no AOT `--optimize`. Corpus nativo 76/76 no AOT, no JIT e
com `--gc-stress --limite-exec 60`. O `02_io_assincrono` só passa sem `HOME`
no ambiente: o oráculo em cache foi gravado sem ele; não tem relação com
esta rodada. Testes de `dartforge-runtime` verdes, incluindo os 4 novos do
alocador (`tests/alocador.rs`).

**Antes e depois** (10 repetições, 3 s, 1 e 64 conexões; "antes" é o
servidor compilado pelo `main` de partida desta rodada; mediana [faixa]):

| medida | DartForge antes | DartForge depois | razão por par | Dart AOT |
|---|---:|---:|---:|---:|
| req/s, 1 conexão | 5014 [1635–5716] | 4318 [2229–6529] | 1,14 [0,70–1,41] | 5804 [2899–8078] |
| req/s, 64 conexões | 4050 [1855–6472] | 5548 [2141–7694] | 1,17 [0,65–1,53] | 7875 [4643–10793] |
| CPU/req, 1 conexão, µs | 221 [129–492] | 180 [110–353] | 0,82 [0,69–1,26] | 102 [71–236] |
| CPU/req, 64 conexões, µs | 241 [147–525] | 182 [127–450] | 0,86 [0,68–1,57] | 123 [94–218] |
| p50 / p99, 1 conexão, ms | 0,229 / 0,674 | 0,222 / 0,565 | — | 0,160 / 0,401 |
| p50 / p99, 64 conexões, ms | 15,1 / 26,5 | 12,5 / 17,7 | — | 0,40 / 189 |
| RSS em repouso, MB | 18,5 | 18,8 | — | 17,0 |
| RSS de pico, MB | 25,5 | 25,7 | — | 27,7 |

As medianas de req/s de cada executor vêm de repetições diferentes (a de
1 conexão "antes" saiu maior que a "depois"). A comparação é a razão por
par: +14% req/s e −18% de CPU por requisição com 1 conexão, e +17% e −14%
com 64. As faixas se sobrepõem, e o número de pares é pequeno. No Windows o
Dart AOT tem RSS de pico maior que o DartForge. O p99 de 64 conexões do Dart
AOT varia muito entre rodadas (59–365 ms).

**O que falta, e por quê:**

1. **A classe do receptor que não é objeto do espaço** (item 4 de §9.9,
   ~2,9%). Uma classe por slot lida em linha exige invalidar a entrada
   quando a classe de um valor do runtime muda depois da criação (a lista
   que vira fixa ou imutável pelos conjuntos `fixas`/`imutaveis`). Não
   entrou nesta rodada: o risco de semântica é maior que o ganho, que
   ficaria abaixo do ruído desta máquina.
2. **`dartforge_contexto` por chamada** (~1,3%). Um TLS do próprio módulo
   (cache preguiçoso do ponteiro do contexto) tiraria a chamada do prólogo,
   mas o JIT (ORC no COFF) não foi verificado com variáveis `thread_local`.
3. **As duas `Uint8List(8192)` e o gatilho da coleta.** Aqui cada uma é
   ~0,4% (o `calloc`), e a coleta inteira (`coletar`) ~2,2%. A coleta menor
   vem a cada ~60 requisições (`LIMITE_JOVEM` de 2 MiB, ~33 KB estimados por
   requisição, metade dessas listas). A velha cresce ~35 KB por coleta menor
   até a completa em ~4,2 MB. No Windows o RSS de pico ficou em 25,5–25,7 MB,
   antes e depois, abaixo do Dart AOT (27,7). O pico de 28 MB do §9.9 é do
   Linux e não foi remedido aqui.
4. **Cache de 2 vias no `P<i>`** para os pontos em que o tipo de `this`
   alterna (o que resta do `rti_avaliar`, ~1%).
5. Refazer tudo no Linux com o `callgrind` (§9.1): as razões acima são do
   Windows, com o `malloc` do sistema mais caro que o da glibc.

## 10. JSON (`dart:convert`): `jsonEncode`/`jsonDecode` (medido em 2026-09-29)

### 10.1 O benchmark

`bench/desempenho/json.dart` segue o padrão de `comum.dart`. Os documentos
saem de um gerador determinístico:

* pequeno: 1 080 unidades, decodificado 2 000 vezes por rodada;
* médio: 101 447 unidades, 30 vezes;
* grande: 5 974 162 unidades, uma vez.

Eles têm objetos aninhados, listas de `int` (até ~2^62, com negativos) e
de `double` (dízimas, expoentes, `0.1 + 0.2`), e strings com aspas,
barras, controles, acentos, CJK e pares substitutos (emoji, clave de sol).
Os núcleos são `decode_*`, `encode_*`, `indentado`
(`JsonEncoder.withIndent`), `utf8_bytes` (`json.fuse(utf8)`, ida e volta
em bytes) e `reviver`.

Antes das medidas, o programa imprime a soma de todas as unidades de cada
documento e confere a ida e volta (`jsonEncode(jsonDecode(s)) == s`). Cada
núcleo imprime uma soma de conferência da estrutura. A saída inteira é
igual à da VM e à do `dart compile exe`.

### 10.2 Dois defeitos achados pelo benchmark

* **`jsonDecode` quadrático.** O `_JsonStringParser` chama o
  `Double_parse` (o `_parseDouble`) para todo número com mais de 15
  algarismos ou com expoente fora de ±22. Para ler a fatia do número, o
  native copiava a string inteira (`texto_de`): com o documento de 6 MB,
  cada número copiava 12 MB, e `decode_grande` levava 38–70 s por rodada.
  Agora o native lê só a fatia, por um buffer da pilha quando é ASCII curto
  (`nativos_listas.rs`).
* **`double` errado.** O mesmo native tem duas declarações no SDK: `double?`
  no `double._nativeParse` e `double` no `_parseDouble` do
  `convert_patch.dart`. O runtime devolve sempre a caixa (`Ref`), e a
  chamada com retorno `double` lia o handle como se fosse os bits do
  número: `2.12e-7` voltava `6.4e-323`. Agora `nativos::RETORNO_REF` marca
  os natives nessa situação, e tanto o `external` (`externos.rs`) quanto a
  chamada direta (`sdk_fonte.rs`) chamam com `Ref` e convertem para o tipo
  declarado.

### 10.3 O que entrou

1. **`String.codeUnitAt` e `length` em linha** (`lower/textos.rs`, item 2
   de §9.9), sem mexer no `heap.rs`.
   * As unidades de um `Texto` moram num `Vec` que não muda depois de
     criado (a string é imutável, e o `writeInto*String` grava no lugar).
     Por isso o endereço delas é fixo enquanto a string vive.
   * Duas funções puras dão o endereço e o comprimento:
     `dartforge_texto_dados` (com o bit 63 ligado quando cada unidade tem
     dois bytes) e `dartforge_texto_len`. As duas são `memory(none)` e
     leem o heap sem o `borrow` (`heap_sem_emprestimo`).
   * O código gerado lê a unidade depois de um teste de limites. Índice
     fora da faixa, ou valor que não é string do runtime, vai ao native de
     antes, que lança o `RangeError` da VM.
   * A string lida de um campo tem um cache por ponto de leitura: o handle
     enraizado, o endereço e o comprimento. A falha do cache é uma chamada
     só (`dartforge_texto_na_falha`), que grava o comprimento no local do
     cache.
   * Continua pendente um cabeçalho de endereço fixo, como o das listas
     tipadas: o cache não sobrevive entre chamadas, e uma função pequena
     que lê o campo (o `_getCharUnsafe` do parser) falha a cada chamada.
2. **`is` pela classe** (`nucleo.rs`, `llvm/externs.rs`).
   `dartforge_is_subclass` ganhou uma tabela de acesso direto de 512 vagas
   na frente do mapa. Passou a ser declarado `memory(read) nounwind
   willreturn`, sem os efeitos conservadores (ele não aloca nem lança).
   Com isso, os testes `is String`/`is num`/`is Map` do `writeJsonValue`
   deixam de levar as referências vivas ao quadro de raízes.
3. **`int.toString` e `double.toString`** (sobreposições
   `sdk_nativo/core/integers.dart` e `double.dart`, `nativos_numeros.rs`).
   * `_Smi.toString`: a tabela pequena da VM continua devolvendo a mesma
     string para -99..99. Fora dela, uma chamada ao runtime. Antes era o
     laço da VM, com um `_setAt` (uma chamada) a cada dois dígitos e o `~/`
     e o `remainder` pelo despacho de `num`.
   * `_Double.toString`: o cache de 8 entradas compara os bits do `double`
     (é o que o `identical` compara), guardados numa `List<int>` ao lado da
     lista de textos. Antes cada entrada custava uma leitura pelo seletor,
     uma caixa e uma chamada ao runtime. O texto devolvido é o mesmo objeto
     nos mesmos casos.
   * O `Double_toString` escreve num buffer da pilha, sem as quatro
     `String` que alocava antes.
4. **`StringBuffer`** (`sdk_nativo/core/string_buffer_patch.dart`,
   `nativos_strings.rs`).
   * `write` de uma `String` é uma chamada só: o teste, o `isEmpty` e o
     `length` ficam no runtime, com duas consultas ao slot.
   * `writeCharCode` escreve direto no acumulador. Antes criava uma string
     de um caractere por chamada. Os `RangeError` agora são os do
     `writeCharCode` da VM (antes eram os do `String.fromCharCode`).
5. **`_JsonListener` e `_JsonStringParser`** (sobreposição
   `sdk_nativo/convert/convert_patch.dart`, gerada do arquivo da VM).
   * O listener acrescenta aos contêineres que ele mesmo criou (os pares de
     um objeto, os elementos de um vetor e a pilha, todos `[]` de
     `dynamic`/`Object?`) pelo native `DartForge_json_acrescentar`. Isso
     evita o despacho do `add` (saber a classe de uma lista do runtime
     custa duas consultas a `HashSet`) e a conferência do `E` na entrada
     uniforme.
   * `parse`, `parseString` e `parseNumber` foram copiados do mixin para o
     `_JsonStringParser`, com o `chunk` num local. Assim o endereço das
     unidades sai dos laços.

**Correção.** `corpus/nativo/75_json_textos_e_numeros.dart` saiu igual à
VM no AOT, no JIT e com `--gc-stress`. Ele cobre:

* `codeUnitAt`/`length` de parâmetro, de campo e de campo trocado no meio
  do laço, com duas unidades, string vazia e índice fora da faixa;
* números do JSON com 17 algarismos, expoentes, `-0.0`, `5e-324` e
  inteiros grandes;
* `int.toString` e a identidade da tabela pequena, e o cache do
  `double.toString`;
* `StringBuffer` com `writeCharCode` de par substituto e fora da faixa;
* ida e volta com escapes, surrogates soltos, `withIndent`, `fuse(utf8)` e
  reviver.

O corpus nativo passou 76/76 nos três modos. Os testes de
`dartforge-runtime` estão verdes; entre eles, um confere a escrita nova do
`double` contra a antiga em 400 mil valores.

### 10.4 Medido (Windows 11, 8 núcleos; Dart SDK 3.6.2)

A tabela dá o tempo estável por núcleo, em ms: a mediana das rodadas
depois da primeira e, entre colchetes, a faixa (mínimo–máximo) de 3
repetições alternadas.

A máquina tinha outros agentes compilando e medindo, e o mesmo binário
variou até 3×: compare as medianas, e leia as faixas como o tamanho do
ruído. "Antes" é o `main` do início (`30eb778f`). "Depois" inclui também o
que outras frentes mudaram na mesma árvore no intervalo (alocador, RTI).

| núcleo | antes | depois | Dart AOT | razão antes | razão depois |
|---|---:|---:|---:|---:|---:|
| decode_pequeno | 614,9 [331–1666] ¹ | 178,6 [127–433] | 31,4 [24–43] | 19,6× | 5,7× |
| encode_pequeno | 293,3 [262–979] | 188,2 [103–509] | 33,0 [25–46] | 8,9× | 5,7× |
| decode_medio | 585,7 [520–1777] ¹ | 394,0 [175–935] | 54,6 [42–75] | 10,7× | 7,2× |
| encode_medio | 406,9 [341–654] | 148,6 [135–401] | 57,3 [42–67] | 7,1× | 2,6× |
| decode_grande | 47 873 [38 060–70 324] ¹ | 508,7 [421–1539] | 126,7 [100–192] | 378× | 4,0× |
| encode_grande | 668,3 [492–847] | 290,1 [283–752] | 123,1 [88–138] | 5,4× | 2,4× |
| indentado | 274,0 [207–405] | 115,1 [114–326] | 88,8 [42–132] | 3,1× | 1,3× |
| utf8_bytes | 533,0 [352–685] ¹ | 304,8 [238–804] | 78,7 [41–104] | 6,8× | 3,9× |
| reviver | 269,1 [227–402] ¹ | 124,5 [114–298] | 49,8 [29–57] | 5,4× | 2,5× |

¹ Com o `double` errado: resultado diferente do da VM.

* **Tempo total do processo:** 351 s → 13,4 s (Dart AOT 4,2 s).
* **Pico de memória residente** (`PeakWorkingSetSize`): 477 → 317 MB (Dart
  AOT 195). A queda não foi atribuída a uma mudança só, porque o alocador
  de outra frente entrou no mesmo intervalo.
* **`bench/desempenho/textos`**, as mesmas três versões: `construir` 187 →
  159 ms (Dart AOT 81), `hashes` 74 → 63 ms (38), pico de memória 87 → 88
  MB.

Ganho de cada passo, pelo mínimo de 3–4 execuções alternadas do mesmo
programa (30 decodificações ou codificações do documento médio). Com o
ruído da máquina, a precisão não passa de ~10%:

| passo | decode | encode |
|---|---:|---:|
| só as correções de 10.2 | ~200 ms (Dart AOT 34) | ~190 ms (35) |
| + 1–2 (em linha, `is`) | 222 | 194 |
| + 3–4 e o cache por ponto de leitura | 227 | 165 |
| + `is` sem efeitos, `write` numa chamada | 229 | 129 |
| + 5 (listener e parser) | 211 | 129 |

Micro (a mesma máquina, ms):

| operação | antes | depois | Dart AOT |
|---|---:|---:|---:|
| `int.toString` × 1 milhão | 920 | 140 | 20 |
| `double.toString` × 300 mil | 800 | 145 | 59 |
| `writeCharCode` × 1 milhão | 130 | 28 | 7 |
| `codeUnitAt` de campo sobre 1,5 milhão de unidades | — | 4,8 | 3,3 |

### 10.5 Onde vai o tempo agora

O perfil é por amostragem: a thread é suspensa a cada ~1 ms e o RIP é
lido. A simbolização usa o PDB, com o runtime compilado com as tabelas de
linha (`DARTFORGE_LIB` apontando para a `staticlib` com `debug =
"line-tables-only"`).

Na decodificação do documento médio:

* **alocação e coleta, ~21%:** só `RtlAllocateHeap`/`RtlFreeHeap` do
  `ntdll` são 7%. Cada string é um slot mais o `Vec`; cada lista, o `Box`
  do cabeçalho mais o `Vec`.
* **classe do receptor, ~12%:** em `cid_do_valor_do_runtime`, cada
  `value_class` de uma lista do runtime procura o handle em `imutaveis` e
  em `fixas`, dois `HashSet`.
* **RTI, ~10%:** `rti_como_em`, `rti_e` e `rti_avaliar`, na covariância das
  entradas `$c` do `_Map` e da `_GrowableList` e nos tipos dos
  `_Map<String, dynamic>` criados.
* **o parser em Dart, ~9%.**
* **as consultas de string ao runtime, ~4%:** a falha do cache e as funções
  puras.

Na codificação:

* **alocação, ~17%;**
* **classe do receptor, ~12%;**
* **as listas, ~10%:** a do `writeMap` (`List.filled` mais a closure do
  `forEach`) e o `_seen` do `_checkCycle`;
* **o acumulador do `StringBuffer`, ~9%:** `sb_escrever_se_texto` e as
  consultas ao slot;
* **RTI, ~7%.**

O que falta, pela ordem do ganho medido. As três primeiras são de outras
frentes (heap, despacho e RTI):

1. A classe da lista no cabeçalho (`CabecalhoDeLista`), sem os dois
   `HashSet` em `cid_do_valor_do_runtime`.
2. Alocação: a string num bloco só (hoje é o slot mais o `Vec`).
3. As conferências de covariância do `_Map.[]=` e do `add` quando o `E` é o
   tipo topo.
4. Um cabeçalho de endereço fixo para o `Texto`: uma chamada pura em vez
   de duas, e um cache que sobrevive entre chamadas. O mesmo problema
   aparece no cache de `tipados.rs`, que falha a cada chamada do `writeByte`
   do `_JsonUtf8Stringifier` (4% do `utf8_bytes`).
5. `writeMap` sem a lista intermediária nem a closure.

**Achado de passagem.** Numa versão intermediária da sobreposição, o
`_JsonStringParser` usava os estáticos do mixin `_ChunkedJsonParser` sem
qualificação. Eles estão fora do escopo léxico, e a VM daria erro de
compilação. O front-end aceitou em silêncio e gerou acesso dinâmico. A
saída saiu certa, mas o programa deveria ter sido recusado.

## 11. Classe do valor em linha e coleta menor mais barata (medido em 2026-09-30, Windows)

São as propostas 1 e 2 de `PLANO-TAMANHO-DESEMPENHO.md` §2.

### 11.1 O que entrou

1. **A classe de cada slot num vetor denso** (`runtime/src/heap.rs`, `Heap::classes`).
   * **O vetor.** É um `ClasseDoSlot { classe: i32, marcas: u32 }` por slot, paralelo a
     `Heap::slots`.
   * **Quando a classe é escrita.** Ela é calculada quando o slot recebe o valor (`guardar`) e é
     a mesma resposta de `dartforge_value_class`: com o SDK da fonte, a classe do SDK; sem ele,
     os códigos negativos de antes.
   * **Quando ela é refeita:**
     * quando uma marca muda (`marcar_fixa`, `marcar_imutavel`: são os únicos pontos em que a
       classe de uma lista muda);
     * quando chega a tabela dos ids do SDK (`dartforge_registrar_cids` →
       `definir_cids_do_runtime`, que refaz os slots já alocados).
   * **Por que isso basta.** O valor de um slot nunca troca de variante. A troca de conteúdo das
     mensagens entre isolados (`portas.rs`) mantém a variante.
   * **O que saiu.** Os `HashSet` `fixas`/`imutaveis` foram removidos. Só os objetos do espaço
     marcados como imutáveis, que são raros, continuam num conjunto (`imutaveis_do_espaco`).
   * **O que o contexto publica.** O endereço e o comprimento do vetor ficam nos deslocamentos
     336 e 344. O vetor é estendido até a capacidade de `slots`, e o endereço só é republicado
     quando ele cresce.
   * **O `df.classe` (`llvm/mod.rs`).** Para um slot (`h > 0`, `h & 3 == 0`), ele confere o
     índice e lê o `i32`. O endereço e o comprimento são relidos a cada uso. Índice fora da
     faixa ou classe desconhecida (`i32::MIN`, que é o slot livre) vão ao runtime.
2. **`is C` por mapa de bits** (`runtime/src/nucleo.rs`, `MapasDeSubtipo`; `df.subclasse` em
   `llvm/mod.rs`). É o equivalente, montado pelo runtime, da tabela `@df.sub.<C>[cid]`.
   * **Montagem.** A primeira consulta de um alvo `C` que vai ao runtime
     (`dartforge_is_subclass`) monta o mapa das classes `cid <: C` pelo grafo inverso.
   * **Publicação.** O contexto publica a tabela de ponteiros por alvo (352), o número de alvos
     (360) e a largura em bits (368).
   * **O caminho rápido.** A partir daí, o `is C` do código gerado lê um bit, sem chamada.
     Continuam indo ao runtime: o `cid` fora da largura, o alvo sem mapa e os ids que não são
     densos (`_Type` `0x3FFF_FF01`, as formas de record `0x4000_0000+`).
   * **Invalidação.** Toda relação nova de subclasse (`dartforge_register_subclass`: a carga e
     cada recarga do JIT, que refaz os registros) descarta todos os mapas. Por isso nenhum mapa
     responde por um grafo antigo, inclusive no código mantido de gerações anteriores (J04),
     que lê os mesmos mapas do runtime.
   * **A tabela de ponteiros não é realocada enquanto houver mapas.** Ela nasce com uma entrada
     por classe da largura. O motivo é que `dartforge_is_subclass` continua declarado
     `memory(read)`, então um laço pode reter um ponteiro lido antes da chamada. Só o
     `register_subclass`, uma chamada comum para o LLVM, solta os mapas.
3. **Coleta menor** (`heap.rs`, `coletar`):
   * os bytes de cada slot ficam guardados na alocação (`bytes_do_slot`, somados pelo
     crescimento que `list_push`/`set_add` contam), e a menor desconta os mortos sem ler o
     valor de novo; a completa refaz a estimativa pelo valor e a regrava;
   * as marcas `late` gravadas desde a última coleta ficam numa lista (`late_novos`), e a menor
     confere só essas: um velho não morre na menor. Antes, era um `retain` da tabela inteira,
     com uma entrada por objeto vivo com campo `late`;
   * `imutaveis_do_espaco`, `iteracoes_ativas` e `origens` só são filtrados quando não estão
     vazios. `fixas`/`imutaveis` já não existem.
4. **Correção achada pelo teste novo:** `(x: 1) is Record` e `is Object` davam `false` com o
   SDK da fonte, porque a forma de record só tinha a aresta para o id 0. Agora ela também tem
   as arestas para as classes `Record` e `Object` do `dart:core` (`lower/mod.rs`).

**Chave de medida:** com `DARTFORGE_SEM_CLASSE_EM_LINHA=1` na execução, o runtime não publica
os dois vetores, e o código gerado vai sempre ao runtime. A resposta é a mesma.

### 11.2 Medido

**Condições.** AOT com `--optimize`, no mesmo notebook de §9.10, disputado com os builds de
outras frentes:

* **JSON:** 8 repetições alternadas, descartando as que tiveram build no meio. Cada célula é
  mínimo / mediana, entre as repetições, da mediana das rodadas depois da primeira.
* **"Antes":** o compilador do início desta rodada.
* **"Depois":** a árvore do fim da rodada, **com as mudanças das outras frentes do intervalo**.

| núcleo (ms) | antes | depois | depois, sem em linha | Dart AOT | razão antes/depois |
|---|---:|---:|---:|---:|---:|
| total do processo (s) | 17,1 / 18,8 | 13,9 / 16,6 | 15,1 / 17,6 | 3,5 / 3,8 | 1,19 [0,98–1,39] |
| decode_pequeno | 153 / 193 | 120 / 127 | 134 / 157 | 32 / 35 | 1,27 |
| encode_pequeno | 180 / 190 | 99 / 105 | 105 / 186 | 32 / 35 | 1,82 |
| decode_medio | 331 / 384 | 227 / 280 | 268 / 317 | 44 / 46 | 1,42 |
| encode_medio | 233 / 257 | 183 / 212 | 201 / 236 | 46 / 51 | 1,21 |
| decode_grande | 640 / 723 | 529 / 582 | 606 / 653 | 118 / 134 | 1,21 |
| encode_grande | 443 / 514 | 407 / 490 | 438 / 456 | 105 / 114 | 1,09 |
| indentado | 189 / 211 | 122 / 171 | 169 / 188 | 55 / 59 | 1,22 |
| utf8_bytes | 410 / 515 | 356 / 408 | 390 / 463 | 56 / 76 | 1,17 |
| reviver | 203 / 236 | 168 / 191 | 193 / 210 | 39 / 46 | 1,22 |

A razão pareada "antes / depois sem em linha" dá 1,11 no total. A parte em linha (`df.classe`
e `df.subclasse`) responde por uns 7%, e o runtime (vetor e coleta) mais as outras frentes pelo
resto. A saída foi igual nos três executáveis.

**Perfil do JSON** (self, thread do isolado, ~2 000 amostras por pilha em 10 s, simbolizado
pelo `/map` do `lld-link`), em % das amostras:

| categoria | antes | depois |
|---|---:|---:|
| código Dart compilado | 22,4 | 29,0 |
| classe do valor (`value_class`, `cid_do_valor_do_runtime`, `is_subclass`) | 12,6 | 0,7 |
| coleta (`coletar`, `trace`, `marcar_pendentes`) | 10,0 | 4,9 |
| `malloc`/`free` | 8,1 | 10,5 |
| listas do runtime | 8,1 | 10,8 |
| strings | 7,6 | 8,0 |
| alocação no heap | 5,8 | 6,8 |
| RTI | 4,1 | 4,5 |
| `dartforge_contexto` | 0,6 | 1,1 |

`value_class` (5,5%), `cid_do_valor_do_runtime` (5,4%) e `is_subclass` (1,7%) saem do perfil.
`Heap::coletar` sozinho cai de 8,6% para 2,5%. O resto sobe em proporção porque o total caiu.
O `dartforge_contexto` sobe um pouco: é a chamada do `df.classe` nas funções sem `%ctx`.

**Servidor HTTP.** 4 executores alternados. A medida foi interrompida depois de 4 repetições
válidas (de 12), com builds concorrentes. Mediana [faixa]:

| medida | antes | depois | depois, sem em linha | Dart AOT |
|---|---:|---:|---:|---:|
| req/s, 1 conexão | 2955 [2333–3331] | 2701 [2557–3185] | 2879 [2612–2908] | 4027 [3833–4152] |
| req/s, 64 conexões | 3259 [2902–3570] | 3196 [2805–4211] | 2955 [2769–3688] | 5771 [4961–6282] |
| CPU/req, 1 conexão, µs | 259 [234–346] | 303 [231–318] | 276 [269–299] | 157 [148–175] |
| CPU/req, 64 conexões, µs | 298 [273–328] | 309 [228–347] | 324 [260–355] | 199 [164–214] |
| RSS em repouso / pico, MB | 18,0 / 25,1 | 16,6 / 23,2 | 16,6 / 23,2 | 17,0 / 27,8 |

A razão pareada antes/depois é 1,00 [0,74–1,10] na CPU/req com 1 conexão e 0,97 [0,83–1,36]
com 64. **Não há ganho mensurável acima do ruído** com 4 pares. A estimativa era −2–3%, e a
classe do receptor era ~2,9% do servidor em §9.10.

### 11.3 Correção

* **Corpus novo:**
  * `85_is_as_listas_fixas_imutaveis`: `List.filled` com e sem `growable`,
    `List.unmodifiable`, `const`, `List.generate`, `toList`, `sublist`, `List.of`, strings de um
    e dois bytes, números, records e closures, por `is`/`as`/`runtimeType`/`whereType`, e o
    `UnsupportedError` de cada operação;
  * `86_is_hierarquias_mixins_interfaces`: herança, interfaces, mixins com `on` e aplicação
    nomeada, `sealed` com `switch`, erros e exceções do SDK e do usuário, `StringSink` do
    usuário, a matriz objeto × alvo nas duas ordens;
  * `87_classe_do_slot_sob_coleta`: 240 mil listas fixas, imutáveis e expansíveis que morrem e
    têm os slots reusados, campos `late` de jovens que morrem, e `Isolate.run`.
* **Resultado:** os três são iguais à VM no AOT, no JIT e com `--gc-stress --limite-exec 60`.
  O corpus nativo passa 81/82 nos três modos. A falha é `78_simd_conversao_de_pistas`, da
  frente de SIMD, em andamento: o LLVM recusa o IR (`<4 x float>` gravado como `i64`), sem
  relação com esta rodada.
* **Testes:** `cargo test --release -p dartforge-emit-native -p dartforge-runtime` verde. Entre
  os testes novos de `heap.rs`: a classe segue as marcas e a tabela do SDK chegada depois; a
  coleta menor purga só as marcas dos jovens mortos; o slot reusado não herda as marcas.

### 11.4 O que não foi verificado

* **`CONTAGEM_JOVEM`** não foi revista: a rodada foi encerrada antes. Pela conta, com
  `LIMITE_JOVEM` de 2 MiB, o gatilho por contagem (256 Ki alocações) só dispara antes do de
  bytes se a média for < 8 bytes por alocação, o que nenhum valor atinge. A conta não foi medida.
* **O perfil do servidor HTTP** depois da mudança.
* **Os testes de recarga do JIT** (`crates/jit/tests/hot_reload.rs`). A invalidação dos mapas
  segue o `register_subclass`, que a publicação de cada geração refaz. O caso "classe nova
  numa recarga muda um `is`" não tem teste próprio.
* **O Linux:** os deslocamentos novos do contexto têm `assert` de compilação, mas o corpus só
  rodou no Windows.

## 12. Espaço unificado: todos os valores no espaço de objetos (2026-09-30)

A especificação é `docs/NATIVO-ESPACO-UNIFICADO.md` (as fases B–D de
`PLANO-TAMANHO-DESEMPENHO.md` §2 de uma vez). Em uma frase por eixo: o handle
de slot (múltiplo de 4) sumiu; todo valor do runtime é um bloco com o
cabeçalho de 16 bytes e um cid fixo (1–65 para as classes do runtime, 128 em
diante para as demais); o coletor percorre o corpo pelo formato (`INSTANCIA`,
`BRUTO`, `REFS`) e uma lista grande tem cartões; os literais de string do AOT
são objetos estáticos; a cópia entre isolados é por bloco; e o caminho sem
SDK da fonte saiu (`lower/sdk_por_nome.rs`, `DARTFORGE_SDK_DA_FONTE`, o
despacho pelo nome em mundo fechado de `lower/despacho.rs` e os externs
`dartforge_list_*`/`map_*`/`set_*`/`string_*` que só ele emitia).

### 12.1 Estado

Implementado por pacotes em paralelo (P0–P5, §4 da especificação); a
integração (§5 da especificação) valida na ordem: testes de unidade do runtime
e do emissor, o módulo `heap` com `DARTFORGE_GC_VERIFICAR=1`, o corpus nativo
(82 programas de antes mais os novos 90–99) no AOT, no JIT e com
`--gc-stress`, o corpus `js`, a produção (`aot --optimize`), os benchmarks com
a saída da VM e a recarga (`crates/cli/tests/reload_estado.rs`,
`cli_preserva_o_estado_do_espaco_unificado_em_tres_recargas`).

Integrado em 2026-09-30, tudo verde: `corpus/nativo` (92 programas) no AOT, no
JIT e no `--jit-aot` (JIT × AOT idênticos), com `--gc-stress --limite-exec 60`
sem e com `DARTFORGE_GC_VERIFICAR=1`, e no `aot --optimize` contra a VM;
`corpus/js` no AOT (235); os testes do runtime, do emissor, do JIT e da CLI,
inclusive os `#[ignore]` de fixture, recarga e JIT, e `cargo test --workspace`.
O que a integração corrigiu está em §4.10 da especificação (itens 55–64).

Os programas novos (`corpus/nativo`):

* `90_textos_um_e_dois_bytes`, `91_textos_grandes_e_interpolacao`,
  `92_textos_identidade_hash_mapas` (duas bibliotecas),
  `93_textos_entre_isolados`, `94_utf8_e_bytes`: as strings (formas, pares
  substitutos, tamanhos das classes médias à região grande, `identical` de
  literais entre bibliotecas, hash da VM, strings entre isolados, UTF-8);
* `95_caixas_closures_records`: `identical` de `double` e `_Mint`, `-0.0` e
  `NaN`, closures que capturam `int`/`double`/mutáveis, tear-offs,
  `Function.apply`, records;
* `96_listas_formas_e_cartoes`, `97_mapas_conjuntos_grandes`: as formas
  compactas e a descompactação, os erros de lista fixa e imutável, e uma
  lista velha de 100 mil elementos que recebe objetos novos entre coletas
  (cartões);
* `98_tipadas_visoes_externas`: listas tipadas de todos os tipos, visões,
  `ByteData`, listas acima de 16 KiB, SIMD e `asTypedList` sobre `malloc`;
* `99_pressao_de_coleta`: pressão de coleta com gravação velho→jovem, para
  rodar com `--gc-stress` e `DARTFORGE_GC_VERIFICAR=1`.

Comportamentos da VM 3.6.2 que os programas fixam e que não são óbvios: a VM
compartilha as strings entre isolados (até as montadas em tempo de execução
voltam `identical` de uma ida e volta por `SendPort`; aqui só os literais,
que são estáticos — o 93 confere só esses); `int.hashCode` não é o valor
(`7.hashCode` é 81207), ao contrário do que §2.10 da especificação supunha;
`'ß'.toUpperCase()` fica `'ß'`; e o `utf8.decode` tira só o BOM do começo.

### 12.2 Medidas (integração, 2026-09-30)

Método de §9.10: execuções alternadas dos três executáveis (`aot --optimize`
do `main` antes do espaço unificado, `6f6d14bf`, compilado à parte; o de
depois; e `dart compile exe` do SDK 3.6.2), 5 rodadas de cada `bench/desempenho`
(6 medidas por rodada), mínimo em ms; razão = mínimo / mínimo do Dart AOT.
Máquina Windows 11 de 2 núcleos com ruído, sem builds rodando. As saídas dos
benchmarks são iguais às da VM nos três. A estimativa é a de
`NATIVO-ESPACO-UNIFICADO.md` §5.4 (não medida).

| Medida | Antes | Depois | Dart AOT | Depois/Dart | Estimativa |
|---|---:|---:|---:|---:|---|
| `json` `decode_pequeno` | 78,8 | 63,9 | 20,1 | 3,2× | — |
| `json` `decode_medio` | 116,3 | 89,2 | 27,9 | 3,2× | 2–3× |
| `json` `decode_grande` | 297,3 | 172,0 | 77,8 | 2,2× | 1,5–2,5× |
| `json` `encode_medio` | 96,7 | 49,8 | 30,2 | 1,65× | 1,5–2× |
| `json` `encode_grande` | 211,2 | 106,5 | 71,1 | 1,5× | — |
| `json` `utf8_bytes` | 207,4 | 101,2 | 31,0 | 3,3× | — |
| `json` `reviver` | 88,4 | 58,8 | 20,3 | 2,9× | — |
| `textos/construir` | 108,5 | 70,0 | 49,1 | 1,43× | 1,0–1,3× |
| `textos/hashes` | 45,6 | 33,9 | 27,7 | 1,22× | ≈ 1,2× |
| `int.toString` × 1 milhão | 88,8 | 37,4 | 11,5 | 3,3× | 40–60 ms |
| `codeUnitAt` de campo, 1,5 milhão | 1,3 | 1,0 | 0,5 | 2,1× | ≈ 3,5 ms (outra máquina) |
| `colecoes/lista_add` | 20,5 | 6,7 | 8,3 | 0,81× | — |
| `colecoes/mapa` (`Map<int,int>`) | 87,5 | 113,3 | 41,7 | 2,7× | — |
| `colecoes/conjunto_str` | 45,6 | 35,1 | 16,2 | 2,2× | — |
| `objetos_em_colecoes/lista_objetos` | 41,8 | 21,0 | 11,5 | 1,8× | — |
| `objetos_em_colecoes/mapa_int_objeto` | 57,3 | 62,9 | 19,3 | 3,3× | — |
| `objetos_em_colecoes/mapa_str_objeto` | 33,4 | 21,6 | 8,5 | 2,6× | — |
| `objetos_em_colecoes/ordenar_objetos` | 196,8 | 95,4 | 45,6 | 2,1× | — |
| `objetos_escapam/arvores` | 47,9 | 47,1 | 26,4 | 1,8× | — |
| `chamadas/closures` | 31,0 | 24,9 | 16,7 | 1,5× | — |
| `blend` escalar / SIMD 3.6 / SIMD 3.14 (µs) | 34,7 / 20,3 / 20,9 | 33,2 / 19,4 / 20,0 | 58,0 / 6 722 / 11 661 | — | — |
| servidor HTTP, CPU/req, 1 conexão (µs, mediana de 3) | 150,7 | 139,1 | 75,3 | 1,85× | 140–155 µs |
| servidor HTTP, req/s, 64 conexões | 6 206 | 6 731 | 9 104 | — | — |
| servidor, RSS em repouso / pico (MB) | 16,2 / 23,2 | 19,3 / 28,1 | 17,0 / 27,7 | — | — |
| `hello` (bytes) | 10 786 816 | 10 711 552 | 5 797 376 | 1,85× | — |
| `bench/http/servidor.dart` (bytes) | 12 213 760 | 12 075 520 | 6 108 160 | 1,98× | — |
| string `"abc"` em memória | ≈ 90 B | 32 B (layout) | 32 B | 1× | 32 B |

Sem mudança (dentro do ruído): `crivo`, `lista_leitura`, `lista_sort`,
`objetos_temporarios`, `numerico`, `fib`, `formas`, `tipados`,
`lista_ligada`. O laço SIMD do `blend` continua sem chamada nas voltas rápidas
(o assembly de `blendSimd314`/`blendSimd36`/`blendEscalar` é o de
`SIMD-NATIVO.md` §8.4).

Os dois pioras são os mapas de chave `int` (`mapa`, `mapa_int_objeto`): o
`int.hashCode` passou a ser o da VM (`HashIntegerOp`; o 95 o fixa), e as
chaves sequenciais deixaram de cair em posições vizinhas do `_index` —
acesso aleatório, que o Dart AOT paga também, mas com menos trabalho por
sonda. O perfil do `Map<int,int>` depois: 38% em `nativos_hash::sondar` (as
faltas de cache da sonda), ~20% nos acessos do runtime ao `_index` e ao
`_data` (`tipada`, `lista_elementos`, `palavras`) e o resto no `_Map` em Dart.

Números da primeira medição da integração, antes da correção de
`bloco_vivo` (§4.10 item 63 da especificação), para registro: `mapa` 254,8,
`conjunto_str` 99,7, `textos/hashes` 85,1, `construir` 120,3 ms — o runtime
passava 65% do tempo conferindo handles no mapa de páginas.

**Gatilhos (P0b passo 10).** `LIMITE_JOVEM` remedido com
`DARTFORGE_GC_JOVEM_KB` em 1, 2, 4 e 8 MiB (3 rodadas alternadas de
`objetos_escapam`, `objetos_temporarios`, `textos`, `json`,
`objetos_em_colecoes`): nenhum valor ganha em todos. 4 e 8 MiB melhoram
`arvores` e `mapa_int_objeto` (5–10%) e pioram o JSON pequeno e médio
(`decode_*`, `encode_*`, 5–20%); 1 MiB piora `arvores` 15%. Fica em 2 MiB.
`CONTAGEM_JOVEM` (256 Ki alocações) não dispara mais antes de
`LIMITE_JOVEM`: todo bloco tem ao menos 24 bytes, e 256 Ki × 24 B = 6 MiB >
2 MiB. Fica como teto de segurança.
