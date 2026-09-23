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
