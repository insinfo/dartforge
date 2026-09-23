# Pesquisa aplicada — LLVM como backend do Dart AOT: dois experimentos e o que eles dizem ao nosso

Dois grupos já tentaram pôr o LLVM atrás de Dart compilado antes de nós, e
os dois deixaram código e números:

* **Dartino / Erik Corry e Dmitry Olshansky (Google, 2016–2017)** — o
  programa inteiro dos bytecodes do Dartino traduzido num **módulo LLVM só**,
  com GC preciso e **móvel** por statepoints e exceções pelo EH do próprio
  LLVM. Notas em `references/NOTAS-ARTIGOS.md` §1; código em
  `references/dartino-llvm` e o LLVM corrigido em `references/llvm-dartino`.
* **Zuojian Lin (linzj, Alibaba, 2019–2026)** — o IL da VM oficial traduzido
  para LLVM IR **função por função**, dentro do pipeline AOT da VM, avaliado
  em 2020-08 por Vyacheslav Egorov (mraleph, equipe da VM). Notas em
  `references/linzj-llvm-project/NOTAS.md`; os 122 commits do LLVM corrigido em
  `COMMITS.md`; o código do tradutor, com os patches de paridade de mraleph, em
  `references/dart-sdk-llvm-mraleph` (branch `with-llvm`) e
  `references/dart-sdk-llvm10-mraleph` (branch `with-llvm10`); o embrião do
  método em `references/llvm-toy`.

A tese deste documento, que vale repetir antes de qualquer decisão de backend:

> O LLVM dá de graça o que é **local** (quadros, registradores, laços,
> seleção de instruções). Ele não dá — e às vezes atrapalha — o que é
> **contrato com o runtime**: raízes do GC, desenrolamento de exceção,
> convenções de slow path. Os dois experimentos gastaram a maior parte do
> esforço exatamente nessa fronteira.

Convenção: citações `arquivo:linha` sem prefixo são relativas a
`references/dart-sdk-llvm-mraleph/runtime/vm/compiler/` (o tradutor do linzj) ou
a `references/dartino-llvm/src/vm/` (o Dartino), conforme a seção. **"Verificado"**
quer dizer lido no código; **"não verificado"** quer dizer afirmado pelo material
(blog, avaliação, thread) e não conferido.

---

## 1. Como o tradutor do linzj funciona

### 1.1 Onde entra no pipeline (verificado)

O tradutor é **um passe a mais** do pipeline de otimização da VM, rodando
imediatamente **antes da alocação de registradores**:

* `compiler_pass.cc:376-381` — `INVOKE_PASS(IRTranslate)` sob
  `FLAG_llvm_compiler`, seguido de `AllocateRegisters` e `ReorderBlocks`.
* `compiler_pass.cc:606-615` — o passe só traduz a função se
  `graph_entry()->unchecked_entry() == nullptr`, isto é, **funções com ponto de
  entrada sem checagens ficam fora do LLVM** e vão pelo backend da VM.
* Tudo o que é front/middle-end — kernel, inlining da VM, especialização por
  tipo, eliminação de checagens de limite, alocação de registradores das
  funções que o LLVM recusou — continua sendo da VM. O LLVM recebe o IL já
  otimizado, em SSA, e faz só o backend.

A consequência é que o programa final é **misto**: parte das funções vem do
LLVM, parte do linear scan da VM. O tradutor desiste da função em vários
casos, marcando `set_exception_occured()` — representação de parâmetro que ele
não sabe (`backend/llvm/ir_translator.cc:2719-2724`), `CheckClass`
(`:4461-4463`), `CheckSmi` (`:4470-4473`), `SimdOp` (`:5321-5326`), `double` em
parâmetro de pilha (`:1999`), valor não `Tagged` no ambiente de um `catch`
(`:2136-2139`) — e há 39 instruções do IL com `UNREACHABLE()` como "unsupported
IR". Como a função recusada vai para o caminho normal é decidido fora da parte
baixada do repositório (`precompiler.cc`/`flow_graph_compiler.cc` não estão no
checkout esparso) — **não verificado**; o que está verificado é que
`FlowGraph::llvm_compile_ready()` (`backend/flow_graph.h:461`) só é verdadeiro
depois de `AnonImpl::End()` (`ir_translator.cc:766-776`).

### 1.2 Uma função vira um módulo LLVM (verificado)

* `ir_translator.cc:2680-2733` — o construtor monta a assinatura: os
  registradores fixos da VM (`THR`, `PP`, `ARGS_DESC_REG`, `NULL_REG`,
  `BARRIER_MASK`, `DISPATCH_TABLE_REG`) viram **parâmetros** de uma convenção
  de chamada própria, `LLVMV8CallConv` (`backend/llvm/output.cc:104-160`), com 15
  parâmetros em registrador no ARM64 (`backend/llvm/target_specific.h:56`).
* `ir_translator.cc:2737-2747` — `Translate()`: liveness própria
  (`liveness_analysis.cc`), visita dos blocos, e `End()`.
* `ir_translator.cc:766-776` — `End()` fecha os `phi`s, emite debug info e o
  mapa de chamadas e chama `Compile`.
* `backend/llvm/compile.cc:186-253` — `Compile` usa **MCJIT** em memória, com
  `OptLevel = 1` e `SizeLevel = 2` (`:189`, `:213-214`), `CodeModelTiny` no
  ARM64 (`:191`) e `NoFramePointerElim` (`:193`). O gerenciador de memória
  captura as seções `.llvm_stackmaps`, `gcc_except_table`/`ARM.extab` e
  `.debug_line` (`:128-135`).
* `backend/llvm/output.cc:44` — `LLVMSetGC(function, "coreclr")`: a estratégia
  de GC do LLVM que habilita statepoints, com um `// FIXME: Add V8 to LLVM.`

### 1.3 O LLVM gera bytes; a VM reescreve as chamadas (verificado)

É a parte mais importante e menos óbvia do desenho. Toda chamada — a outra
função Dart, a stub, ao runtime, à barreira de escrita, a C — é emitida como um
`llvm.experimental.gc.statepoint` com um **id de patchpoint** e um **tamanho de
instrução reservado** (`ir_translator.cc:2078-2164`, operandos em `:2080-2103`).
O LLVM reserva ali um buraco de `instr_size` bytes e registra o id no stack map.

Depois, `backend/llvm/llvm_code_assembler.cc:41-66` (`CodeAssembler::AssembleCode`)
**copia os bytes do LLVM para o assembler da VM** e, em cada deslocamento de
patchpoint, executa uma "ação" que emite a sequência de chamada **da VM**
(`:163-269`): chamada relativa a outra função (`kCallRelative`), a stub
(`kStubRelative`), por registrador, por offset do `Thread`, nativa, patchável
(IC) ou C (`kCCall`). Os tipos estão em `backend/llvm/stack_map_info.h:46-60`.
`WrapAction` confere que a ação emitiu exatamente os bytes reservados (`:271-282`).

Ou seja: o LLVM faz registradores, seleção de instruções e quadro; **a VM
continua dona de toda a ABI externa** — descritores de PC, tabela de exceções,
stack maps no formato dela, pool de objetos, relocação de chamadas. É o que
permite misturar funções dos dois backends no mesmo snapshot.

### 1.4 Os dois antecessores

* `references/llvm-toy` — MCJIT + `llvm.experimental.patchpoint` com
  `anyregcc` (`llvm/Output.cpp:118-122`) + parser de stack maps
  (`llvm/StackMaps.cpp`). É o embrião do método: gerar com o LLVM, remendar os
  buracos depois. (Os nomes `Output`, `CommonValues`, `Abbreviations` são os do
  FTL do WebKit; a origem é **não verificada**.)
* `references/dart-sdk-llvm-mraleph/runtime/llvm_codegen` — protótipo **da
  própria equipe do Dart** (cabeçalho "the Dart project authors", 2019): lê o
  flow graph serializado como S-expressão (`test/codegen/Inputs/hello.sexp`) e
  gera LLVM IR fora do processo (`test/codegen/Inputs/hello.ll.expected`). É a
  rota da flag `--serialize_flow_graphs_to` que mraleph cita — e, no SDK
  oficial de referência, a flag **não existe mais** (verificado por busca em
  `references/dart-sdk/runtime/vm`).

---

## 2. GC: statepoints, o que quebrou, e o que isso diz das nossas raízes explícitas

### 2.1 Como eles geram e leem os stack maps (verificado)

**linzj.**

1. `liveness_analysis.cc:80-100` calcula, para cada `InstanceCall`,
   `StaticCall` e `ClosureCall`, o conjunto de SSA vivos depois da chamada. O
   `PP` é tratado como sempre vivo (`:51-52`). Um `MaterializeObject` no
   ambiente de desotimização **quebra a análise** (`:219-223`) e a função é
   recusada.
2. `ir_translator.cc:2105-2115` põe todo SSA vivo **de tipo `tagged`** como
   operando gc-live do statepoint; `:2195-2207` emite um `gc.relocate` por
   operando e troca o valor SSA pelo relocado, e pega o resultado com
   `gc.result`.
3. O LLVM (`RewriteStatepointsForGC` e o `StackMaps` do backend) produz a seção
   `.llvm_stackmaps`.
4. `llvm_code_assembler.cc:163-174` lê a seção, tira o tamanho do quadro (menos
   2 palavras de FP e LR) e o usa como `spill_slot_count` da VM;
   `:411-445` (`RecordSafePoint`) converte cada local `Indirect` (relativo a SP
   ou FP) em um bit do **bitmap de stack map da VM**, e acrescenta os
   parâmetros de pilha marcados como `tagged` em `CallSiteInfo::parameter_bits`
   (`ir_translator.cc:1978-2007`).
5. A VM varre a pilha com os mapas dela, como sempre. O LLVM nunca sabe como
   o GC anda na pilha — só produz a informação.

**Dartino.**

1. `codegen_llvm.cc:315-338` registra uma `GCStrategy` própria (`DartinoGC`)
   com `UseStatepoints = true` e declara que **todo ponteiro em
   `addrspace(1)` é do heap** e só esses.
2. O pipeline tem três etapas (`codegen_llvm.cc:3543-3551`):
   `OptimizeModule` (inlining, `mem2reg`, LICM, EarlyCSE, GVN, ADCE —
   `:3748-3779`) **com os ponteiros ainda opacos** no espaço 1;
   `LowerIntrinsics` (`:3781-3793`) — ids de statepoint por chamada,
   `PlaceSafepoints`, rebaixamento dos intrínsecos próprios (leitura de tag,
   barreira de escrita por *card marking* em `:3643-3676`) e
   `RewriteStatepointsForGC`; e `OptimizeAfterLowering` (`:3795-3817`), mais
   leve, porque depois dos `gc.relocate` o otimizador enxerga pouco.
3. `gc_llvm.cc:23-112` anda na pilha pelo endereço de retorno (um mapa
   endereço → registro de stack map, montado em `:114-156`), trata os pares
   **base/derivado** e, como o GC **move** objetos, reescreve o derivado a
   partir da base movida (`:75-96`).

### 2.2 O que quebrou no fork do LLVM do linzj (classificação dos 122 commits)

Classificação feita **pelo título** de cada commit em `COMMITS.md` (um commit,
uma família; lida sem abrir o diff — é **não verificado** no conteúdo):

| família | commits | exemplos |
| --- | ---: | --- |
| **stack map / statepoint / relocação / "gc bug"** | **42** | `bad26cc6d9` (2026-08) "stack maps marking uninitialized slots live, crashing GC"; `e2ba01eea6` (2023) "Incorrect stack maps, missing one stack slot"; `85b3762359`→`62ed6a9ad7`→`63f0e11007` "Fix the gc problem" / revert / de novo; `d1ba837104`→`caf685daa4` "propagateToListeners produces wrong stack maps" / revert |
| **CSR, *regmask*, coalescing e alocação de registradores em volta dos statepoints** | **19** | `6293e9d4d5` (2026-09) "Keep statepoint-observed and raw registers apart"; `240df7e776` (2026-06) "statepoint CSR interference out of band"; `ee7b0e2f5a` "Redesign custom regmask"; `f4fe7067b2`→`d291b31b08` ignora interferência / revert |
| quadro: elisão, *shrink wrap*, FP/LR, prólogo | 12 | `ef40519359` "V8 frame elide algorithm to replace ShrinkWrap"; `ea508c9b3a` "Force hasFP if dart/v8 has calls"; `18a1bb0bd4` "lr spill but fp not spill" |
| codegen AArch64/ARM (correção e qualidade, fora do GC) | 11 | `5667299d6c` mul sext; `fa0db9381f` zext; `9381f70ade` x em w |
| convenção de chamada, argumentos, chamada de cauda, iOS | 9 | `9502deb1fc` X8/X9 como argumento; `49d12e37aa` tailcall destrói o LR gravado; `48a94edeb4` ABI do iOS |
| globais da VM | 5 | `b5a2bd9b70`→`abbef7e168`→`f7c8b44cb7` endereçamento de global / revert / de novo; `ab24cc9340` CSR poluído ao carregar global |
| divisão de módulo (`SplitModule`) | 4 | `c1c098aa06`, `fd9fc21933`, `fe52bf93d3` (revert), `9a65763919` |
| exceções | 2 | `88ab975228` "split token exception block"; `25db4df804` "Opt for exception table" |
| `async` (funções suspensas) | 1 | `0cb82915a5` (2023-03) "Support dart suspendable function" |
| infraestrutura (build, sincronização, asserts, limpeza) | 17 | `219ebcc8fd` o patch inicial; `1b97d424c3` "Sync the dart upstream" |
| **total** | **122** | 9 deles são `Revert` |

A família de GC **e** a de registradores em volta dela somam **61 de 122**
(50%). A NOTAS.md fala em "~32"; a diferença é de critério — aqui entram também
os commits de `RewriteStatepointsForGC`, `gc.relocate`, `StatepointSimplify` e
os "Fix gc bug" sem mais detalhe.

O que a tabela diz, mais do que o número:

* **Não converge.** O primeiro commit da família é de 2020-09; os três mais
  recentes do fork inteiro são dela: 2026-06, 2026-08 ("crashing GC"),
  2026-09. Seis anos de fork — e de uso interno na Alibaba, afirmado e
  **não verificado** — e o GC ainda dá crash por stack map errado.
* **O defeito é de interação, não de um passe.** Os títulos passam por
  `RegisterCoalescer`, `LiveIntervals`, `MachineCopyPropagation`, `taildup`,
  rematerialização, `SelectionDAG` — todo passe que move um valor entre
  registrador e memória precisa saber que ele é "observado pelo statepoint".
  Cada otimização nova do LLVM upstream é um lugar novo para errar.
* **Três ciclos de corrige/reverte/corrige** na mesma família: o sintoma
  aparece longe da causa (um objeto não relocado só dá problema na próxima
  coleta).
* **A convenção de registradores custou caro.** Egorov registra que o
  statepoint "derrama tudo no chamador" (NOTAS.md); linzj criou uma convenção
  que derruba quase todos os registradores e um statepoint "só registro" — e
  metade da família de registradores é consertar essa convenção.

### 2.3 O que isso diz da nossa escolha

O nosso desenho (`docs/NATIVO-PLANO.md` §6.5, na branch `ci/nativo-contrato`;
em `main` o emissor **declara** `dartforge_gc_push_frame`/`set_root`/`pop_frame`
— `crates/emit_native/src/llvm/mod.rs:133-136` — mas ainda não os chama) é
**raízes explícitas**: um quadro de raízes por função, um slot fixo por valor
SSA `Ref` e por `alloca` `Ref`, `set_root` depois de cada definição, e o
runtime varre os quadros (`crates/runtime/src/heap.rs:316-372`).

| eixo | statepoints (linzj, Dartino) | raízes explícitas (DartForge) |
| --- | --- | --- |
| custo no caminho feliz | ~zero instruções extras; o custo é o LLVM derramar os ponteiros vivos antes de cada chamada | uma escrita por definição de `Ref` e um `push`/`pop` por função com `Ref` — hoje como **chamada externa opaca** |
| o que o LLVM pode otimizar | tudo antes do rebaixamento (Dartino); quase nada depois (`gc.relocate` é opaco) | tudo, exceto o que atravessa as chamadas de raiz — que, opacas, são barreiras para alias e para elisão de quadro |
| onde mora a correção | em **todo** passe do backend do LLVM que toca registradores (§2.2) | no **nosso** emissor e no **nosso** runtime; um defeito é "faltou `set_root`", localizável com `DARTFORGE_GC_STRESS=1` (G7) |
| GC móvel | possível (Dartino: `gc_llvm.cc:75-96`), mas é a parte mais difícil — base/derivado, relocação | o heap é por *handles* (`heap.rs:83-115`): um objeto pode mudar de lugar sem reescrever a pilha, porque a pilha guarda o índice |
| plataforma | Dartino: só x64 tinha os intrínsecos; linzj: ARM/ARM64 com LLVM corrigido | qualquer alvo do Clang, LLVM **sem patch**; Windows/MSVC incluído |
| manutenção | um fork do LLVM (122 commits, ainda crescendo em 2026) | nenhum fork |

**Conclusão.** O risco dos statepoints é de **correção**, e ele é de outra
pessoa (o LLVM upstream muda, o fork corre atrás). O risco das raízes
explícitas é de **desempenho**, e ele é nosso e mensurável. A escolha fica — e
o trabalho que ela cria é baratear o caminho feliz, não trocá-la:

1. **Raiz como `store`, não como chamada.** O quadro de raízes pode ser um
   `alloca` de `N` palavras na própria função, encadeado numa lista do isolate
   (o *shadow stack* de Henderson, "Accurate Garbage Collection in an
   Uncooperative Environment", ISMM 2002 — a estratégia `shadow-stack` do LLVM
   faz exatamente isso). `set_root` vira um `store i64` no slot; o LLVM não pode
   apagá-lo (o `alloca` escapa para o runtime) mas também não tem de supor que
   ele escreve em tudo. **Proposta, a medir** contra a chamada atual.
2. **Raiz só onde pode haver coleta** (a lição da VM, abaixo).
3. **Medir** o custo com o mesmo programa em três formas — sem raízes
   (`DARTFORGE_GC_OFF=1`, só para medir), raízes por chamada, raízes por
   `store` — antes de afirmar qualquer número.

### 2.4 A lição da chamada LEAF

Egorov, sobre a barreira de escrita: a entrada de runtime chamada no slow path
é **LEAF** — não é safepoint, não pode disparar GC, então **não precisa de
stack map**. Na VM oficial ela é declarada assim:
`references/dart-sdk/runtime/vm/runtime_entry.cc:778`
(`DEFINE_LEAF_RUNTIME_ENTRY(EnsureRememberedAndMarkingDeferred, …)`, verificado).

O tradutor do linzj não aproveitava isso: a barreira ia por `CallResolver` →
statepoint (`ir_translator.cc:1592-1601`), com stack map e derramamento, embora
`GenerateRuntimeCall` exija `!runtime_entry.is_leaf()` (`:1080`). A única
chamada sem stack map era a C de `InvokeMathCFunction` (`kCCall`, sem
`AddMetaData` em `llvm_code_assembler.cc:258-262`).

O Dartino fez o certo: `ScanForGC` (`codegen_llvm.cc:948`) decide por função se
ela pode alocar; chamada que não pode vira `call` com o atributo
`gc-leaf-function` (`:1253`, `:1278-1281`), e `AddStatepointIDsToCallSites` a
pula (`:3578`, `:3585`) — sem statepoint, sem derramamento. E usa o mesmo fato
para as exceções: "throwing causes allocation", então chamada que não aloca
também não lança e **não precisa de `invoke`** (`:1234-1236`).

**Para nós**, a regra é a mesma com outra forma:

* **Extern que não aloca não é ponto de coleta.** Um valor `Ref` cuja vida
  inteira não cruza nenhum ponto de coleta **não precisa de slot**. Uma função
  cujo corpo não alcança nenhum ponto de coleta **não precisa de
  `push_frame`/`pop_frame`** — e aí, sem nenhuma chamada opaca, o Clang pode
  elidir o quadro dela (§6).
* **Extern que não lança não precisa de `exception_pending()` depois** — é a
  primeira das duas saídas de `docs/NATIVO-PLANO.md` §1.3.
* As duas coisas pedem **a mesma tabela**: para cada extern do runtime,
  `{aloca, lança, chama código Dart}`. Ela tem de ser verificável — um teste
  que roda cada extern marcado "não aloca" com o GC em modo estresse e exige
  zero coletas — porque uma marca errada é exatamente o defeito que os
  statepoints tinham, só que do nosso lado.
* G5 (`NATIVO-PLANO.md` §6.5) já depende do mesmo fato: "entre o `pop_frame` do
  chamado e o `set_root` do resultado no chamador não há alocação".

---

## 3. Exceções e `async`

### 3.1 linzj: `invoke` sobre o statepoint, tabela da VM (verificado)

* Uma chamada dentro de `try` que pode lançar vira `invoke` do statepoint, com
  bloco de continuação e *landing pad* (`ir_translator.cc:471`,
  `:2151-2161`).
* Os valores que o `catch` vai ler (as variáveis do ambiente) entram como
  operandos gc-live **do próprio statepoint** (`:2116-2145`) e são relocados
  **dentro do landing pad** para os `phi`s do bloco de `catch`
  (`:2219-2254`). É isso que substitui as *catch entry moves* da VM: o
  mapeamento é aberto e fechado vazio (`llvm_code_assembler.cc:290-294`),
  porque os valores já chegam no `catch` pelo SSA do LLVM.
* A tabela de exceções da VM é **reconstruída a partir da LSDA do LLVM**:
  `backend/llvm/exception_table_parser.cc:13-51` lê a tabela de *call sites*
  (só *cleanup*, `:41`), e `CollectExceptionInfo`
  (`llvm_code_assembler.cc:357-409`) casa cada chamada com o landing pad,
  criando um "try index estendido" quando o LLVM duplicou landing pads
  (`:389-404`).
* **O `throw` continua da VM**: `VisitThrow` chama a entrada de runtime
  `Throw` e emite `llvm.trap` depois (`ir_translator.cc:2952-2960`). O
  desenrolador é o da VM; o landing pad do LLVM é usado só como **endereço**
  de handler. Não há `_Unwind_RaiseException` nem personalidade C++.
* Funções `async`: em 2020, o tradutor registra os `yield_index` dos `Return`
  (`llvm_code_assembler.cc:115-122`) — as funções assíncronas eram então
  closures reentrantes geradas pelo kernel. Em 2023 a VM passou a funções
  suspensas, e o fork ganhou "Support dart suspendable function"
  (`0cb82915a5`). Como isso foi feito é **não verificado** (só o título do
  commit do LLVM está disponível).

### 3.2 Dartino: o EH do LLVM de verdade (verificado)

* `llvm_eh.cc:58-71` — `ThrowException` guarda a exceção no processo e chama
  `_Unwind_RaiseException`; `:193-264` é uma **personalidade própria**
  (`DartPersonality`) que lê a LSDA e instala o landing pad.
* `codegen_llvm.cc:1263-1274` — o bloco de `catch` é um `landingpad` que lê a
  exceção corrente do processo.
* Funciona porque o alvo era x64 com o desenrolador `_Unwind_*` do Itanium
  ABI e um runtime em C++ que lança por ele.

### 3.3 O nosso modelo, comparado

| | linzj | Dartino | DartForge |
| --- | --- | --- | --- |
| mecanismo | `invoke` + landing pad do LLVM, desenrolamento **da VM** | `invoke` + landing pad + `_Unwind_RaiseException` + personalidade própria | exceção **pendente** no isolate + verificação depois de cada chamada que pode lançar (`NATIVO-PLANO.md` §1) |
| caminho feliz | zero instruções | zero instruções | `call` + `icmp` + `br` por chamada |
| valores no `catch` | relocados pelo statepoint no landing pad | variáveis no quadro | SSA/`alloca` normais — o `catch` é um bloco comum |
| exceção vinda do runtime | a VM desenrola os próprios quadros | o runtime C++ lança pelo desenrolador | a extern marca a pendência e retorna; Rust nunca desenrola por `extern "C"` |
| `await` | máquina da VM (funções suspensas) | não havia | a exceção é um valor que atravessa a máquina de estados sem caso especial |
| Windows | não era alvo | não era alvo | alvo principal — SEH/funclets seriam um segundo lowering |

Nenhum dos dois contradiz a nossa escolha; os dois a reforçam por outro
caminho: **o linzj não usou o desenrolamento do LLVM** — usou o landing pad só
como endereço e manteve o desenrolador da VM, que é dono dos quadros. É o
mesmo princípio nosso ("o runtime é dono da exceção"), com um custo de
caminho feliz diferente. O Dartino usou o EH completo e pôde, porque não tinha
MSVC, nem runtime em Rust, nem `async` na pilha.

O que tirar deles para o nosso custo de caminho feliz é a regra do Dartino
(`codegen_llvm.cc:1234-1236`): **se não aloca, não lança** — no nosso runtime,
lançar aloca o objeto de erro e o rastro. Toda chamada a uma extern marcada
"não aloca" dispensa a verificação de pendência (§2.4).

---

## 4. Semântica: o atalho que eles tomaram e que nós não podemos tomar

### 4.1 O que foi descartado (verificado)

```cpp
// ir_translator.cc:3007-3012 (with-llvm) e 3008-3012 (with-llvm10)
// FIXME: implement assertions.
void IRTranslator::VisitAssertAssignable(AssertAssignableInstr* instr) {}
void IRTranslator::VisitAssertSubtype(AssertSubtypeInstr* instr) {}
void IRTranslator::VisitAssertBoolean(AssertBooleanInstr* instr) {}
```

`AssertAssignable` é a checagem de tipo em tempo de execução: parâmetro
covariante, `as` implícito de `dynamic`, atribuição a campo genérico.
Descartá-la transforma um `TypeError` num **crash ou corrupção de memória** —
Egorov chamou de "violação da semântica do Dart". linzj confirmou que, para
medir desempenho, "desativou todas as checagens" **nos dois lados**.

O Dartino tomou atalhos da mesma natureza, declarados no blog e visíveis no
código:

* `noSuchMethod` sem a semântica: com `assume_no_nsm`, chamada a seletor de
  implementação única vira chamada direta sem checar a classe
  (`codegen_llvm.cc:1685-1691`); sem a flag, a falha "lança o Smi 42"
  (`:1711`, comentário "not quite up to the spec … near enough for a
  performance evaluation").
* **Sem verificação de estouro de pilha**: `kStackOverflowCheck` → "Do
  nothing" (`codegen_llvm.cc:2510-2513`). O blog estima ~10% para ter.
  (O linzj **manteve** a sua: `ir_translator.cc:4354-4404`.)
* Inteiros sem precisão arbitrária (o Dart 1 tinha `bigint` implícito).

### 4.2 Por que é inaceitável para nós

A regra de projeto está em `PLANO.md` ("Regra de projeto — equivalência
semântica com o Dart oficial"): o DartForge pode ser mais rápido, **não pode**
fazer um programa semanticamente diferente do mesmo código compilado pelo Dart
oficial. Um `TypeError` que vira crash, um `noSuchMethod` que vira outro erro,
uma recursão infinita que vira `SIGSEGV` em vez de `StackOverflowError` — os
três são diferenças observáveis, e o corpus diferencial as compara byte a
byte com a VM.

Também não é opção "desligar em produção": o `dart compile exe` oficial não
deixa o usuário desligar checagens de tipo (só `assert`, que é da linguagem e
vem desligado por padrão — `--enable-asserts` em
`references/dart-sdk/pkg/dartdev/lib/src/commands/compile.dart:1135`). Um
número nosso sem as checagens seria contra um oficial com elas.

### 4.3 Como medir com honestidade

As duas armadilhas documentadas viram regras:

1. **Mesmas verificações nos dois lados.** Checagem de tipo, estouro de pilha,
   `noSuchMethod`, estouro de inteiro com volta de 64 bits — tudo ligado nos
   dois. `assert` no mesmo estado nos dois (desligado, como o `compile exe`
   por padrão). Se um lado precisar de um atalho para rodar, o experimento é
   de outra coisa e o relatório diz isso.
2. **O mesmo conjunto de funções.** O primeiro número do linzj ("−57% de
   código") estava errado porque o backend LLVM compilava **3× menos funções**
   — Egorov percebeu por "bytes instruction object header" 3× menor (o seletor
   do *dispatch table* não era registrado e o ponto fixo de compilação
   descartava código). E, pelo §1.1, funções com entrada sem checagens nem
   passavam pelo LLVM. Toda comparação de tamanho reporta **quantas funções**
   cada lado emitiu e **quais** faltam de um lado — antes de qualquer
   percentual.
3. **O programa passa no diferencial antes de ser medido.** Programa que
   imprime outra coisa não tem tempo.

---

## 5. Tamanho × velocidade, e um módulo por programa × módulos separados

### 5.1 Os números que existem

| medida | valor | condição | fonte |
| --- | --- | --- | --- |
| Gallery, oficial × LLVM, com todos os patches de paridade | 8.638.892 B × 9.273.600 B (**+7%**; 5.359 símbolos menores, 4.682 maiores) | release, ProductSIMARM64, mesmas checagens | avaliação de Egorov (NOTAS.md) |
| linzj depois | 10,6 MB × 12,8 MB (LLVM maior); −400 KB após corrigir globais; −200–300 KB após corrigir peso de spill no alocador do LLVM (D86680) | não informado | thread (NOTAS.md) |
| linzj, tempo | `libapp.so` de 56% para 46% do tempo ⇒ "−33% de tempo" no código Dart | **checagens desligadas nos dois lados** | thread (NOTAS.md) |
| linzj, primeiro número | "−57% de código" | **errado** (3× menos funções) | avaliação |
| protótipo interno de 2018 | ganho de velocidade, regressão de tamanho, mesmo com `-Os` | tradução ingênua | Egorov |
| Dartino | desempenho comparável ao AOT da VM de 2016-11; inicialização melhor | com os atalhos do §4.1 | blog (NOTAS-ARTIGOS.md §1) |

Nenhum número de velocidade existe **com paridade semântica**. O de tamanho que
existe com paridade diz: o LLVM função por função, com `-Os`, sai **7% maior**
que o backend da VM — porque a VM tem truques de tamanho que o LLVM não
reproduz (§6.2), e porque uma função por módulo tira do LLVM o inlining e o
outlining entre funções.

### 5.2 A sugestão da equipe da VM: o programa inteiro como um módulo

Egorov: gerar LLVM IR do **programa inteiro** e compilar como módulo,
otimizando para tamanho — o LLVM faria inlining melhor (a heurística da VM às
vezes não inlina métodos minúsculos) e **outlining** (sequências repetidas
viram funções). O Dartino **fez isso** (um módulo a partir do heap do
programa, `codegen_llvm.cc:3526-3553`, gravado como um bitcode só em
`:3819-3827`) — e foi assim que a chamada direta por mundo fechado virou
inlining (`:1747-1753`, "Is often inlined by LLVM").

O nosso emissor **já é** um módulo por programa (ESTADO.md §1.5). Então, hoje,
nós estamos no desenho que a equipe da VM recomendou — com duas ressalvas: o
SDK ainda não está no IR (0% do IR é corpo do SDK, ESTADO.md §2.5), e o
Clang a `-O2` num módulo só não paraleliza.

### 5.3 A tensão: o cache de objeto e o hot reload pedem módulos separados

`docs/PESQUISA-HOT-RELOAD.md` §4.2 e o plano R2-3 pedem **um módulo por
biblioteca**, com símbolos por `DeclId` estável, para que o SDK seja compilado
uma vez, o cache acerte por biblioteca e a recarga recompile só a biblioteca
suja e as dependentes; §4.3 põe uma **célula de indireção** no ponto de chamada
do perfil JIT recarregável. Módulo separado e chamada por célula matam
exatamente o inlining entre módulos que o §5.2 quer.

### 5.4 A pergunta de Corry, respondida

Corry deixou em aberto: **como usar conhecimento do programa inteiro e ainda
compilar projetos grandes em paralelo?** A resposta que se tira das duas
experiências e da literatura já citada no repositório
(`docs/PESQUISA-OTIMIZACAO.md` §6, ThinLTO) é **separar a decisão global da
geração de código**:

1. **A decisão global é nossa, sobre resumos, antes do LLVM.** Alcançabilidade
   e classes instanciadas (`crates/mundo`, o RTA no estilo do
   `ResolutionWorldBuilder`), ids de seletor e de classe, devirtualização
   (seletor com uma implementação viva ⇒ chamada direta; poucas classes ⇒
   teste de `class_id` e chamada direta, como o Dartino em
   `codegen_llvm.cc:1716-1753`), e a escolha de quais corpos pequenos
   **copiar** para outras bibliotecas. Isso é barato (resumos, não IR),
   determinístico e não precisa do LLVM. É o que o Dartino fazia dentro do
   LLVM sem precisar — o `OnlyOneMethodMatches` dele é uma consulta ao mundo
   fechado, não uma otimização do LLVM.
2. **A geração é local e paralela.** Cada biblioteca (ou partição) vira um
   módulo; os módulos são compilados em paralelo pelo Clang. O conhecimento
   global chega a cada módulo como **fatos já decididos no IR**: chamada
   direta em vez de despacho, `internal` para o que não sai do módulo,
   atributos (`nounwind`, `memory(none)`, `gc-leaf`-equivalente) nas
   declarações das externs e das funções de outras bibliotecas.
3. **O inlining entre módulos, quando valer, é do ThinLTO**, que existe para
   isso: resumo por módulo, índice global na ligação, importação de funções
   pequenas, *backend* por módulo em paralelo, com cache por módulo
   (`clang -flto=thin`; no `lld-link` o cache é `/lldltocache:`, **não
   verificado** neste repositório). O fork do linzj ganhou `SplitModule`
   (`c1c098aa06`, `fd9fc21933`) — sinal, **não verificado**, de que eles
   também chegaram a módulos grandes e tiveram de parti-los para paralelizar.

### 5.5 A conciliação proposta

| perfil | módulos | chamadas entre bibliotecas | otimização | cache |
| --- | --- | --- | --- | --- |
| **dev / JIT recarregável** | um por biblioteca, símbolos por `DeclId` | pela célula (`PESQUISA-HOT-RELOAD.md` §4.3) | `-O0`/`-O1` por módulo | objeto por biblioteca; o SDK uma vez |
| **dev AOT** (`compile-native`) | um por biblioteca | diretas | `-O2` por módulo | objeto por biblioteca |
| **produção** (`dartforge aot`) | os **mesmos** módulos | diretas, com mundo fechado decidido antes (§5.4) | ThinLTO (`-flto=thin -O2`/`-Os`) | cache do ThinLTO por módulo |
| produção, programa pequeno | um módulo (o de hoje) | diretas | `-O2` ou LTO completo | objeto por programa (o de hoje) |

Três invariantes seguram isso:

* **O IR de uma biblioteca é função só da fonte dela e das decisões do mundo
  fechado que ela consome** — e essas decisões entram na chave de cache. Sem
  isso, uma classe nova em outra biblioteca que invalida uma devirtualização
  não invalida o objeto, e o programa fica errado (a versão nossa do "stack
  map desatualizado").
* **O mesmo IR nos dois perfis**, diferente só no que o perfil manda
  (`PESQUISA-HOT-RELOAD.md` §4.3: célula no JIT, chamada direta no AOT).
* **Medir antes de escolher o padrão**: tamanho e tempo do mesmo programa em
  "um módulo `-O2`", "N módulos `-O2`" e "N módulos ThinLTO". A previsão (não
  verificada) é que ThinLTO fique perto do módulo único em velocidade e bem
  melhor em tempo de compilação paralela; os números da §5.1 dizem que não
  inlinar entre funções custa tamanho.

---

## 6. O que o LLVM dá de graça, e os truques da VM que ele não reproduz

### 6.1 De graça (e que a VM teve de fazer à mão)

Egorov listou o que o LLVM fazia melhor e que virou prova de conceito no
backend oficial (NOTAS.md):

| técnica | ganho no Gallery | estado na VM oficial de referência |
| --- | --- | --- |
| quadros de tamanho fixo (argumentos numa área reservada, sem `push`/`pop`) | ~240 KB (2%) | `FlowGraph::max_argument_slot_count` (`references/dart-sdk/runtime/vm/compiler/backend/flow_graph.h:615-618`) — verificado que existe |
| elisão de quadro em função sem chamadas nem spills | ~720 KB (7,5%) | `FlowGraphAllocator::RemoveFrameIfNotNeeded` (`backend/linearscan.cc:3436` e seguintes, só AOT) — verificado |
| *tail merging* (sequências finais iguais) | ≥ 60 KB | passe `TailMerge` na branch do mraleph (`compiler_pass.cc:604`); **não encontrado** por esse nome no SDK oficial de referência |
| movimentos redundantes do linear scan | ~60 KB | não verificado |
| LDP, `LoadClassId` + `if`, parâmetros nomeados | menores | não verificado |

Para nós tudo isso é **de graça** — o Clang faz a `-O2` sem nada do nosso
lado. Com uma condição que é a mesma dos dois experimentos: **o LLVM só
otimiza o que enxerga**. Uma função com `push_frame`/`pop_frame` e um
`exception_pending()` depois de cada chamada nunca é "sem chamadas", então
nunca tem o quadro elidido; um `set_root` opaco entre duas leituras do mesmo
campo impede o LLVM de fundi-las. Os ganhos de graça dependem de o caminho
feliz não ter chamadas opacas — que é o trabalho do §2.3 e do §2.4.

### 6.2 Truques da VM que o LLVM não reproduz

* **Stubs de slow path com convenção própria.** O stub salva todos os
  registradores, não o chamador: slow path é raro, salvar tudo lá não custa,
  e o ponto de chamada — que se repete milhares de vezes — fica com uma
  instrução. O statepoint faz o contrário (derrama no chamador). O linzj
  contornou marcando as chamadas com `dart-shared-stub-call`
  (`ir_translator.cc:2256-2262`) e escolhendo a variante com ou sem FPU pelos
  registradores vivos (`llvm_code_assembler.cc:215-226`).
  **Para nós:** os slow paths (alocação que precisa coletar, barreira, estouro
  de pilha, lançar) devem ser funções `cold` e `noinline`, fora da linha,
  chamadas de um único bloco frio; e vale **experimentar** `preserve_mostcc`
  nelas (a convenção do LLVM que deixa o chamado salvar quase tudo — o
  análogo do stub da VM; suporte no Windows x64 **não verificado**).
* **Múltiplos pontos de entrada.** A VM tem uma entrada que faz as checagens
  de tipo dos parâmetros e outra que as pula quando o chamador já garante o
  tipo. O linzj nem tentou: funções com essa entrada ficavam fora do LLVM
  (`compiler_pass.cc:608-609`). **Para nós:** duas funções LLVM — `f` (o
  corpo, sem checagens) e `f$checado` (checa os parâmetros covariantes/
  `dynamic` e chama `f`) —, com o emissor chamando `f` direto onde o tipo
  estático garante e `f$checado` no despacho dinâmico/por seletor. O LLVM
  inlina o invólucro onde valer. Mesma semântica, sem checagem repetida.
* **Entradas monomórficas** (~30 KB no Gallery): a chamada de instância da VM
  AOT testa a classe na entrada do alvo. O nosso despacho por seletor na
  vtable (`docs/NATIVO.md` §2) não tem esse mecanismo; o equivalente útil é o
  do Dartino — teste de `class_id` **no ponto de chamada** quando o mundo
  fechado diz que poucas classes implementam o seletor, e chamada direta.
* **Caminho rápido inline de `MathPow`** e outros intrínsecos: a VM os tem
  por reconhecimento de método; para nós é a mesma coisa — reconhecer no
  lowering, não esperar do LLVM.

---

## 7. O que adotar, o que não copiar, o que medir

### 7.1 Adotar

1. **Raízes explícitas**, como decidido, com o custo do caminho feliz atacado
   em ordem: marca `{aloca, lança, chama Dart}` por extern (§2.4) → raiz e
   quadro só onde há ponto de coleta → raiz como `store` num quadro-`alloca`
   (§2.3).
2. **"Não aloca ⇒ não lança"** para dispensar a verificação de pendência
   (Dartino, `codegen_llvm.cc:1234-1236`), com teste que prova a marca.
3. **`invariant.load` na leitura do `class_id` e da entrada da vtable**, como
   o Dartino (`codegen_llvm.cc:1761-1776`, com `dereferenceable`) — o
   despacho sai do laço. O `never.faults` deles era patch do LLVM
   (`references/llvm-dartino/include/llvm/IR/LLVMContext.h:72`,
   `lib/Transforms/Scalar/LICM.cpp:952`); no LLVM atual, `dereferenceable` +
   `noundef` + `!invariant.load` cobrem parte disso — **não verificado** se
   basta para o LICM içar a leitura.
4. **Mundo fechado decidindo chamadas diretas antes do LLVM** (§5.4), com
   teste de `class_id` no ponto de chamada para poucos implementadores.
5. **Dados estáticos no binário** para o que é imutável: descritores de
   classe, tabelas de despacho, constantes. O Dartino emitia os objetos do
   heap como globais constantes do LLVM (`HeapBuilder`,
   `codegen_llvm.cc:340` em diante; raízes em `:2839-2898`) e ganhou
   inicialização. No nosso heap por *handles* isso pede um jeito de um handle
   apontar para memória estática (uma faixa de handles permanentes, ou um bit
   de tag) — **proposta**; como o emissor atual materializa constantes não foi
   verificado para este documento.
6. **Invólucro checado + corpo sem checagem** no lugar de múltiplos pontos de
   entrada (§6.2).
7. **Slow paths `cold`/`noinline`** fora da linha; `preserve_mostcc` como
   experimento medido.
8. **Módulos por biblioteca com ThinLTO em produção** (§5.5), quando o SDK
   entrar no IR.

### 7.2 Não copiar

1. **Statepoints e um fork do LLVM.** 61 dos 122 commits do fork do linzj são
   GC ou registradores em volta do GC, e os mais recentes (2026) ainda são
   crash por stack map. Não há versão do nosso projeto em que manter um fork
   do backend do LLVM seja a melhor alocação de esforço.
2. **O EH do LLVM** (`invoke`/`landingpad`/personalidade): as três razões de
   `NATIVO-PLANO.md` §1.2 continuam de pé; o próprio linzj não usou o
   desenrolador do LLVM.
3. **Qualquer checagem descartada** — `AssertAssignable`/`AssertSubtype`/
   `AssertBoolean`, estouro de pilha, `noSuchMethod` —, nem "para medir".
4. **MCJIT por função com remendo de bytes** (§1.3): faz sentido dentro de uma
   VM que já tem ABI, pool e descritores próprios; nós emitimos IR textual para
   o Clang e ligamos com o linker do sistema, e o contrato com o runtime fica
   no IR, não em bytes remendados.
5. **Um módulo por função.** É o pior dos dois mundos: nem inlining entre
   funções nem compilação incremental útil.

### 7.3 Medir

| o que | como | por quê |
| --- | --- | --- |
| custo das raízes | o mesmo programa sem raízes (`DARTFORGE_GC_OFF=1`, só medição), raiz por chamada, raiz por `store` | decidir §2.3 com número |
| custo da verificação de pendência | com e sem a marca "não lança" nas externs; `exception_pending` como chamada × `load` de global | `NATIVO-PLANO.md` §1.3 pede exatamente isso |
| elisão de quadro | contar funções sem quadro no objeto (`llvm-objdump -d`) antes e depois da regra "sem ponto de coleta ⇒ sem `push_frame`" | é o ganho de 7,5% da VM, e só aparece se o caminho feliz ficar limpo |
| 1 módulo × N módulos × ThinLTO | tamanho do texto, tempo de execução, tempo de compilação com 1/4/8 trabalhadores | §5.5 |
| GC sob estresse | `--gc-stress` (G7) em todo programa aprovado | o nosso equivalente do "stack map errado" é a raiz faltando |

### 7.4 Comparar o nosso AOT com o `dart compile exe`, com honestidade

Metodologia, derivada dos dois erros documentados (checagens desligadas;
conjuntos de funções diferentes):

1. **Mesmo SDK** (3.6.2, `C:/tools/dartsdk-3.6.2`), mesma fonte, mesmas
   bibliotecas. O programa passa no diferencial (`dartforge-diferencial
   --nativo`) **antes** de ser medido.
2. **Mesma semântica ligada**: checagens de tipo sempre (não há como desligar
   no oficial); `assert` no mesmo estado dos dois lados (desligado por padrão
   no `compile exe`); estouro de pilha e `noSuchMethod` reais do nosso lado.
3. **Tamanho por partes, não o arquivo**: o executável oficial carrega o
   runtime da VM e o nosso o runtime Rust. Reportar (a) o arquivo inteiro,
   (b) o código Dart gerado — do oficial com
   `--extra-gen-snapshot-options=--print-instructions-sizes-to=<json>`
   (`compile.dart:595`, `references/dart-sdk/runtime/vm/image_snapshot.cc:493`),
   do nosso pelos símbolos do módulo —, e (c) **o número de funções** de cada
   lado, com a lista das que só um lado tem. Diferença grande de contagem
   invalida o percentual até ser explicada.
4. **Tempo**: pelo menos 10 execuções, mediana e dispersão; início do
   processo separado do regime (um `Stopwatch` dentro do programa para o
   núcleo, o relógio de parede para o processo); a mesma máquina, sem build
   concorrente (`docs/historico/BENCHMARKS.md`: "Não execute benchmarks durante outros
   builds"); resultado por programa e média **geométrica** das razões, nunca um
   percentual agregado único.
5. **Memória**: pico de RSS dos dois; o nosso teto de heap
   (`DARTFORGE_HEAP_MAX_MB`) desligado ou igualado, e registrado.
6. **Declarar o que difere e não é o compilador**: o GC (geracional e móvel
   na VM; tracing por *handles* no nosso), a representação de `int` (Smi com
   tag na VM; `i64` cru no nosso), o runtime (C++ × Rust). Quando um número
   surpreender, perfilar e atribuir a diferença a código gerado ou a runtime
   antes de publicar.
7. **Programas**: os do corpus que passam, mais um conjunto de benchmarks
   clássicos (DeltaBlue, Richards, Havlak, JSON) — **proposta**; não estão no
   repositório. O runner existente (`scripts/conformance-native.ps1`,
   `docs/historico/BENCHMARKS.md` "AOT nativo inicial") já compara com o AOT oficial;
   se ele usa a trilha nova (`emit_native`) é **não verificado**.

---

## 8. Os dois experimentos lado a lado, com o nosso

| eixo | Dartino (2017) | linzj (2020–2026) | DartForge |
| --- | --- | --- | --- |
| entrada | bytecodes do Dartino (closures e opcionais já rebaixados) | IL da VM, otimizado, antes da alocação de registradores | HIR própria da trilha nova |
| unidade | **programa inteiro**, um módulo | **uma função**, um módulo, MCJIT | programa inteiro, um módulo (hoje); por biblioteca (proposto, §5.5) |
| LLVM | sem patch no caminho do `llc` (o `never.faults` era patch) | fork com 122 commits | Clang sem patch |
| GC | statepoints, preciso e **móvel**, `addrspace(1)`, otimiza antes de rebaixar | statepoints, mapas convertidos para o formato da VM | raízes explícitas por quadro; heap por *handles* |
| ponto sem coleta | `gc-leaf-function` por análise (`ScanForGC`) | só a chamada C; a barreira tinha stack map | proposto: marca por extern (§2.4) |
| exceções | EH do LLVM + personalidade própria + `_Unwind_RaiseException` | `invoke` + landing pad, desenrolador da VM | pendente + verificação |
| despacho | `invariant.load`/`never.faults`; mundo fechado ⇒ chamada direta ⇒ inlining | o da VM (tabela de despacho, ICs) | vtable por seletor; mundo fechado em `crates/mundo` |
| dados estáticos | classes, constantes, tabelas como globais do binário | o snapshot da VM | proposto (§7.1, item 5) |
| atalhos semânticos | sem `noSuchMethod`, sem estouro de pilha, `int` de 64 bits | sem `AssertAssignable`/`AssertSubtype`/`AssertBoolean` | **nenhum** (`PLANO.md`) |
| resultado declarado | desempenho comparável ao AOT de 2016, inicialização melhor | +7% de tamanho com paridade; −33% de tempo sem checagens | — (ainda sem SDK no IR) |

O que os dois ensinam juntos: o LLVM **sem** conhecimento do programa inteiro
(linzj) sai maior que um backend feito à mão; o LLVM **com** ele (Dartino)
compete — mas as duas medições foram feitas com atalhos semânticos. O que
ninguém mediu ainda é o LLVM com mundo fechado **e** paridade semântica. É essa
a medição que o DartForge está em posição de fazer, e a metodologia do §7.4 é
para que ela valha.
