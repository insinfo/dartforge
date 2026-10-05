# Mapas de pilha e exceções por tabelas no nativo: especificação da escolha chaveável

Escrito em 2026-10-02; detalhado em 2026-10-04 com os experimentos da rodada 2 (§12) e o nível de
implementação das Etapas 1 e 2 (§13 a §15). Especificação; nada aqui está implementado no produto.
Onde o §12 em diante diverge das seções anteriores, vale o mais novo, e a divergência está dita no texto.

Atalhos de caminho:

* `EN/` = `crates/emit_native/src/`;
* `RT/` = `crates/runtime/src/`;
* `PERRY/` = `E:\references\perry\`.

Nas citações, "verificado" quer dizer lido no código ou medido num experimento. "Não verificado" quer dizer
afirmado por documento ou por terceiros e não conferido. O §11 junta o que não foi confirmado.

---

## 0. Resumo das decisões

1. **Duas dimensões independentes.** Elas formam quatro combinações:

   | | `--excecoes=checagem` | `--excecoes=tabelas` |
   |---|---|---|
   | `--raizes=sombra` | **A0**, o modelo atual (padrão, oráculo) | **A1**, Etapa 1 |
   | `--raizes=mapas` | **B0** | **B1** |

   Cada dimensão tem o modo atual como padrão. O modo atual nunca sai do código: ele é a referência, o
   fallback por alvo e por função, e o oráculo de comparação (§5).
2. **Raízes por mapas (B).** As referências viram `ptr addrspace(1)` no IR, com a estratégia
   `gc "statepoint-example"` do LLVM sem fork.
   * O `rewrite-statepoints-for-gc` (RS4GC) roda **depois** da otimização, dentro de um backend que nós
     dirigimos (§3.4).
   * A seção `.llvm_stackmaps`/`__LLVM_STACKMAPS` é convertida, na geração do objeto, num mapa compacto
     próprio (`__df_gcmap`, §3.5).
   * O runtime percorre os quadros pelo desenrolador do sistema. Onde for barato e seguro, usa a cadeia de
     frame pointers (§3.6).
   * O coletor não move: todo `gc.relocate` é identidade, e o runtime só **marca**.
3. **A análise de raízes é uma só.** A colocação por vivacidade de `EN/llvm/raizes.rs` e o predicado
   `pode_coletar` (`EN/llvm/mod.rs:2728`) alimentam as duas lowerings:
   * no modo sombra, os slots e os `store`;
   * no modo mapas, o `"gc-leaf-function"` nas chamadas que não coletam e a manutenção dos argumentos vivos.

   É a lição do Perry (`PERRY/docs/src/internals/garbage-collector.md:207`).
4. **Exceções por tabelas (1).** São `invoke`/`landingpad` no formato Itanium **em todos os alvos,
   inclusive windows-msvc**, com uma personalidade própria em Rust.
   * No Linux e no macOS ela é chamada pelo `_Unwind_RaiseException`.
   * No Windows ela é instalada como *language handler* do `.xdata` e transfere o controle com
     `RtlUnwindEx`.
   * Os funclets (`catchswitch`/`catchpad`) ficam de fora: o RS4GC quebra com eles (§9). Para o LLVM, a
     personalidade desconhecida faz o Windows usar uma LSDA Itanium. É o desenho que o Perry usa em produção
     (`PERRY/crates/perry-runtime/src/eh_windows.rs:1-37`).
5. **Exceção Dart nunca atravessa quadro Rust.** Toda entrada Rust→Dart passa por uma **porta** gerada, que
   pega tudo e converte a exceção em pendência. O protocolo de hoje continua valendo na fronteira com o
   runtime, nos dois sentidos (§4.6).
6. **Mistura proibida entre modos numa ligação.** Um marcador de ABI por objeto e por imagem impede, e o
   modo entra na chave dos caches de objeto e do SDK (§5.4).
7. **Ordem das etapas:**
   * Etapa 1: só exceções por tabelas (A1).
   * Etapa 2: protótipo de mapas no **Windows x86-64**, o alvo do dono e o mais difícil.
   * Etapa 3: formato compacto e medida do executável completo.
   * Etapa 4: os demais alvos.

   Cada etapa tem critério de pronto e de abandono (§6).
8. **Nenhum ganho está demonstrado.** As metas de 27 MB e ~2 min do `new_sali/backend`
   (`docs/NATIVO-PRODUCAO-GRANDE.md:12-13`) são **hipóteses a medir**, não resultados esperados. O §8 dá a
   fórmula do saldo, e o único número externo (Perry: +1,86% de binário, −1–2% de tempo) é de outro
   compilador, outro GC e outra linguagem.

---

## 1. Estado atual e o que custa

### 1.1 Raízes: pilha-sombra com slots por vivacidade (A)

* **Slots.** `raizes::atribuir_slots` (`EN/llvm/raizes.rs:61-258`) dá um slot a cada `alloca` `Ref` e a cada
  valor SSA `Ref` vivo na entrada de um ponto de coleta, inclusive os operandos dele. Dois valores que não
  interferem dividem o slot (`raizes.rs:1-24`).
* **Quadro.** O prólogo faz (`EN/llvm/mod.rs:646-699`):
  * cria `%gcq = alloca { ptr, i64, [n x i64] }`;
  * zera o quadro (`:680`);
  * lê o contexto da thread (`%ctx = call @dartforge_contexto()`, `:665`);
  * encadeia o quadro no topo da pilha-sombra (`%ctxtopo`, deslocamento 8; `:685-687`);
  * grava os parâmetros `Ref` nos seus slots.
* **Gravação.** Cada definição enraizada recebe `store i64 %v, ptr %gcsN` logo depois (`:1277-1290`).
* **Volta.** Antes de **todo** `ret`, o anterior volta ao topo: `%gcvolta` (`:1308-1313`, contrato G3,
  `docs/NATIVO-PLANO.md` §6.5).
* **Runtime.** `QuadroDeRaizes { anterior, n, slots }` (`RT/heap.rs:233-238`). A coleta percorre a cadeia em
  `visitar_quadros` (`RT/heap.rs:389-403`) junto com as demais raízes (`Heap::raizes`, `RT/heap.rs:1454-1469`).
  As próprias funções do runtime enraízam por `com_raizes` (`RT/gc_raizes.rs:8-20`), num vetor separado
  (`Heap::frames`).

### 1.2 Exceções: pendência e checagem depois de cada chamada (0)

* **Checagem.** `emit_call_with_check` (`EN/lower/fn_builder.rs:940-992`) põe depois de cada chamada que pode
  lançar uma leitura do espelho `Contexto::pendente` e um desvio:
  * para o alvo de exceção corrente;
  * ou para a entrada do `finally`;
  * ou para um retorno com o valor padrão.

  No IR, a leitura é `load i8, ptr %ctx` (`EN/llvm/mod.rs:1017-1020`).
* **`throw`.** É `dartforge_exception_throw(bits, tag)` seguido de desvio (`fn_builder.rs:994-1046`). O
  runtime guarda o valor e liga o espelho (`RT/excecoes.rs:179-184`, `:229-279`).
* **`try`/`catch`/`finally`** (`EN/lower/comandos.rs:468-861`):
  * o `catch` lê `dartforge_exception_peek_ref`, pergunta `dartforge_exception_capturavel` (o desenrolar de
    `Isolate.exit` não é capturável), testa os `on T` e limpa;
  * o `finally` usa o **discriminador de razão**, um `phi` com 0 = normal, 1 = `return`, 2 = exceção e
    `5 + k` = salto `k` (`fn_builder.rs:10-28`, `comandos.rs:740-812`). A exceção é guardada e sai da
    pendência enquanto o corpo do `finally` roda (`comandos.rs:740-761`).
* **Estouro de pilha.** O prólogo compara um `alloca` com `Contexto::limite_da_pilha`. Abaixo dele, chama
  `dartforge_estouro_de_pilha` e retorna com a pendência (`EN/llvm/mod.rs:2449-2481`, `RT/gc_raizes.rs:108-121`).
  A folga é de 256 KiB (`RT/gc_raizes.rs:39`).
* **Por que não `landingpad`** (`docs/NATIVO-PLANO.md` §1.2): dependência de plataforma, o runtime em Rust
  (`extern "C"` não desenrola) e `async`. O §4 responde às três.

### 1.3 Medidas do repositório

No IR vivo do `new_sali/backend` em produção (`docs/NATIVO-PRODUCAO-GRANDE.md:410-423`):

| padrão | ocorrências |
|---|---:|
| volta do quadro de raízes em cada saída (`%gcvolta`, 2 linhas) | 396 mil saídas (792 mil linhas) |
| gravação de raiz no quadro | 674 mil |
| checagem da exceção pendente depois de chamada | 495 mil |
| funções `$async` | 1 287 (76,5 MB de IR) |

* Na partida, essas duas causas eram estimadas em ~15% (raízes, P5) e ~12% (checagem, P4) das linhas de IR
  vivas (`NATIVO-PRODUCAO-GRANDE.md:90-112`).
* No executável há ~400 mil `cmpb $0,(ctx); jne` (`:162`).
* O executável de produção tem 57,2–63,4 MB, contra 27,3 MB do `dart compile exe`. O `.text` tem 55 MB em
  58 mil funções (`:397-416`).
* A conclusão do próprio documento: "cada passo de forma que não mexe nelas dá 3–5%" (`:423`).
* **Atenção:** linhas de IR não são bytes de `.text`. Ninguém mediu quanto do `.text` essas linhas viram. A
  primeira medida da Etapa 1 e da Etapa 2 é justamente essa (§8.2).

---

## 2. Contratos que qualquer implementação preserva

Cada contrato vale nas quatro combinações e tem pelo menos um teste no §7.

* **C1 — argumentos vivos durante a chamada que coleta.**
  * O runtime conta com o chamador para manter vivos os argumentos durante a chamada. Exemplo:
    `dartforge_string_concat` aloca o resultado e depois lê os dois textos (`EN/llvm/raizes.rs:10-13`).
  * No modo sombra isso sai da regra "enraizado = vivo na **entrada** do ponto de coleta, operandos
    inclusive".
  * No modo mapas **não sai de graça**: o RS4GC só registra o que está vivo **depois** da chamada. Um
    argumento que morre na chamada fica de fora, e a função Rust que o recebeu não tem mapa.
  * O modo mapas precisa então de uma **manutenção explícita** (§3.3, "argumentos").
* **C2 — valores etiquetados.**
  * 0 é null, ímpar é `Smi`, bloco+2 é objeto (`RT/layout.rs:9-13`, `:382-417`).
  * O coletor descarta 0 e ímpar (`smi::e_handle`, `RT/layout.rs:407-409`).
  * Uma raiz que seja par e não seja handle de um bloco vivo (ou estático) é **defeito**. O modo de
    verificação aborta. Nos dois modos, `Heap::handle_invalido` já faz isso na marcação de verificação
    (`RT/heap.rs:1484-1517`).
  * Conversões `int`↔`Ref`: no modo mapas, criar referência a partir de inteiro (`inttoptr` para
    `addrspace(1)`) só é permitido em três casos:
    1. o resultado de uma chamada ao runtime que devolve handle;
    2. `Smi` e null construídos;
    3. o endereço+2 de um objeto estático da imagem.

    Nenhum outro `inttoptr` para `addrspace(1)` sai do emissor. Um verificador do IR confere (§7.4).
* **C3 — referências derivadas e interiores.**
  * O handle já é derivado: bloco+2. O endereço de campo é derivado de novo: `h − 2 + 8k`.
  * Coletor não móvel: basta que a **base** esteja no mapa sempre que um derivado estiver vivo. O RS4GC
    garante isso pela análise de base (Statepoints.rst, "Base & Derived Pointers"). O runtime só marca bases.
  * Nunca guardar só um endereço de campo através de um ponto de coleta sem a base.
* **C4 — raízes do runtime.** Tabelas e caches continuam raízes explícitas do `Heap` nos dois modos:
  * globais, literais, enums, tear-offs, `raizes_do_runtime[0..1]` (exceção pendente e rastro), anexos de
    finalizador (`RT/heap.rs:1454-1469`);
  * áreas de globais por isolado (`RT/gc_raizes.rs:155-180`);
  * frames de `com_raizes`.

  O modo mapas troca **só** a fonte "quadros do código gerado". Um cache novo do runtime com ponteiro de
  GC e sem raiz registrada é o formato 5 do Perry (`PERRY/docs/src/internals/gc-rooting-invariant.md:108-130`).
  O §7.3 tem um teste para isso.
* **C5 — `async` suspenso.**
  * O que atravessa um `await` mora no quadro do heap (`EN/lower/async_sm.rs:25-38`). Ele é alcançado
    pelas arestas do heap, não pela pilha.
  * Nenhum quadro nativo sobrevive à suspensão. Por isso a armadilha do linzj com o `SuspendState`
    (`bad26cc6d9`, slot "vivo" antes de escrito, copiado para o heap) **não se aplica** do mesmo jeito.
  * A forma geral dela se aplica sim: um slot do mapa lido antes de ser escrito (§7.3, D5).
* **C6 — isolados.**
  * Cada isolado é uma thread com o seu `Heap` e o seu `Contexto` (`RT/nucleo.rs:140`; `RT/heap.rs:334-354`).
  * A coleta é síncrona e só percorre a pilha da própria thread. Não há parada do mundo nem *safepoint
    poll* entre threads. Por isso o modo mapas **não** usa `place-safepoints`.
* **C7 — pontos de coleta.**
  * A coleta só acontece numa alocação ou numa chamada ao runtime que pode alocar ou rodar Dart.
  * O ponto seguro do laço (`dartforge_interrupcao_pendente`) só chama o runtime no caminho lento
    (`EN/lower/fn_builder.rs:913-938`). Essa chamada é um statepoint como qualquer outra.
  * Intrínsecos do LLVM e externs que não coletam levam `"gc-leaf-function"`.
* **C8 — `--gc-stress`.**
  * `DARTFORGE_GC_STRESS=1` coleta em toda alocação, e os mortos são zerados (`RT/heap.rs:582-626`).
  * O diferencial roda o corpus assim (`crates/diferencial/src/main.rs:13-20`).
  * As quatro combinações passam por ele, e o teste **prova** que coletou (§7.5).
* **C9 — exceção nunca atravessa quadro Rust.** O runtime é `panic=abort` (`EN/cache.rs:6`, `:50`).
  * Um desenrolar estrangeiro entrando num quadro Rust é indefinido com `extern "C"`.
  * Com `extern "C-unwind"`, o desenrolar estrangeiro aborta sob `panic=abort` (Rust Reference, "Unwinding";
    não verificado neste repositório).
  * Pior ainda: pular quadros Rust deixaria `RefCell` emprestado e frames de `com_raizes` empilhados.
  * O Perry pula quadros Rust de propósito (`eh_windows.rs:33-37`). **Nós não podemos.** Ver §4.6.
* **C10 — semântica igual à da VM.** `finally`, `rethrow`, `on T`, `Isolate.exit` não capturável,
  `StackOverflowError` capturável, rastro. O corpus diferencial compara byte a byte.

---

## 3. Modo de raízes por mapas (B)

### 3.1 Estratégia do LLVM

* **Estratégia.** `gc "statepoint-example"` em toda função gerada que tenha referência viva através de
  chamada. Ela usa statepoints e trata **todo** ponteiro em `addrspace(1)` como referência (GarbageCollection.rst
  e Statepoints.rst, seção "Supported Strategies"; o código está em `llvm/lib/IR/BuiltinGCs.cpp`).
  * Não usamos `coreclr`: ele também considera `addrspace(1)` e é o que o linzj usou
    (`docs/PESQUISA-LLVM-DART-AOT.md:82-83`), mas não traz nada a mais para nós.
  * Uma `GCStrategy` própria só seria possível com plugin, e o `clang`/`lld-link` do Windows não carregam
    plugins (§3.4).
* **Ressalvas da própria documentação** (Statepoints.rst da tag 22.1.8):
  * o rótulo "experimental" significa "sem garantia de compatibilidade entre versões" (:18-21);
  * o lowering é "somewhat poor": derrama todo ponteiro vivo na pilha (:764-768). O padrão é
    `max-registers-for-gc-values=0` (StatepointLowering.cpp:72-74), o que nos favorece: só aparecem locais
    `Indirect [SP+off]`;
  * o IR cresce, com custo de memória e de tempo (:770-774);
  * relocação em caminhos excepcionais com relançamento está "broken in ToT" (:779-783; §4.4);
  * regiões `alloca` no `gc-live` são "not well exercised", e o RS4GC "does not do anything for allocas"
    (:306-309);
  * só AArch64 e X86_64 são suportados (:728-729);
  * `inttoptr`/`ptrtoint` quebram a inferência de base no modelo abstrato, mas não para quem baixa direto
    para o modelo físico (:739-748);
  * o RS4GC rematerializa o `gep +2` depois do statepoint e registra só a base (experimento `t1`). Isso
    confirma o C3.
* **Datalayout.** Acrescentar `-ni:1` ao `target datalayout` (`EN/alvo.rs:48-58`, que hoje fixa a string no
  Windows e no Linux x86-64). Ponteiros não integrais impedem o otimizador de **introduzir**
  `ptrtoint`/`inttoptr` em `addrspace(1)`. É o que o Julia faz com `-ni:10:11:12:13`
  (`E:\references\julia\src\jitlayers.cpp:1909`; `doc/src/devdocs/llvm.md:333-341`).
* **O JIT.** A `LLJIT` exige o mesmo datalayout que o dela (`docs/JIT.md`; `EN/alvo.rs:40-46`). Portanto o
  JIT no modo mapas cria a `LLJIT` com o datalayout acrescido (§3.8).

### 3.2 Representação de `Ref` no IR

* **Hoje.** `Type::Ref` sai como `i64` (`EN/hir.rs:44-57`). A etiqueta é testada com aritmética, o endereço de
  campo é `inttoptr(h − 2 + …)`, e há 58 `inttoptr` e 24 `ptrtoint` escritos à mão nos geradores de IR
  (contagem por `grep` em `EN/`).
* **No modo mapas,** `Type::Ref.llvm_ir()` passa a ser `ptr addrspace(1)`. As regras de emissão:

| operação | modo sombra (hoje) | modo mapas |
|---|---|---|
| constante null | `add i64 0, 0` | `ptr addrspace(1) null` |
| `Smi` de `int` | `or (shl n,1),1` | `inttoptr` do mesmo `i64` para `addrspace(1)`; é uma base que o coletor descarta (C2) |
| teste de etiqueta, `identical`, hash | aritmética em `i64` | `ptrtoint` → `i64`. O resultado **nunca** volta a ser referência |
| endereço do campo | `inttoptr(h − 2 + 8k)` | `getelementptr i8, ptr addrspace(1) %h, i64 (8k − 2)`: derivado, com base `%h` |
| leitura e gravação de campo `Ref` | `load`/`store i64` | `load`/`store ptr addrspace(1)` |
| argumento e retorno de função Dart | `i64` | `ptr addrspace(1)` |
| argumento e retorno de extern do runtime | `i64` | `ptr addrspace(1)` na **declaração**; na ABI é o mesmo registrador inteiro |
| objeto estático (literal da imagem) | `ptrtoint @obj + 2` | o global declarado em `addrspace(1)` (`@obj = … addrspace(1) …`) e `getelementptr (i8, ptr addrspace(1) @obj, i64 2)`, uma constante. **Nunca `addrspacecast`**: o RS4GC não o suporta ([#61917](https://github.com/llvm/llvm-project/issues/61917), §9.2). Falta confirmar que a seção `.dfimg$m` (`EN/alvo.rs`) aceita global em `addrspace(1)` no x86-64 (E2.1) |
| `phi`/`select` de `Ref` | `i64` | `ptr addrspace(1)`. Uma entrada indefinida é **sempre** `null`, nunca `undef`/`poison` (§7.3, D5) |
| `alloca` de `Ref` (local mutável) | slot próprio no quadro (G2) | `alloca ptr addrspace(1)` iniciado com `null`. O `mem2reg` o promove antes do RS4GC; o RS4GC não rastreia memória |
| `alloca` cujo endereço escapa (`allocas_no_quadro`, `EN/llvm/mod.rs:53-57`) | slot no quadro | **quadro-sombra residual** da função (§3.7), nunca `alloca` comum |

* **Tamanho da mudança.** O emissor tem ~400 ocorrências de `i64` só em `EN/llvm/mod.rs`, além dos
  ajudantes `@df.*` escritos em IR (76 `define` em `EN/llvm/*.rs`, `lib.rs`, `particao.rs`, `poda.rs`).
* **Disciplina** (formato 6 do Perry e B.6 do Julia): toda conversão passa por **funções do emissor**, nunca
  por `format!` solto:
  * `ref_para_bits`;
  * `bits_para_ref_permitido(origem)`;
  * `endereco_de_campo`.

  No modo sombra elas emitem o IR de hoje, byte a byte.
* **Critério de pronto da Etapa 2.0:** com `--raizes=sombra`, o IR é idêntico ao de antes (os resumos de
  determinismo e as chaves de cache continuam valendo, como `EN/alvo.rs:8-10` exige).

### 3.3 O que o emissor marca

* **Folhas.** `"gc-leaf-function"` vai:
  * na **declaração** de cada extern do runtime que não pode coletar;
  * na **chamada** de cada `@df.*` de caminho rápido que não coleta.

  A fonte é a mesma de `pode_coletar` (`EN/llvm/mod.rs:2728`), e a tabela de efeitos fica por extern:
  `{coleta, lança, roda Dart}`. É o plano antigo de `docs/PESQUISA-LLVM-DART-AOT.md:270-283`. O Perry gera
  essa tabela do grafo de chamadas do runtime (`PERRY/crates/perry-codegen/src/gc_call_effects.rs:8-37`).
  * Uma marca de folha errada é o defeito mais perigoso deste modo. Ela tem teste próprio: com
    `--gc-stress`, um contador por extern exige **zero** coletas dentro de uma folha (§7.3, D7).
  * O `"gc-leaf-function"` vale **também em `invoke`** (`PERRY/changelog.d/11531-invoke-gc-leaf.md`).
  * Vale também em todo `asm` em linha. Sem isso, o RS4GC o embrulha num statepoint e o verificador recusa
    (`PERRY/changelog.d/8128-rs4gc-inline-asm-and-compile-blowup.md:3-11`).
* **Argumentos (C1).** Depois de cada chamada **não folha** a extern do runtime que tenha argumento `Ref`,
  vai um uso fictício de cada argumento `Ref`. A forma preferida é `call void (...) @llvm.fake.use(ptr
  addrspace(1) %a)`. A alternativa é `asm sideeffect "", "r"(ptr addrspace(1) %a)` marcado como folha.
  * O uso fictício põe o argumento no conjunto vivo depois da chamada, e o RS4GC o registra.
  * Falta confirmar que o `llvm.fake.use` sobrevive ao O2 e entra na vivacidade do RS4GC. É o primeiro
    experimento da Etapa 2 (§6, E2.1). Se não sobreviver, fica o `asm`.
  * **Observação que pode dispensar tudo isso** (verificada num caso só, `E:\dftemp\spec-mapas\t2b.ll`):
    o RS4GC 22.1.8 pôs no `gc-live` da chamada `@usa(%obj.relocated1)` o próprio argumento, que **não**
    tinha uso depois dela. O `llvm-readobj --stackmap` do objeto COFF mostra o registro dessa chamada com
    `Indirect [R#7 + 32]` (RSP+32), no quadro de 40 bytes.
    * Se isso valer em geral (argumento `ptr addrspace(1)` de chamada não folha sempre no mapa da
      própria chamada), o C1 sai de graça, como no modo sombra.
    * A E2.1 confirma no fonte do RS4GC e com um teste de sabotagem (D1) antes de dispensar o uso
      fictício.
  * Chamadas Dart→Dart **não** precisam disso. O argumento que o chamado ainda usa está no mapa do
    chamado; o que o chamador ainda usa está no mapa do chamador.
* **`ret`, `this` e o ambiente.** Nada especial: são SSA como os outros.

### 3.4 Onde o RS4GC roda no pipeline

O RS4GC só é correto e barato **depois** da otimização:

* Statepoints.rst ("RewriteStatepointsForGC") recomenda rodar tarde.
* Depois do rewrite, o `gc.relocate` é opaco e o otimizador enxerga pouco (`docs/PESQUISA-LLVM-DART-AOT.md:157-161`).
* Um statepoint não é mais inlinável.

O Perry roda **antes** do O2: `always-inline,function(mem2reg,sccp),rewrite-statepoints-for-gc`
(`PERRY/crates/perry-codegen/src/linker.rs:25-38`). O motivo: GVN/CSE fundiam cadeias
`ptrtoint`/`inttoptr` através de safepoints futuros (`PERRY/docs/statepoint-gc-experiment.md:540-547`). Isso
**não** serve para nós:

* o ThinLTO e o inlining entre o programa e o SDK são o que hoje segura o desempenho do programa grande
  (`NATIVO-PRODUCAO-GRANDE.md` §3, item 2);
* o problema do Perry nasce de valores NaN-box que **só** viram referência na fronteira. Com `addrspace(1)`
  de ponta a ponta e `-ni:1`, o otimizador não fabrica essas cadeias.

A Etapa 2 mede as duas ordens no mesmo programa (E2.3) antes de fixar uma.

**Por perfil.** O fato que decide (verificado em 2026-10-02 com o LLVM 22.1.8 de `E:\llvm`):

* `ld.lld` e `ld64.lld` aceitam `--lto-newpm-passes` e `--load-pass-plugin`;
* `lld-link` **não aceita nenhum dos dois**: `-lto-newpm-passes:foo` dá "ignoring unknown argument";
* o `opt` lista `rewrite-statepoints-for-gc`, `place-safepoints` e `fixup-statepoint-caller-saved`.

| perfil | hoje | modo mapas |
|---|---|---|
| desenvolvimento (`-O0`, objetos por parte) | Clang ou LLVM embutido gera objeto | pipeline próprio em processo (feature `llvm-embutido`): `default<O0>` → `mem2reg` → verificador → `rewrite-statepoints-for-gc` → verificador → código. Sem o LLVM embutido: `opt` + `llc`. A geração de código **sem FastISel** nas funções `gc`, por causa das [#56158](https://github.com/llvm/llvm-project/issues/56158) e [#39891](https://github.com/llvm/llvm-project/issues/39891) (`gc.relocate` em -O0) |
| produção, programa pequeno (um módulo, LTO com o SDK) | `clang` + LTO do linker | Linux/macOS: `--lto-newpm-passes='lto<O2>,rewrite-statepoints-for-gc'` (a confirmar que o nome `lto<O2>` vale em `--lto-newpm-passes`). Windows: LTO completa em processo (`llvm-link` do programa com o bitcode do SDK, `default<O2>`, RS4GC, código), entregando objeto ao `lld-link` |
| produção, programa grande (partes em ThinLTO, `EN/driver.rs`) | `lld-link /lldltocache` | **ThinLTO distribuído**: `lld-link /thinlto-index-only` gera os índices; para cada parte, `clang -x ir parte.o -fthinlto-index=parte.thinlto.bc -emit-llvm -c` dá o bitcode **depois** da importação e da otimização do backend do ThinLTO; nós rodamos RS4GC + código e ligamos os objetos. O `lld-link` 22.1.8 também tem `/thinlto-distributor:` (o DTLTO), que pode chamar o nosso passo diretamente |

* **Experimento** (verificado em 2026-10-02, `E:\dftemp\spec-mapas\dtlto\run.sh`, LLVM 22.1.8 no Windows).
  Dois arquivos C, `f` em `a.c` chamando `g` de `b.c`:
  1. `clang -O2 -flto=thin -c`;
  2. `lld-link -thinlto-index-only …` gerou `a.o.thinlto.bc` e `b.o.thinlto.bc`;
  3. `clang -O2 -x ir a.o -fthinlto-index=a.o.thinlto.bc -emit-llvm -S` deu o IR de `f` **com `g`
     importado e inlinado** (`%2 = mul nsw i32 %0, 3`, sem `call`).

  Ou seja, o bitcode depois do backend do ThinLTO sai, e é nele que o RS4GC entra. A escala (77 partes do
  backend), o cache e o caminho pelo `/thinlto-distributor:` são a E3.1.
* O cache do ThinLTO (`/lldltocache`) deixa de servir. O cache vira o nosso, por parte, com chave = bitcode
  importado + índice + modo (§5.4).
* **Orçamento.** O RS4GC cresce de forma superlinear em funções com muitos vivos e muitos safepoints. O Perry
  mediu 439 mil → 6,5 milhões de instruções numa função (`PERRY/changelog.d/8589-root-spill.md`).
  * Antes do RS4GC, contar `vivos × safepoints` por função.
  * Acima do teto, a função cai para o quadro-sombra (§3.7).
  * O teto entra na chave do cache.
* **`optnone`.** Uma função `optnone` pula o `mem2reg` e perde todas as raízes
  (`PERRY/changelog.d/8586-rs4gc-budget-assert.md:3`). O modo mapas **proíbe** `optnone` em função com
  `gc`. O `-O0` usa `mem2reg` explícito.
* **Verificador.** Roda depois do RS4GC sempre, inclusive na produção. O Perry viu o RS4GC produzir IR
  inválido que só morria na seleção de instruções (`PERRY/crates/perry-codegen/src/inprocess/optimize_emit.rs:117-130`).

### 3.5 O mapa compacto

* **O formato do LLVM** (StackMaps.rst, "Stack Map Format", versão 3) é feito para patching de JIT:
  * cabeçalho;
  * `StkSizeRecord` por função;
  * constantes;
  * registros por chamada, com id de 64 bits, deslocamento da instrução, locais e *live-outs*;
  * alinhamento de 8.

  A própria documentação diz que o consumidor deve convertê-lo para um formato próprio. No Perry, 40,6% da
  seção eram locais `Constant`, 13,3% pares base/derivado duplicados, 18% cabeçalhos com o id nunca usado e
  11,3% enchimento. A compactação dele deu 4,21 MB → 227 KB (`PERRY/docs/statepoint-gc-experiment.md:408-461`).
* **Quando converter.** Na geração de cada objeto, no mesmo passo que roda o RS4GC (§3.4):
  1. o backend em processo emite o objeto;
  2. nós lemos a seção de stack maps do próprio objeto (crate `object`);
  3. convertemos;
  4. emitimos `__df_gcmap` como dados do objeto;
  5. removemos a seção do LLVM.

  O Perry converte no texto assembly (`gc_map.rs:40-49`). Nós ficamos no objeto, porque o caminho em
  processo já entrega objeto e o JIT precisa do mesmo conversor (§3.8).
* **O que o LLVM 22.1.8 emite.** Verificado com `llc -O2 -filetype=obj` sobre o resultado do RS4GC em
  `E:\dftemp\spec-mapas` (`out_run.txt`, `out_run2.txt`):
  * `.llvm_stackmaps` em COFF x86-64 **e** arm64, sem relocação separada visível no `objdump -h`;
  * `.llvm_stackmaps` + `.rela.llvm_stackmaps` em ELF x86-64 e aarch64;
  * `__llvm_stackmaps` em Mach-O x86-64 e arm64.

  Os problemas de ligação dessa seção:
  * na imagem PE, o `lld-link` corta o nome para `.llvm_st` (lld `COFF/Writer.cpp` L1575-1588, segundo
    a pesquisa de issues);
  * o `ld.lld --gc-sections -shared` **descartou** a seção e, sem `--gc-sections`, recusou a relocação
    `R_X86_64_64` sem `-fPIC` (verificado; é a [#75074](https://github.com/llvm/llvm-project/issues/75074)).

  As duas coisas reforçam converter e remover a seção **antes** da ligação.
* **O registro, como sai.** Para o `invoke` de uma função com um `ptr addrspace(1)` vivo (`t2b.ll`), o
  `llvm-readobj --stackmap` dá versão 3, tamanho de quadro 40, e por registro três `Constant 0`
  (convenção, *flags*, número de *deopt*) seguidos do par base/derivado `Indirect [R#7 + 32]` duas vezes.
  R#7 é o RSP no DWARF. O conversor descarta os três primeiros e o derivado igual à base.
* **Símbolos e relocações.** Cada `StkSizeRecord` aponta a função por endereço, uma relocação de 64 bits.
  O mapa compacto guarda o deslocamento **relativo ao começo da seção** (32 bits, como a v6 do Perry, que
  economizou 631 KB de relocações dinâmicas em 20 mil funções; `changelog.d/11533`). A seção é só de leitura:
  * ELF: `SHF_GNU_RETAIN`;
  * COFF: `.dfgcm$<parte>`, agrupada e ordenada pelo `lld-link` como as seções `.dfimg$m` de hoje
    (`EN/alvo.rs`);
  * Mach-O: `__DATA_CONST,__df_gcmap` com `no_dead_strip`.

  O nome no COFF tem no máximo 8 bytes antes do `$`.
* **Layout v1** (little-endian):

  ```
  "DFGM" u8 versão u8 alvo u16 bandeiras
  u32 n_funcoes
  n × { i32 inicio_da_funcao (relativo a este blob), u32 tamanho_do_quadro, u32 n_registros, u32 inicio_no_fluxo }
  fluxo de varints, por função:
    por registro: varint deslocamento_de_retorno (delta do anterior)
                  varint cabeçalho = (n_raizes << 1) | repete_o_anterior
                  n × varint slot = zigzag(deslocamento) << 1 | base_fp
  ```

  * Pares base/derivado: só a **base** entra. Coletor não móvel (C3); um derivado sem base vivo é defeito.
  * O conversor **recusa** (erro de compilação, não aviso):
    * local `Register`: o desenho exige tudo derramado (`statepoint-max-registers-for-gc-values` = 0, o
      padrão, a confirmar no 22.1.8 em E2.1). O Perry achou o mapa de `stackmap` comum inseguro por isso
      (`statepoint-gc-experiment.md:156-166`);
    * local `Constant`/`ConstIndex` em raiz: null e constantes não precisam de raiz e são descartados; mas
      uma constante **par não nula** que não seja objeto estático é defeito;
    * dois registros no mesmo deslocamento de retorno.
* **Ida e volta conferida.** Toda conversão decodifica o compacto e o compara ao original
  (`verify_roundtrip` do Perry, `gc_map.rs:865-878`).
* **Registro em tempo de execução.**
  * `@df.preparar_isolado` de cada imagem (executável, DLL do SDK) já chama `dartforge_registrar_imagem`
    (`RT/gc_raizes.rs:474-480`). Ele passa a chamar também `dartforge_registrar_mapa(inicio, fim)`.
  * O índice por imagem é ordenado por endereço de função e decodificado **por função, sob demanda**. O Perry
    decodificava 2 milhões de registros (~117 MB) para atender 74 consultas antes de passar ao preguiçoso
    (`stack_maps_lazy.rs:7-15`).
  * Busca: endereço de retorno → imagem → função (busca binária) → registro **exato**. Sem a tolerância de
    ±16 bytes do Perry: um endereço de retorno sem registro exato numa função com `gc` é defeito e aborta no
    modo de verificação.

### 3.6 Como o runtime percorre os quadros

* **A coleta.** Ela começa dentro de `Heap::coletar`, chamada de dentro do runtime. Entre ela e o primeiro
  quadro Dart há quadros Rust: o extern chamado, `HEAP.with` e o alocador. O percurso:
  1. captura o contexto corrente;
  2. sobe quadro a quadro;
  3. em cada quadro, olha o endereço de retorno no índice de mapas. Se é de função gerada, visita as raízes
     daquele registro; senão, só sobe.

  Termina na base da pilha da thread. Os quadros-sombra residuais (§3.7) e os frames de `com_raizes` são
  visitados à parte, como hoje.
* **Por alvo:**

| alvo | percurso | viável? |
|---|---|---|
| Windows x86-64 | `RtlCaptureContext` + `RtlLookupFunctionEntry` + `RtlVirtualUnwind`. **A cadeia de RBP não serve aqui**: com `"frame-pointer"="all"`, o `llc` 22.1.8 gera `leaq 128(%rsp),%rbp; .seh_setframe %rbp,128` num quadro grande (experimento `t11.ll`). O RBP aponta para o meio do quadro, não para o RBP salvo (X86FrameLowering.cpp:1270-1277; a ABI x64 da Microsoft, `UWOP_SET_FPREG`). O `.pdata`/`.xdata` é obrigatório em toda função não folha do x64, inclusive nas do Rust e nas do sistema. Cada endereço é conferido contra os limites da pilha, e o percurso aborta na primeira anomalia (o Perry faz isso em `PERRY/crates/perry-runtime/src/gc/roots/stack_maps.rs:1493-1739`) | **sim**: o alvo da Etapa 2 |
| Linux x86-64 e macOS x86-64 | `_Unwind_Backtrace` com `_Unwind_GetCFA`. O runtime e o SDK precisam de `-C force-unwind-tables=yes`, senão o desenrolador vê zero quadros (`PERRY/changelog.d/7322-statepoint-prerequisites.md:59-67`). O Perry corrigiu a base de `CFA − 8 − tamanho` para o CFA puro (`stack_maps.rs:1446-1461`). **Nós derivamos a fórmula por teste, não por cópia** | sim, Etapa 4 |
| macOS arm64 e Linux aarch64 | cadeia `x29` (`"frame-pointer"="non-leaf"` no código gerado), com o desenrolador como reserva. No Darwin, SP = FP + 16 − tamanho; no Linux o par x29/x30 fica abaixo dos *callee-saved* e o Perry decodifica o prólogo (`stack_maps.rs:985-1003`, `changelog.d/7398`). Proposta: usar **só** o desenrolador no começo e a cadeia como otimização medida | sim, Etapa 4 |
| Windows arm64 | não há percorredor no Perry (`gc_map.rs:1382-1396`). Duas saídas: a ABI garante a cadeia `x29` ("must point to the previous {x29, x30} pair", learn.microsoft.com, arm64-windows-abi-conventions), confirmada no `t11.ll` (`stp x29,x30,[sp,#8]; add x29,sp,#8`); ou `RtlVirtualUnwind` com o `CONTEXT` do arm64. O `.llvm_stackmaps` sai no COFF arm64 (`t1`, `t2b`) | **fallback: sombra** até alguém escrever e testar |
| JIT (todos) | §3.8 | fallback: sombra até a Etapa 4 |

* **Dois percorredores e uma conferência.** Com `DARTFORGE_GC_PERCURSO=conferir`, o percurso pelo
  desenrolador e o percurso rápido (cadeia de FP, onde houver) têm de visitar **o mesmo** conjunto de slots.
  Se diferirem, pânico. É a única checagem que pega um percorredor que pula quadros (`stack_maps.rs:651-662`).
* **Custo.** O percurso pelo desenrolador custa por quadro e por coleta. As coletas menores são frequentes
  (`LIMITE_JOVEM`). No Raspberry Pi 5, o Perry mediu +14,7% de tempo causado pelo *parsing* de CFI no
  desenrolador. Com cache por PC caiu para −1,74% (`statepoint-gc-experiment.md:626-707`).
  * Proposta: um cache por endereço de retorno → (tamanho do quadro, registro).
  * Medir antes de otimizar (§8).

### 3.7 Mistura dentro do modo mapas: o quadro-sombra residual

O modo mapas **não** apaga a pilha-sombra. Ela continua para três casos, por função:

1. `alloca` `Ref` cujo endereço escapa (`allocas_no_quadro`);
2. funções acima do orçamento do RS4GC (§3.4);
3. funções com uma construção que o modo mapas ainda não sabe baixar. Ela é **listada**, e o emissor
   recusa em vez de errar.

Uma função assim não leva `gc "statepoint-example"`. Ela grava os slots como hoje, e o percorredor a pula
(o endereço de retorno não tem registro). O Perry tem o mesmo mecanismo ("root spill", `changelog.d/8589`).

* **Inlining entre funções com e sem `gc`.** O inliner do LLVM põe a estratégia do chamado num chamador sem
  estratégia, e recusa estratégias diferentes (`llvm/lib/Transforms/Utils/InlineFunction.cpp`, não
  verificado no 22.1.8). O resultado é uma função com quadro-sombra **e** mapa: correto (as duas fontes são
  visitadas), mas com custo duplicado. O relatório de compilação conta essas funções.

### 3.8 JIT e hot reload

* **Hoje.** O JIT entrega IR textual à `LLJIT` pela API C (`LLVMOrcLLJITAddLLVMIRModuleWithRT`,
  `crates/jit/src/ffi.rs:57-61`). A geração de objeto é da `LLJIT`.
* **No modo mapas,** o JIT passa a gerar o objeto ele mesmo:
  1. pipeline em processo (§3.4);
  2. `LLVMTargetMachineEmitToMemoryBuffer`;
  3. conversão do mapa (§3.5);
  4. `LLVMOrcLLJITAddObjectFile`.

  Os deslocamentos de função do mapa compacto são resolvidos para endereços absolutos depois da ligação do
  objeto (`LLVMOrcLLJITLookup`). O mapa é registrado por geração.
* **Recarga.**
  * A publicação de uma geração acontece sem quadro Dart na pilha (`RT/gc_raizes.rs:212-229`, J02), e o
    código antigo que continua vivo só existe em closures e quadros `async` do heap, nunca na pilha nativa.
  * O mapa de uma geração aposentada sai do índice quando a memória dela é solta.
  * Uma função que a recarga mantém (J04) mantém o mapa da geração dela.
  * A mesma sessão JIT não mistura modos: o modo é fixado na criação da sessão e entra na impressão digital
    do módulo.
* **Tabelas de desenrolamento no JIT (dimensão 1).**
  * Linux e macOS: a `LLJIT` usa JITLink (`ObjectLinkingLayer`) com `EHFrameRegistrationPlugin` ou
    *compact unwind* (LLJIT.cpp:1220-1259 da tag 22.1.8).
  * COFF: usa **RTDyld** (`UseJITLink = !TT.isOSBinFormatCOFF()`, LLJIT.cpp:806-816; `SectionMemoryManager`,
    :946-963).
    * O RTDyld COFF x64 entrega o `.pdata` a `registerEHFrames`, que procura `__register_frame`
      dinamicamente (RuntimeDyldCOFFX86_64.h:288-313; RTDyldMemoryManager.cpp:32-125). Num LLVM do MSVC
      isso provavelmente não registra nada; a Microsoft exige `RtlAddFunctionTable` para código dinâmico.
    * No COFF arm64, `registerEHFrames() {}` é vazio (RuntimeDyldCOFFAArch64.h:386).
  * O modo `tabelas` no JIT do Windows pede então um gerenciador de memória nosso que chame
    `RtlAddFunctionTable`. Até um teste provar que uma exceção atravessa dois quadros JIT, o JIT fica com
    `checagem`.
  * No modo mapas, o mesmo gerenciador acha a seção de stack maps pelo nome em `allocateDataSection`
    (StackMaps.rst:420-427). Fica como alternativa ao "gerar o objeto nós mesmos" acima.
* **Padrão do JIT:** `sombra` + `checagem`, nas duas dimensões, até a Etapa 4.

### 3.9 Fora do escopo, mas registrado: a pilha-sombra colocada tarde (o modelo do Julia)

* **O que o Julia faz.** Coloca a mesma pilha-sombra encadeada **depois** do otimizador
  (`E:\references\julia\src\pipeline.cpp:571-590`):
  1. vivacidade sobre o IR otimizado;
  2. coloração;
  3. as gravações afundadas até logo antes do primeiro ponto de coleta que precisa delas
     (`llvm-late-gc-lowering.cpp:2405-2435`);
  4. o slot morto zerado antes do ponto seguinte (`:2384-2403`);
  5. função sem ponto de coleta fica sem quadro (`:2597-2609`).

  Ele usa referências em espaços de endereço não integrais (`doc/src/devdocs/llvm.md:276-341`).
* **Para nós,** seria uma **terceira lowering** da dimensão raízes. Ela pede a mesma representação
  `addrspace(1)` do §3.2, mas não pede RS4GC, nem mapas, nem percorredor. Os ganhos esperados:
  * menos gravações, porque o DCE e o afundamento vêm antes;
  * nenhum risco de backend do LLVM, porque o quadro continua explícito.
* **O preço:** um passe nosso dentro do pipeline do LLVM. No Windows isso só é viável em processo, pelos
  mesmos motivos do §3.4.
* **Decisão:** fora desta especificação. Se a Etapa 2 abandonar o modo mapas pelo backend do LLVM, esta é a
  alternativa natural, e o trabalho do §3.2 (a representação) é reaproveitado inteiro.

---

## 4. Exceções sem custo (dimensão 1, `tabelas`)

### 4.1 O lowering, igual em todos os alvos

* **Dentro de `try`.** Uma chamada que pode lançar vira `invoke`. O destino de desenrolamento é um bloco
  `landingpad` próprio da chamada, com `catch ptr null` (pega tudo) e tipo `token` no modo mapas (§4.4), que
  desvia para o **mesmo** alvo de exceção da HIR de hoje.
  * O tratador é sempre "pega tudo" para a personalidade.
  * O teste `on T`, o `capturavel`, a guarda do `finally` e o `switch` de razão continuam sendo código Dart
    comum no bloco de destino (`EN/lower/comandos.rs:579-861`).
  * Dart não tem filtros; a VM faz o mesmo (o `catch` é um bloco que testa e relança).
* **Fora de `try`,** uma chamada Dart→Dart é `call` comum: **sem checagem**. O desenrolamento atravessa o
  quadro.
* **`throw`/`rethrow` dentro de `try` na mesma função** continuam como hoje: `dartforge_exception_throw` e
  desvio direto. Não há desenrolamento.
* **`throw` sem tratador local:** `dartforge_exception_throw` seguido de `call void @df.lancar()
  noreturn`, que dispara o desenrolamento nativo (§4.3).
* **Extern do runtime que pode lançar** (pela tabela de efeitos, §3.3). A extern continua só gravando a
  pendência e retornando: o runtime **não** desenrola (C9). Depois dela vai a checagem de hoje, com o
  destino trocado:
  * dentro de `try` → o alvo local;
  * fora → `@df.lancar`.

  A checagem só fica depois das externs que lançam. A medida E1.2 conta quantas das 495 mil sobram.
* **Saída por exceção.** Não existe mais na função gerada: o `ret` do valor padrão das saídas por exceção
  (`fn_builder.rs:977-988`) some no modo `tabelas`.

### 4.2 A personalidade, em Rust, com uma LSDA só

* **O LLVM.** Uma personalidade que ele não reconhece:
  * não usa funclets;
  * gera uma LSDA no formato Itanium (`GCC_except_table`);
  * no COFF x64, instala a rotina como *language handler* com `.seh_handler <pers>, @unwind, @except`.

  Isso é afirmado e usado pelo Perry (`PERRY/crates/perry-runtime/src/eh_windows.rs:16-31`;
  `PERRY/crates/perry-codegen/src/stmt/try_stmt.rs:46-55`).
* **Verificado aqui** com o `llc` 22.1.8 (`E:\dftemp\spec-mapas\t2b.ll`, `t6.ll` e `t8.ll`, com
  `personality ptr @dartforge_personalidade`):
  * `x86_64-pc-windows-msvc` e `aarch64-pc-windows-msvc` emitem
    `.seh_handler dartforge_personalidade, @unwind, @except` e `GCC_except_table0` em `.seh_handlerdata`;
  * Linux e macOS emitem `.gcc_except_table`/`__gcc_except_tab`;
  * com `gc "statepoint-example"` e `landingpad token cleanup`, o RS4GC produz o `invoke` de statepoint e o
    `gc.relocate` no *landing pad*, e o `llc` gera objeto nos quatro alvos testados.
* **A decodificação da LSDA** (tabela de *call sites*, `catch` só como "pega tudo") é **uma só**,
  compartilhada pelos três sistemas (`eh_lsda.rs` do Perry).
* **Linux/macOS: `dartforge_personalidade`**, com a ABI `_Unwind_Personality_Fn`:
  * fase de busca: devolve `_URC_HANDLER_FOUND` se o *call site* tem *landing pad*;
  * fase de limpeza: instala o contexto (`_Unwind_SetIP`, `_Unwind_SetGR`) e devolve `_URC_INSTALL_CONTEXT`;
  * recusa `exception_class` diferente de `"DARTFRGE"`: uma exceção C++ ou Rust estrangeira continua a subir.
* **Windows x64: a mesma função com a assinatura de `EXCEPTION_ROUTINE`:**
  * só aceita o código `0xE0444652`, um código de cliente próprio, não contínuo;
  * na busca, achou o pad → `RtlUnwindEx(quadro, pad, …)`, que não volta;
  * na fase de desenrolar, nada a fazer.

  O Perry faz exatamente isso (`eh_windows.rs:143-205`), e recusa qualquer outro código: uma violação de
  acesso não vira exceção Dart (`:42-47`, `:160-166`).
* **Windows arm64:** a mesma rotina, com o `DISPATCHER_CONTEXT` do arm64. Precisa de teste próprio; até lá,
  `checagem`.

### 4.3 O lançamento

* **`@df.lancar` é gerado em IR,** não escrito em Rust. Um quadro Rust de onde parte um desenrolamento
  aborta sob `panic=abort` (C9).
  * Linux/macOS: chama `_Unwind_RaiseException(ptr %obj)`. `%obj` é o `_Unwind_Exception` da thread, um campo
    novo no fim do `Contexto` (`RT/heap.rs:246-305`, com o `assert` de deslocamento em `:315-332`), com
    `exception_class = "DARTFRGE"` e sem carga.
  * Windows: chama `RaiseException(0xE0444652, EXCEPTION_NONCONTINUABLE, 0, null)` do `kernel32`.
* **O valor lançado** continua onde está hoje: a exceção pendente do runtime, enraizada
  (`raizes_do_runtime[0]`, `RT/excecoes.rs:276-278`). O *landing pad* o lê com
  `dartforge_exception_peek_ref` como o `catch` de hoje. O par `{ptr, i32}` da ABI é ignorado.
* **Volta de `_Unwind_RaiseException`.** Ela só volta se não achar tratador (`_URC_END_OF_STACK`). Como toda
  entrada de Dart é uma porta que pega tudo (§4.6), isso é defeito: `@df.lancar` aborta com mensagem.

### 4.4 Raízes durante `catch`, `finally` e `rethrow`

* **Sombra + tabelas (A1).** O desenrolamento pula as voltas de quadro (`%gcvolta`) dos quadros
  intermediários, e `Contexto::topo` fica apontando para um quadro morto. Regra:
  * todo *landing pad* começa restaurando o topo para o valor que ele tinha **depois do prólogo** da própria
    função: `%gcq` se a função tem quadro, ou o topo lido na entrada se não tem;
  * a função que pega é a que ainda vive, então esse valor é correto e desempilha de uma vez todos os
    quadros pulados;
  * as funções sem `try` **não** precisam de *landing pad* de limpeza;
  * as portas (§4.6) fazem o mesmo com o topo da entrada;
  * teste: `assert` de topo em `desempilhar_quadro` (`RT/heap.rs:379-385`) sob `--gc-stress` e exceções
    profundas (§7.3, D3).
* **Mapas + tabelas (B1).**
  * O RS4GC transforma o `invoke` num `invoke` de `gc.statepoint` e põe `gc.relocate` nas duas arestas.
  * O `gc.relocate` da aresta de exceção usa o **token do `landingpad`**, e um `landingpad { ptr, i32 }` com
    relocação é IR inválido. O *landing pad* do modo mapas é então `landingpad token` (Statepoints.rst,
    "Rewriting statepoints"; o Perry tropeçou nisso em `statepoint-gc-experiment.md:1002-1014`).
  * O RS4GC já registra no mapa do `invoke` o que está vivo no *landing pad*. Nada mais a fazer: os relocates
    são identidade.
  * **Ressalva documentada.** "Relocations along exceptional paths are currently broken in ToT … no way to
    represent a rethrow on a path which also has relocations" (Statepoints.rst:776-783 da tag 22.1.8; igual
    na doc atual).
    * Com o coletor não móvel, o valor relocado é o original, então a ressalva toca a forma do IR, não o
      valor. Isso é inferência.
    * Mesmo assim, o nosso relançamento **não** reaproveita o token do pad: é uma chamada nova a
      `@df.lancar`, que é ela mesma um statepoint com mapa próprio.
    * Testes D3 e D4 em B1 com `-verify-machineinstrs`.
  * **Nunca tirar os `gc.relocate`** "porque o coletor não move". Sem relocate, o valor some do mapa:
    * o lowering só coleta raízes de `getGCRelocates()` e dos `alloca` do `gc-live`
      (StatepointLowering.cpp:666-688, 1067-1091);
    * nos experimentos `t6.ll` e `t7.ll`, o `gc-live` sem relocate deu **zero** raízes, com o valor
      guardado em `%rbx`.

    A "simplificação para coletor não móvel" da doc (Statepoints.rst:258-280) só vale com `alloca`.
* **`finally` que aloca e relança.** O `finally` é um `catch` "pega tudo" que guarda a exceção (enraizada no
  runtime), roda o corpo e relança com `@df.lancar` se a razão for 2. É o código de `comandos.rs:740-849`
  com o destino trocado. Teste D4.
* **`rethrow` dentro de `catch`** relança o valor guardado pelo mesmo caminho.

### 4.5 `StackOverflowError`, `async`, desenrolar de isolado

* **Estouro de pilha.** O prólogo continua o mesmo (`EN/llvm/mod.rs:2456-2481`). Depois de
  `dartforge_estouro_de_pilha`, em vez de `ret`, vai `@df.lancar`.
  * O desenrolador precisa de pilha; a folga de 256 KiB (`RT/gc_raizes.rs:39`) foi pensada para montar o
    erro e é medida de novo para o desenrolador no teste D8.
  * No Windows, a página de guarda continua com a pilha do sistema. O `RaiseException` roda na folga.
* **`async`.**
  * O corpo `f$async` é uma função comum chamada pelo laço de eventos (Rust) **pela porta**.
  * O `try` sintético do topo do corpo (`EN/lower/async_sm.rs:333-352`, que leva a `_asyncRethrow`) é um
    `catch` como qualquer outro.
  * A exceção que chega por um `await` (`código == 1`) é lançada no ponto do `await` e segue o caminho
    local. Nenhuma exceção atravessa uma suspensão. A objeção 3 de `NATIVO-PLANO.md` §1.2 deixa de valer:
    a pilha nativa de cada retomada é uma pilha comum.
* **`Isolate.exit`/`kill`.**
  * A pendência não capturável continua: o `catch` testa `capturavel`, e o `finally` o pula
    (`comandos.rs:763-795`).
  * No modo `tabelas`, um *landing pad* que acha "não capturável" relança com `@df.lancar` até a porta mais
    externa, que a entrega ao laço de eventos.

### 4.6 Fronteiras Rust↔Dart e FFI

* **Dart → runtime.** O runtime nunca desenrola. A extern grava a pendência e volta (§4.1).
* **Runtime → Dart.** São ~50 pontos: `transmute` para `extern "C" fn` em 18 arquivos de `RT/`, por exemplo
  `closures.rs:211`, `isolados.rs:391-419`, `excecoes.rs:339-553` e `portas.rs:662`.
  * No modo `tabelas`, nenhum deles chama Dart diretamente. Eles chamam uma **porta** gerada por
    assinatura: `@df.porta.<k>(ptr %f, args…)`.
  * A porta faz `invoke %f(args…)`. Se volta, devolve o resultado. Se o desenrolamento chega, o *landing
    pad* "pega tudo" restaura o topo da pilha-sombra (A1) e retorna o valor padrão com a pendência ligada.
  * É exatamente o protocolo que o runtime já espera de uma função Dart que lançou: o código depois do
    `transmute` já confere `dartforge_exception_pending`.
  * As assinaturas são poucas (`i64` × aridade). O emissor gera uma porta por assinatura usada, e o runtime
    as acha por uma tabela registrada pelo `@df.preparar_isolado`.
  * **Teste:** toda chamada Rust→Dart passa por um ajudante único `chamar_dart::<F>(…)`. Um `grep` no CI
    recusa `transmute` para `extern "C" fn` fora dele.
* **Callbacks FFI.** A entrada C gerada (`RT/ffi_callbacks.rs:6-12`) já devolve o retorno excepcional se a
  closure lançou. No modo `tabelas`, ela é uma porta: o mesmo `invoke` com "pega tudo". Exceção Dart nunca
  atravessa quadro C, como na VM.
* **Dart → C pela FFI.** Uma exceção C++ ou SEH vinda de uma biblioteca C não é Dart. A personalidade a
  recusa (código ou classe diferente), e ela continua até o sistema, como na VM.
* **`extern "C-unwind"`.** Não é usado em lugar nenhum. A regra é: nenhuma função Rust está no caminho de um
  desenrolamento Dart.
  * A personalidade é chamada pelo desenrolador, mas não desenrola por si: só devolve um código (Itanium)
    ou chama `RtlUnwindEx`, que não volta para ela.
  * A personalidade é `extern "C"` (Itanium) ou `extern "system"` (Windows) e não pode entrar em pânico
    (`panic=abort` garante o abort).

---

## 5. O chaveamento

### 5.1 Nível 1: o build do dartforge (features do Cargo)

* **`crates/runtime`** ganha duas features, com padrão desligado:
  * `raizes-mapas` compila o índice de mapas, os percorredores e `dartforge_registrar_mapa`;
  * `excecoes-tabelas` compila a personalidade, o registro das portas e os campos do `Contexto`.

  Sem elas, o runtime é o de hoje, byte a byte. O `build.rs` monta as `staticlib` `aot`/`dll`
  (`crates/runtime/Cargo.toml`), uma variante **por modo**, com o modo no nome da biblioteca:
  `dfrt_<modo>.lib`.
* **`crates/emit_native`** ganha as features `raizes-mapas` e `excecoes-tabelas`. Elas compilam os lowerings
  novos. A `raizes-mapas` depende de `llvm-embutido` no Windows, porque o pipeline próprio precisa do LLVM
  em processo (§3.4).
* **`crates/cli`** ganha a feature `nativo-experimental = ["dartforge-emit-native/raizes-mapas",
  "dartforge-emit-native/excecoes-tabelas", "dartforge-runtime/…"]`.
* Um dartforge compilado sem a feature **recusa** a opção da CLI com mensagem, nunca cai em silêncio para o
  padrão. O Perry aprendeu isso com o `PERRY_RS4GC=1` (`PERRY/crates/perry-codegen/src/codegen/helpers.rs:135-138`).

### 5.2 Nível 2: a compilação de cada programa (CLI)

* **Opções.** `dartforge compile-native|aot … --raizes=sombra|mapas --excecoes=checagem|tabelas`. Elas
  entram no mesmo laço de opções de `crates/cli/src/nativo.rs:60-200`.
* **Padrão:** `sombra` e `checagem`.
* **Variáveis de ambiente** para medir sem mudar scripts: `DARTFORGE_RAIZES` e `DARTFORGE_EXCECOES`. A
  opção da CLI vence.
* **O diferencial** (`crates/diferencial/src/main.rs`) ganha `--raizes` e `--excecoes`, e os perfis
  `aot-mapas`, `aot-tabelas` e `aot-mapas-tabelas`, com e sem `--gc-stress`.
* **Por alvo,** uma opção pedida num alvo sem suporte é **erro** ("raízes por mapas não suportadas em
  aarch64-pc-windows-msvc"), não fallback. O fallback **por função** do §3.7 continua automático e é
  contado no relatório (`--timings`).

### 5.3 Padrões por alvo (estado final desejado, condicionado às medidas)

| alvo | raízes | exceções |
|---|---|---|
| Windows x86-64 | sombra até a Etapa 3 decidir; depois, o que ganhar | tabelas, se a Etapa 1 passar |
| Linux x86-64, macOS (x86-64 e arm64), Linux aarch64 | sombra até a Etapa 4 | tabelas, depois da Etapa 4 |
| Windows arm64 | sombra | checagem |
| JIT (todos) | sombra | checagem |
| alvo desconhecido | sombra | checagem |

* Trocar um padrão é decisão do dono, depois das medidas do §8 e com o corpus inteiro verde nas quatro
  combinações.
* O modo atual continua compilado e testado em todo CI, para sempre.

### 5.4 Caches, SDK e compatibilidade de mistura

* **Cache de objetos.** `cache_objeto::chave` (`EN/cache_objeto.rs:76-89`) já mistura os `args`. O modo vai
  como argumento explícito (`--df-raizes=…`, `--df-excecoes=…`). `VERSAO_FORMATO` (`:35`) sobe para 2.
* **SDK compilado.** `sdk_modulo::chave_do_sdk` (`EN/sdk_modulo.rs:380`) recebe o par de modos. A DLL
  `dfsdk_<chave>`, o bitcode ThinLTO do SDK de produção e os resumos são **por modo**. Quatro combinações
  dão até quatro SDKs em cache, e a poda do cache trata cada um como chave distinta.
* **Marcador de ABI.** Todo módulo emitido ganha `@df.abi.raizes.<modo>` e `@df.abi.excecoes.<modo>`. São
  símbolos fortes definidos uma vez por módulo e **exigidos** pelo runtime da variante certa: o runtime da
  variante `mapas` referencia `df.abi.raizes.mapas`. Uma ligação que mistura módulos de modos diferentes,
  ou que liga um módulo com o runtime errado, falha **na ligação**, com um nome de símbolo legível.
  * No JIT a mesma conferência é feita por comparação de string na carga.
  * A mistura por **função** dentro do mesmo modo (§3.7) não é mistura de ABI: o percorredor de mapas
    sempre visita também a pilha-sombra.
* **Determinismo.** O IR de cada modo é determinístico, e o resumo de determinismo do corpus ganha uma
  linha por combinação.

---

## 6. Plano de implementação

Cada etapa é pequena, mensurável, tem critério de pronto e de **abandono**, e não toca no padrão.

### Etapa 1: exceções por tabelas com as raízes de hoje (A1)

**Por que primeiro.** É a dimensão de risco menor:

* não muda a representação de `Ref`;
* não depende do RS4GC;
* ataca as 495 mil checagens e os `ret` de saída por exceção.

**Passos:**

| passo | o quê | arquivos |
|---|---|---|
| E1.0 | `ModoExcecoes` no `Context`/opções; marcador de ABI; recusa por alvo | `EN/context.rs`, `EN/lib.rs`, `crates/cli/src/nativo.rs`, `EN/cache_objeto.rs`, `EN/sdk_modulo.rs` |
| E1.1 | tabela de efeitos por extern (`lança`) e checagem só depois das que lançam; **medida isolada no modo atual** | `EN/lower/fn_builder.rs:940-992`, `EN/lower/verificador.rs`, `RT/` (o teste da marca) |
| E1.2 | `invoke`/`landingpad` dentro de `try`; `@df.lancar`; restauração do topo no pad; portas Rust→Dart | `EN/llvm/mod.rs` (terminadores e chamadas), `EN/lower/comandos.rs`, `EN/lower/fn_builder.rs`, `EN/lower/async_sm.rs`, `RT/excecoes.rs`, `RT/heap.rs` (`Contexto`), `RT/closures.rs`, `RT/isolados.rs`, `RT/portas.rs` e os demais `transmute` |
| E1.3 | personalidade Itanium (Linux) e SEH (Windows x64), LSDA compartilhada | `RT/excecoes_tabelas.rs` (novo), `RT/lib.rs` |
| E1.4 | callbacks FFI como portas | `EN/lower/ffi.rs`, `RT/ffi_callbacks.rs` |

**Pronto quando:**

* o corpus nativo inteiro passa no diferencial em A1, com e sem `--gc-stress`;
* o e2e do `new_sali/backend` dá as mesmas 39/42 rotas;
* o JIT continua em `checagem` e verde;
* as medidas do §8 estão publicadas.

**Abandonar (e manter só E1.1) se:**

* o `.text` + `.xdata`/`.eh_frame` + LSDA do backend em A1 não cair **pelo menos 3%** contra A0;
* ou o tempo de `bench/desempenho` piorar além do ruído em qualquer medida sem exceção;
* ou o lançamento-captura do micro-bench de exceções (§8.3) ficar mais de 50× mais lento que em A0, sem
  caminho para baixar.

**Riscos:**

* Exceção atravessando quadro Rust (C9). Mitigação: porta única com o `grep` no CI e o teste D9.
* Tamanho das tabelas comer o ganho. O Perry viu o `__eh_frame` cair ~105 KB com statepoints, mas isso não
  é a mesma medida (`statepoint-gc-experiment.md:1027-1042`). Medir.
* Desenrolador lento no estouro de pilha (D8).

### Etapa 2: protótipo de mapas num alvo (B0, e B1 se a Etapa 1 passou)

**O alvo é o Windows x86-64:**

* é o alvo do dono e o do `new_sali/backend`;
* é onde estão as três dificuldades que decidem a viabilidade: `lld-link` sem `--lto-newpm-passes`, COFF e
  SEH;
* o percurso pelo `RtlVirtualUnwind` é o mais simples (tabelas obrigatórias), e o Perry já o mostrou
  funcionando no x86-64 Windows (`garbage-collector.md:209-214`).

Começar pelo Linux deixaria a pergunta mais cara sem resposta.

**Passos:**

| passo | o quê | arquivos |
|---|---|---|
| E2.0 | funções de conversão do emissor (§3.2); IR idêntico no modo sombra | `EN/llvm/mod.rs`, `EN/llvm/*_ir.rs`, `EN/hir.rs` |
| E2.1 | experimentos de fato (não de produto), em `E:\dftemp\spec-mapas`: (a) `.llvm_stackmaps` sai no COFF? com que nome? (b) `llvm.fake.use` sobrevive ao O2 e entra no mapa? (c) `statepoint-max-registers-for-gc-values` = 0 no 22.1.8? (d) `landingpad token` + `invoke` de statepoint com personalidade própria no `x86_64-pc-windows-msvc` | — |
| E2.2 | emissão `addrspace(1)`, `-ni:1`, `gc "statepoint-example"`, folhas, uso fictício de argumentos | `EN/llvm/mod.rs`, `EN/llvm/externs.rs`, `EN/alvo.rs` |
| E2.3 | pipeline em processo (O0 e O2 → RS4GC → verificador → objeto), **as duas ordens medidas** (RS4GC depois × antes do O2) | `EN/gerador.rs`, `crates/llvm` |
| E2.4 | conversor `.llvm_stackmaps` → mapa v0 **sem compactação** (só o decodificado, para correção primeiro) e registro por imagem | `EN/gcmap.rs` (novo), `RT/gc_mapas.rs` (novo), `RT/gc_raizes.rs` |
| E2.5 | percorredor Windows x64 (`RtlVirtualUnwind`) + o de conferência | `RT/gc_mapas.rs` |
| E2.6 | quadro-sombra residual (§3.7) e orçamento | `EN/llvm/raizes.rs`, `EN/llvm/mod.rs` |

* Escopo: só o programa pequeno de produção e o desenvolvimento, **sem** ThinLTO distribuído.

**Pronto quando:**

* o corpus nativo passa em B0, com e sem `--gc-stress`, no Windows x64;
* os testes direcionados do §7.3 estão verdes **e** cada um falha com a sua sabotagem;
* o `bench/desempenho` está medido nas quatro combinações.

**Abandonar se:**

* um defeito do LLVM 22.1.8 sem contorno local quebrar a correção (§9);
* ou o `.text` dos programas do `bench/desempenho` em B0 não ficar **menor** que em A0, com o mapa
  incluído;
* ou o tempo de compilação no desenvolvimento passar de 1,5× o de A0.

Abandonar é **arquivar** com as medidas: o código fica atrás da feature, sem custo para o padrão.

### Etapa 3: formato compacto e o executável completo

| passo | o quê |
|---|---|
| E3.1 | ThinLTO distribuído no Windows (§3.4) com o nosso passo RS4GC; cache por parte |
| E3.2 | mapa compacto v1 (§3.5), decodificação preguiçosa, ida e volta conferida |
| E3.3 | `new_sali/backend` em B0 e B1: tamanho por seção, tempo por fase e e2e (§8) |

**Pronto quando:** o backend passa no e2e (39/42) em B0 e em B1, e as medidas estão publicadas.

**Abandonar o modo mapas como candidato a padrão** (fica experimental) se o executável B1 não for menor
que o A1 **e** o tempo do `bench/desempenho` não for melhor. Os dois juntos; o dono decide se só um vale.

### Etapa 4: os demais alvos

* Linux x86-64: `_Unwind_Backtrace`, `.llvm_stackmaps` ELF, `--lto-newpm-passes` no `ld.lld`.
* macOS arm64: desenrolador primeiro; a cadeia `x29` como otimização medida; seção `__LLVM_STACKMAPS`; o
  `ld64.lld`/`ld` do sistema.
* Linux aarch64.
* JIT (§3.8).
* Windows arm64 só com um percorredor testado.

**Estado em 2026-10-05 (escrito, não compilado nem executado).** Das exceções por tabelas, o lado
Itanium (Linux e macOS, x86-64 e aarch64) está escrito:

* `crates/runtime/src/excecoes_tabelas.rs`: a personalidade Itanium da §13.11 (`dartforge_personalidade`
  sob `cfg(unix)`, com a mesma `pouso_da_lsda`), `dartforge_lancar_desenrolamento` (o objeto
  `_Unwind_Exception` é um por thread, em `thread_local`, e não um campo do `Contexto`) e os call sites
  da LSDA também em `udata4` (0x03), para o caso do Mach-O;
* `crates/emit_native/src/llvm/mod.rs`: `@df.lancar` chama `dartforge_lancar_desenrolamento` fora do
  Windows (`EXCECOES_POR_TABELAS_ITANIUM`);
* `crates/emit_native/src/sdk_modulo.rs` (`com_uwtable`) e `lib.rs`: `uwtable` em toda função do
  programa e do SDK, para o desenrolador atravessar os quadros sem pouso;
* `crates/emit_native/src/alvo.rs`: `--excecoes=tabelas` deixa de ser recusado nesses alvos.

**Não verificado:** a codificação dos call sites no Mach-O; a ligação com o desenrolador do sistema no
macOS; e tudo o que só a execução mostra. O runtime passou a ser compilado com
`-C force-unwind-tables=yes` no Linux e no macOS. As raízes por mapas no Linux e no macOS estão escritas
desde 2026-10-05 (§14.10); a cadeia `x29` como otimização medida, o JIT e o Windows arm64 continuam fora.

Por alvo: **pronto** = corpus + `--gc-stress` + teste de conferência dos dois percorredores; **abandonar**
o alvo = ele fica em sombra/checagem.

---

## 7. Plano de testes

### 7.1 As quatro combinações no corpus inteiro

* O CI pesado (`pesado.yml`) roda `dartforge-diferencial --nativo` em A0, A1, B0 e B1, cada uma também com
  `--gc-stress`.
* Saída byte a byte igual à da VM. Qualquer diferença entre combinações é defeito, mesmo que as duas
  pareçam "certas".
* **A0 é o oráculo.** Uma falha só em B* ou em *1 é atribuída ao modo novo até prova em contrário.

### 7.2 Diferencial entre modos (mais barato que a VM)

* Um programa compilado nas quatro combinações dá a mesma saída e o mesmo número de objetos vivos depois
  de um `dartforge_gc_collect()` final. A contagem vem de um extern de teste.
* **Uma raiz a mais** (retenção) também é diferença: ela aparece como contagem maior. No modo sombra é
  esperada, por causa dos slots compartilhados guardando mortos (`raizes.rs:22-24`). Por isso a comparação
  é `B ≤ A`, não igualdade.

### 7.3 Testes direcionados aos estados perigosos

Cada teste é um programa em `corpus/nativo/gc_*` ou `crates/emit_native/tests/`. Cada um tem uma
**sabotagem** que o deixa vermelho: um modo de compilação de teste que remove uma raiz ou uma folha. Um
teste que não falha com a sabotagem não conta.

| id | estado | o programa | sabotagem |
|---|---|---|---|
| D1 | argumento vivo na chamada que coleta (C1) | `a + b` de textos recém-criados, com `--gc-stress`, em laço; `List.add` de objeto novo | tirar o uso fictício (§3.3) |
| D2 | muitos temporários e `phi` | expressões com 20+ `Ref` vivos atravessando chamadas; laço com `phi` de `Ref` e de `int`/`double` desencaixotado | slot compartilhado errado (modo sombra); folha falsa (mapas) |
| D3 | exceção profunda e topo da pilha-sombra (A1) | recursão de 1 000 níveis que lança no fundo e pega no meio, depois aloca muito | não restaurar o topo no pad |
| D4 | `finally` e `catch` que alocam e relançam | `try { … } finally { aloca; }` aninhados, com `return`/`break` no `finally` (razões 1, 5+k) | tirar a raiz da exceção guardada |
| D5 | slot lido antes de escrito | referência definida só num ramo, com chamada antes; `phi` com entrada "indefinida"; função `async` com `await` antes da definição (a armadilha `bad26cc6d9` do linzj) | emitir `undef` em vez de `null`; modo de depuração que enche o quadro com padrão par inválido |
| D6 | valor bruto tratado como referência | `double` e `int` desencaixotados (bits pares) vivos com referências através de chamadas, `--gc-stress` (a família `6293e9d4d5` do linzj) | pôr um valor bruto par no `gc-live` |
| D7 | folha falsa | contador por extern: cada folha chamada sob `--gc-stress` com **zero** coletas dentro | marcar `dartforge_string_concat` como folha |
| D8 | `StackOverflowError` | recursão infinita capturada, nas quatro combinações, depois uma alocação | reduzir a folga |
| D9 | fronteira Rust | closure Dart chamada pelo runtime (`sort` com comparador que lança, `Future.then` que lança, callback FFI que lança, `Isolate.exit` dentro de `finally`) | chamar Dart sem a porta |
| D10 | closures e `async` suspenso | closure que captura objeto e só ele o mantém, `await` no meio, coleta durante a suspensão | — (o contrato C5 vale nos dois modos) |
| D11 | derivado sem base | laço que lê campos de um objeto cuja única referência morre antes | — (só o mapa deve mostrar a base viva) |
| D12 | cache do runtime | todo cache estático do runtime com handle: conferir que é raiz (formato 5 do Perry) | tirar o registro |
| D13 | estática e constante | literais da imagem como `Ref` através de chamadas (C2, item 3) | — |
| D14 | quadro-sombra residual | `alloca` que escapa (função local direta capturando por endereço) no modo mapas | — |

### 7.4 Verificações estáticas

* **Verificador do IR do modo mapas** (no espírito do `GCInvariantVerifier` do Julia,
  `E:\references\julia\src\llvm-gc-invariant-verifier.cpp:66-220`). Roda antes do RS4GC e confere:
  * nenhum `inttoptr` para `addrspace(1)` fora das três origens permitidas (C2), marcadas por metadado do
    emissor;
  * nenhum `undef`/`poison` de tipo `ptr addrspace(1)`;
  * nenhum `ptr addrspace(1)` guardado em memória que não seja campo de objeto, `alloca` promovível ou
    quadro residual;
  * todo `call`/`invoke` folha está na tabela de efeitos.
* **Depois do RS4GC:**
  * o verificador do LLVM;
  * um conferidor de **dominância de raízes** no estilo do `--statepoints` do Perry
    (`gc-rooting-invariant.md:227-253`): nenhum `ptr addrspace(1)` usado abaixo de um statepoint sem ser o
    relocado;
  * catraca `--max-unrooted 0`;
  * as violações plantadas (`--seeded-violations`) têm de ser todas achadas.
* **No modo sombra:** o mesmo conferidor de dominância sobre o IR de hoje (o `store` no slot domina todo
  ponto de coleta em que o valor está vivo). Hoje isso não existe e vale para A0 também.
* **No conversor do mapa:** recusa de `Register`, de constante par suspeita e de registro duplicado (§3.5);
  ida e volta.

### 7.5 Instrumentos e a prova de que coletou

* **`--gc-stress`** (C8) nas quatro combinações.
* **Coleta agendada por semente** (o "zeal" do Perry, `gc-rooting-invariant.md:448-466`):
  `DARTFORGE_GC_AGENDA=<semente>,<taxa>` coleta deterministicamente em uma de cada N alocações. Serve para
  achar o defeito que o `--gc-stress` total esconde por mudar o tempo, e para reproduzir.
* **Veneno da memória livre.** O coletor não move, então a "quarentena de from-space" do Perry não se
  aplica. O equivalente é:
  * o espaço já zera os mortos no estresse (`EspacoDeObjetos::zerar_mortos`, `RT/espaco.rs:914-954`);
  * acrescentar `DARTFORGE_GC_VENENO=1`, que enche o bloco liberado com um padrão par inválido
    (`0xDFDF…DE`) e **atrasa a reutilização** por N coletas (anel de quarentena de blocos);
  * um uso depois de liberar então lê o veneno e falha na validação de handle (`validar_handles`,
    `RT/heap.rs:620-623`), em vez de ler um objeto novo por acaso;
  * o Perry achou que profundidade 4 dá falsos verdes e usa 800 (`gc-rooting-invariant.md:473-476`).
    Começar com 1 000 blocos.
* **Percurso conferido** (`DARTFORGE_GC_PERCURSO=conferir`, §3.6).
* **Prova de que coletou.** Todo teste do §7.3 lê, no fim, `dartforge_gc_estatisticas()`: coletas,
  quadros percorridos e raízes de mapa visitadas. Ele **falha** se:
  * coletas = 0;
  * no modo mapas, raízes de mapa visitadas = 0;
  * no D7, alguma folha registrou coleta.

  Isso impede o falso verde de um teste que nunca chegou a coletar no ponto perigoso.

---

## 8. Plano de medição

### 8.1 O que medir, nas quatro combinações

| eixo | medida | como |
|---|---|---|
| tamanho | arquivo; `.text`; `.pdata`+`.xdata` (COFF) ou `.eh_frame`+`.gcc_except_table` (ELF) ou `__eh_frame`+`__unwind_info`+`__gcc_except_tab` (Mach-O); `__df_gcmap` (e, durante a Etapa 2, `.llvm_stackmaps`); dados; runtime (a `staticlib` da variante) | `llvm-objdump -h`/`llvm-size`; o mapa da ligação com `tools/tamanho/quebra.py` |
| forma do código | funções com e sem quadro; `%gcvolta`/gravações/checagens restantes; statepoints, relocates, folhas; funções em quadro residual | contadores do emissor (`--timings`) e `llvm-objdump -d` |
| compilação | front-end, mundo, HIR, IR, **RS4GC**, otimização, objetos, conversão do mapa, ligação | `--timings`, frio e quente |
| desempenho | `bench/desempenho/*.dart` (chamadas, coleções, json, textos, numérico, objetos_*); micro-bench de exceção (lançar e pegar a 1, 10 e 100 quadros); micro-bench de GC (pilha profunda + muitas coletas menores) | mínimo de ≥ 10 execuções alternadas, mediana e dispersão, máquina sem build concorrente (`docs/PESQUISA-LLVM-DART-AOT.md` §7.4) |
| programa grande | `new_sali/backend`: tamanho por seção, tempo de compilação frio e quente, e2e (39/42) e tempo de algumas rotas | o procedimento de `NATIVO-PRODUCAO-GRANDE.md` §5 |
| memória | pico de RSS, número de coletas | `DARTFORGE_GC_RASTRO=1` |

### 8.2 A fórmula do saldo

Para um programa, entre uma combinação nova N e a referência A0:

```
Δtamanho = − (código de raízes eliminado: prólogo/encadeamento, gravações, voltas)
           − (checagens e saídas por exceção eliminadas)
           + (derramamentos e recargas dos statepoints)
           + (mapa compacto)
           + (tabelas de desenrolamento a mais: LSDA, .xdata/.eh_frame dos pads)
           + (portas, @df.lancar, personalidade e percorredor no runtime)
           ± (efeito no inlining e no otimizador: medido, não estimado)
```

* Cada termo tem uma medida própria (contadores e seções), e a soma tem de bater com o Δ do arquivo com
  erro ≤ 1%. Senão, há um termo esquecido.
* O tempo tem a mesma decomposição: caminho feliz mais leve, contra derramamentos, percurso mais caro na
  coleta e lançamento mais caro.

### 8.3 O que **não** é resultado esperado

* "27 MB / 2 min" do `new_sali/backend` é a **meta** do documento de produção (`NATIVO-PRODUCAO-GRANDE.md:12-13`).
  **Não** é uma previsão desta especificação, e nada aqui a demonstra.
  * As linhas de IR das §1.3 são contagens de **texto de IR**, não de bytes.
  * O único dado externo é do Perry: +1,86% de binário e −1–2% de tempo no zod; +50 KB no drizzle, porque o
    mapa comeu o ganho de `.text`; +14,6% de compilação (`PERRY/changelog.d/7366-statepoints-default.md:18-22`;
    `statepoint-gc-experiment.md:477-480`, `:1027-1042`).
  * Esses números indicam que o saldo de **tamanho** dos mapas pode ser **nulo ou negativo**. A hipótese só
    vale para nós se o código de raízes do DartForge for muito mais pesado que o do Perry, o que as 674 mil
    gravações e 396 mil voltas sugerem, mas não provam.
* Hipóteses a testar, cada uma com a sua medida:
  * H1: A1 reduz o `.text` do backend mais do que aumenta as tabelas;
  * H2: B0 reduz o `.text` mais do que custam derramamentos + mapa;
  * H3: B1 ≈ A1 + B0 (os efeitos somam);
  * H4: o tempo de compilação de B* fica ≤ 1,3× o de A0;
  * H5: o percurso pelo desenrolador não deixa nenhum `bench/desempenho` mais lento além do ruído.

---

## 9. Riscos, mitigação e issues do LLVM

### 9.1 Riscos

| risco | efeito | mitigação |
|---|---|---|
| defeito do LLVM em statepoints | raiz faltando = uso depois de liberar | o modo atual como padrão e oráculo; os testes D1–D14; o conferidor de dominância; a fixação do LLVM em 22.1.8 com testes de regressão das issues (§9.2) antes de cada troca de versão |
| RS4GC quebra com funclets | não compila, ou compila errado | **não usar funclets** (§4.2); recusa no emissor se algum aparecer |
| `lld-link` sem `--lto-newpm-passes` | sem RS4GC na LTO do Windows | ThinLTO distribuído ou LTO em processo (§3.4); o experimento E3.1 decide |
| crescimento de IR e de tempo no RS4GC | compilação lenta ou estouro de memória | orçamento por função, com fallback para quadro residual (§3.4) |
| marca de folha errada | coleta numa função sem mapa | tabela de efeitos única, o D7, o contador por extern |
| exceção atravessando quadro Rust | abort ou estado do runtime corrompido | portas, `grep` no CI, D9 |
| percurso errado (pula quadros) | raiz faltando | dois percorredores com conferência; abortar na primeira anomalia |
| valor bruto em slot de referência | marcação de lixo | verificação de forma da raiz (0, ímpar, ou handle de bloco vivo ou estático), o D6 |
| tempo do desenrolador na coleta | coletas menores mais caras | cache por PC; medir (§8) |
| mapa maior que o ganho | executável maior | a fórmula do saldo; abandono na Etapa 3 |
| JIT sem tabelas no Windows | exceções não funcionam | o JIT fica em `checagem` até um teste provar |
| divergência sutil entre modos | bug que só aparece num modo | diferencial entre modos (§7.2) |

### 9.2 Issues do LLVM

Abertas no GitHub em 2026-10-02. "No 22.1.8?" foi conferido pela comparação com a tag
`llvmorg-22.1.8`. Legenda das classes:

* **D** = defeito real do upstream;
* **U** = uso incorreto ou integração;
* **L** = limitação documentada.

| issue | o quê | estado | classe | nos afeta? | o que fazer |
|---|---|---|---|---|---|
| [#74612](https://github.com/llvm/llvm-project/issues/74612) (dup. [#156254](https://github.com/llvm/llvm-project/issues/156254)) | statepoint cuja chamada devolve struct grande pela pilha (`{i64×4}`) derruba o ISel do x86 | aberta; [PR #157251](https://github.com/llvm/llvm-project/pull/157251) não integrado | D | só se uma função `gc` devolver agregado grande | retorno de agregado só por `sret` explícito; teste de regressão com `{i64×4}` |
| [#75162](https://github.com/llvm/llvm-project/issues/75162) | tamanho de quadro do stack map errado com struct grande **por valor** em -O1+ | aberta, sem resposta | D provável | Linux/macOS x86-64. O `X86CallFrameOptimization`, hipótese não confirmada, fica desligado no Win64 ([fonte, L152-155](https://github.com/llvm/llvm-project/blob/llvmorg-22.1.8/llvm/lib/Target/X86/X86CallFrameOptimization.cpp#L152-L155)) | nenhum agregado por valor em chamada de função `gc`; teste com muitos argumentos na pilha comparando o SP medido com o do mapa; `-no-x86-call-frame-opt` como reserva |
| [#199191](https://github.com/llvm/llvm-project/issues/199191), [#189460](https://github.com/llvm/llvm-project/issues/189460) | `gc.relocate` com índice fora do `gc-live` → estouro no verificador | corrigida pelo [PR #208278](https://github.com/llvm/llvm-project/pull/208278) só no 23.x | D (IR malformada) | só se escrevermos statepoints à mão | não escrever statepoint à mão: só o RS4GC os cria |
| [#207508](https://github.com/llvm/llvm-project/issues/207508) | callee de statepoint vindo de `extractvalue` de agregado chama o elemento 0 | aberta (22.1.4, arm64); [PR #205364](https://github.com/llvm/llvm-project/pull/205364) | D, miscompilação silenciosa | se um ponteiro de função vier de agregado SSA | callee sempre de `load` ou escalar. O emissor tem 1 `extractvalue` (por `grep`): conferir |
| [#36858](https://github.com/llvm/llvm-project/issues/36858) ([graal#3185](https://github.com/oracle/graal/issues/3185)) | RS4GC não relata ponteiros GC dentro de agregados SSA ("FCA unimplemented", só um `assert`, [RS4GC.cpp L2687](https://github.com/llvm/llvm-project/blob/llvmorg-22.1.8/llvm/lib/Transforms/Scalar/RewriteStatepointsForGC.cpp#L2687)) | aberta | L | **crítico**: raiz perdida em silêncio num build sem `assert` | nenhum `ptr addrspace(1)` em agregado SSA vivo através de chamada; o verificador do §7.4 recusa |
| [#61917](https://github.com/llvm/llvm-project/issues/61917) | RS4GC: "unsupported addrspacecast" ([L502](https://github.com/llvm/llvm-project/blob/llvmorg-22.1.8/llvm/lib/Transforms/Scalar/RewriteStatepointsForGC.cpp#L502)) | aberta | L | **sim**: a primeira ideia para objetos estáticos (§3.2) era `addrspacecast` | objetos estáticos da imagem **declarados como globais `addrspace(1)`**; nenhum `addrspacecast` em valor GC; verificador recusa |
| [#80673](https://github.com/llvm/llvm-project/issues/80673), [#87625](https://github.com/llvm/llvm-project/issues/87625) | RS4GC quebra com `alloca` em `addrspace(1)` | abertas | L/U | não, se seguirmos o §3.2 | `alloca` sempre em `addrspace(0)` guardando `ptr addrspace(1)` |
| [#179878](https://github.com/llvm/llvm-project/issues/179878) | `cast<InvokeInst>` falha no RS4GC | aberta (2026) | D | padrão não confirmado | coberto pelos testes de `invoke` (D3, D4) |
| [#35450](https://github.com/llvm/llvm-project/issues/35450) | RS4GC transforma `musttail` em statepoint (módulo inválido) | aberta | D | não: o emissor não gera `musttail` (por `grep`) | proibir `musttail` em função `gc` |
| [#56158](https://github.com/llvm/llvm-project/issues/56158), [#39891](https://github.com/llvm/llvm-project/issues/39891) | `gc.relocate` com FastISel / -O0 | abertas | D | **sim**, no perfil de desenvolvimento (`-O0`) | funções `gc` geradas sem FastISel (`-fast-isel=false`) ou em `-O1` para o código; teste do corpus em `-O0` no modo mapas |
| [#222894](https://github.com/llvm/llvm-project/pull/222894) (PR) | relocate de constante/`alloca` no *landing pad* de um `invoke` gera vreg que não domina o uso | integrado só no `main` (24.x) | D | se houver constantes no `gc-live` com `invoke` | o RS4GC não põe constantes no `gc-live`; testar o B1 com `-verify-machineinstrs` |
| [#75074](https://github.com/llvm/llvm-project/issues/75074), [#55622](https://github.com/llvm/llvm-project/issues/55622) | `.llvm_stackmaps` com relocação absoluta não liga em PIE | abertas | L | **sim** no Linux (PIE) | a seção do LLVM é **removida** depois da conversão (§3.5); o mapa compacto é relativo |
| [#37625](https://github.com/llvm/llvm-project/issues/37625) | o formato supõe uma unidade de compilação, e o linker concatena blobs | aberta | L | sim, com vários objetos | converter **por objeto** (§3.5), antes da ligação |
| [#117757](https://github.com/llvm/llvm-project/issues/117757) | no arm64, `x19` como *base pointer* com `alloca` dinâmica | fechada, não é bug | U | arm64 | `alloca` só no bloco de entrada (já é regra, `EN/llvm/mod.rs:638-645`); o percorredor aceita `x19` |
| [#55649](https://github.com/llvm/llvm-project/issues/55649) | entrada de stackmap errada para struct maior que um registrador | aberta | D | como #36858 | idem |
| [#61264](https://github.com/llvm/llvm-project/issues/61264) | `gc_transition` não selecionável no AArch64 | aberta | L | não | `flags = 0` sempre (o RS4GC já usa) |
| [#186229](https://github.com/llvm/llvm-project/issues/186229) | valores fora de registrador (globais, constantes) somem do `gc-live` | aberta | L | não: objetos estáticos são imortais | — |
| [#49897](https://github.com/llvm/llvm-project/issues/49897), [#142314](https://github.com/llvm/llvm-project/issues/142314), [#80294](https://github.com/llvm/llvm-project/issues/80294), [#56267](https://github.com/llvm/llvm-project/issues/56267), [#56493](https://github.com/llvm/llvm-project/issues/56493), [#55308](https://github.com/llvm/llvm-project/issues/55308), PRs [#85908](https://github.com/llvm/llvm-project/pull/85908), [#68439](https://github.com/llvm/llvm-project/pull/68439), [#97280](https://github.com/llvm/llvm-project/pull/97280), [#75826](https://github.com/llvm/llvm-project/pull/75826), [#69795](https://github.com/llvm/llvm-project/pull/69795), [#119682](https://github.com/llvm/llvm-project/pull/119682) | LR em volta de statepoint no arm64, `TargetFrameIndex`, relocate de `undef`, id truncado, atributos do `gc.result` e outros | corrigidos e presentes no 22.1.8 | D | não | ficam na bateria de regressão ao trocar de versão |
| Perry #7354 (sem número upstream; "eight-line repro" em `PERRY/changelog.d/7355-windows-gc-walker.md:14-16`) | RS4GC com `catchswitch`/`catchpad` → violação de acesso no `opt` 22.1.3, ainda no 22.1.8 (`PERRY/crates/perry-codegen/src/stmt/try_stmt.rs:38-45`) | — | D | **sim** no Windows, se usássemos funclets | não usar funclets (§4.2). **Reproduzido aqui:** `opt -passes=rewrite-statepoints-for-gc` 22.1.8 sobre `t3.ll`, `t4.ll` e `t5.ll` (funções `gc` com `personality @__CxxFrameHandler3` e funclets) termina com `Exception Code: 0xC0000005` dentro do passe (`E:\dftemp\spec-mapas\out_run.txt`) |
| (sem issue; verificado aqui, `t2.ll`) | `landingpad { ptr, i32 }` + `invoke` numa função `gc`: o RS4GC gera `gc.relocate` com o pad como token e o verificador recusa ("Call parameter type does not match function signature!") | — | L | sim, no B1 | *landing pad* do tipo `token` no modo mapas (§4.4). Com `landingpad token`, `t2b.ll` passa |
| Perry #9499 | slot de spill reaproveitado (`findPreviousSpillSlot`) e InstCombine dobrando `ptrtoint`/`inttoptr` entregava outro objeto vivo como argumento (`PERRY/changelog.d/9499-root-reload-relocate-identity.md:13-38`) | — | D/U | **sim**: reproduzia **sem** mover objetos | não fazer ida e volta `ptrtoint`→`inttoptr` de referência (C2); teste D6 |
| [Discourse 59225](https://discourse.llvm.org/t/statepoint-gc-query-about-non-relocating-statepoints-and-instcombine/59225) | o InstCombine tira do `gc-live` a entrada cujo `gc.relocate` não é usado (D85959); recomendação de Reames: sempre usar o relocado, mesmo com coletor não móvel | — | U | **sim**, se alguém trocar o relocado pelo original "porque não move" | nunca reescrever os usos depois do RS4GC; o conferidor de dominância (§7.4) recusa uso do original abaixo do statepoint |

**Leituras gerais:**

* "Experimental" no nome dos intrínsecos quer dizer que não há garantia de compatibilidade entre versões
  ([Statepoints](https://llvm.org/docs/Statepoints.html)). A RFC de 2025 contra o prefixo não os renomeia
  ([Discourse 85352](https://discourse.llvm.org/t/rfc-dont-use-llvm-experimental-intrinsics/85352)).
  Consequência: **toda troca de versão do LLVM** passa pela bateria de regressão destas issues e pelo
  corpus em B0/B1 antes de ser aceita.
* Usuários em produção conhecidos:
  * Azul Falcon: "2+ person years" de desenho
    ([keynote 2017](https://llvm.org/devmtg/2017-10/slides/Reames-FalconKeynote.pdf), lida só pelo resumo);
  * GraalVM Native Image com backend LLVM, só x86-64 com AArch64 "being pushed"
    ([doc 22.0](https://www.graalvm.org/22.0/reference-manual/native-image/LLVMBackend/index.html));
  * LLILC, arquivado ([llilc-gc.md](https://github.com/dotnet/llilc/blob/main/Documentation/llilc-gc.md));
  * Perry.

  Nenhum deles com coletor não móvel e valores etiquetados no Windows: o nosso caso não tem precedente
  publicado.
* Em 2019 houve uma proposta de `llvm.experimental.gc.inttoptr/ptrtoint` justamente para as etiquetas do
  Dart ([Discourse 53228](https://discourse.llvm.org/t/proposal-for-llvm-experimental-gc-intrinsics-for-inttoptr-and-ptrtoint/53228)).
  Não há registro de que tenha entrado. O RS4GC trata `inttoptr` como base conhecida
  ([L485-494](https://github.com/llvm/llvm-project/blob/llvmorg-22.1.8/llvm/lib/Transforms/Scalar/RewriteStatepointsForGC.cpp#L485-L494)),
  o que é o que queremos para `Smi` e para o handle devolvido pelo runtime (C2).

---

## 10. Referências

* LLVM: `Statepoints.rst`, `StackMaps.rst`, `GarbageCollection.rst`, `ExceptionHandling.rst` (llvm.org/docs).
* Perry: `PERRY/docs/statepoint-gc-experiment.md`, `PERRY/docs/src/internals/garbage-collector.md`,
  `PERRY/docs/src/internals/gc-rooting-invariant.md`, `PERRY/docs/invoke-eh-experiment.md`,
  `PERRY/crates/perry-codegen/src/gc_map.rs`, `PERRY/crates/perry-runtime/src/gc/roots/stack_maps.rs`,
  `PERRY/crates/perry-runtime/src/eh_windows.rs`.
* linzj: `E:\references\linzj-llvm-project\NOTAS.md`, `COMMITS.md`; commits `bad26cc6d9`, `6293e9d4d5`,
  `e2ba01eea6`, `6a4d1a4bb5`, `29c70482e2`, `e1c7266797` em `github.com/linzj/llvm-project`.
* Julia: `E:\references\julia\src\llvm-late-gc-lowering.cpp`, `llvm-final-gc-lowering.cpp`,
  `llvm-gc-invariant-verifier.cpp`, `doc/src/devdocs/llvm.md`.
* DartForge: `docs/NATIVO-PLANO.md` §1 e §6.5, `docs/PESQUISA-LLVM-DART-AOT.md`,
  `docs/NATIVO-PRODUCAO-GRANDE.md`, `docs/NATIVO-ESPACO-UNIFICADO.md`, `docs/JIT.md`.

## 11. O que não foi confirmado (pendências)

Os experimentos citados estão em `E:\dftemp\spec-mapas`:

* os casos `t1`–`t11`, executados com `run.sh`, `run2.sh` e `run3.sh`, com as saídas em `out\` e
  `out_run*.txt`;
* o experimento do ThinLTO distribuído, em `dtlto\`.

As citações de `Statepoints.rst`, `StackMaps.cpp`, `RewriteStatepointsForGC.cpp`, `LLJIT.cpp` e afins com
número de linha são da tag `llvmorg-22.1.8`. Elas foram baixadas para `E:\dftemp\spec-mapas\src\` porque
`E:\references` não tem fonte moderno do LLVM: o `linzj-llvm-project` só tem `NOTAS.md`/`COMMITS.md`.

**Por experimento (entram na E2.1/E3.1):**

1. Se o RS4GC **sempre** põe no `gc-live` o argumento `ptr addrspace(1)` de uma chamada não folha (§3.3).
   Hoje há um caso observado. Daí depende o C1 sair de graça ou precisar do uso fictício.
2. Se o `llvm.fake.use` sobrevive ao O2 e entra no mapa.
3. Global em `addrspace(1)` para os objetos estáticos da imagem, na seção `.dfimg$m`.
4. A fórmula SP/CFA por alvo para os locais `Indirect [SP+off]`: a validar por teste, nunca copiando a do
   Perry.
5. A personalidade de ponta a ponta. Nenhum experimento executou um lançamento real:
   * `RaiseException` + `RtlUnwindEx` até o *landing pad* no `-msvc`;
   * `_Unwind_RaiseException` no Linux e no macOS.

   O Perry afirma que funciona (`eh_windows.rs`); aqui não foi rodado.
6. O ThinLTO distribuído na escala das 77 partes do backend, com cache, e pelo `/thinlto-distributor:`.
7. `--lto-newpm-passes='lto<O2>,rewrite-statepoints-for-gc'` no `ld.lld`/`ld64.lld` (o nome da pipeline).
8. Statepoint **dentro** de funclet. Não é usado pelo desenho, mas não foi testado.
9. O RS4GC num build com `assert` ligado (o oficial não tem).

**Por leitura (fonte ou documentação):**

10. Se o `LLJIT`/RTDyld registra o `.pdata` no Windows. A inferência é que não.
11. Um plugin JITLink para stack maps.
12. O comportamento do `link.exe` da Microsoft e do `ld64` do sistema com as seções e relocações.
13. A regra do inliner do LLVM 22 para estratégias `gc` diferentes (§3.7).
14. A afirmação de que um desenrolar estrangeiro entrando num quadro Rust `C-unwind` aborta sob
    `panic=abort` (C9). O desenho não depende dela: as portas evitam qualquer quadro Rust no caminho.
15. Os conteúdos dos PDFs do Azul Falcon (lidos só pelo resumo da busca) e o padrão de IR da
    [#179878](https://github.com/llvm/llvm-project/issues/179878).
16. A relação entre as linhas de IR da §1.3 e os bytes de `.text`. Nunca foi medida, e é a primeira medida
    das Etapas 1 e 2.

**Atualização de 2026-10-04.** Os itens 1, 2, 3, 4, 5 (no Windows) e 10 (no `lli`) foram resolvidos por
experimento; a tabela está no §12.2. Continuam pendentes: 5 no Linux e no macOS, 6, 7, 8, 9, 11 a 16, e
os dois experimentos não concluídos (`x8`, ELF; `x10`, ThinLTO).

**Seções pendentes de escrita:** nenhuma. O documento está completo como especificação. A §3.9 (colocação
tarde, como no Julia) está registrada como alternativa, fora do escopo.

---

## 12. Experimentos da rodada 2 (2026-10-04): o que passou a estar verificado

Os casos ficam em `E:\dftemp\spec-mapas\r2\` (`x1`…`x16`, `efeitos\`, `ir-hoje\`). Todos usam o
LLVM/Clang 22.1.8 de `E:\llvm` e rodam no Windows x86-64, salvo indicação. As saídas abaixo são as de uma
reexecução dos binários em 2026-10-04; "verificado" passa a incluir estes casos.

### 12.1 Resumo

| caso | o que testa | resultado |
|---|---|---|
| `x1` | personalidade própria com LSDA Itanium sob SEH: `RaiseException` em IR, busca, `RtlUnwindEx` até o *landing pad*; `try` simples, aninhado com relançamento, `finally` que relança, exceção estrangeira | **passou**: `pega(0,5)=1102`, `pega(1000,7)=1126`, `aninhado(3)=2078`, `com_finally(2)=177`; a violação de acesso (`0xC0000005`) atravessa o quadro com a nossa personalidade e chega ao filtro de topo |
| `x2` | o que o `llc` emite como LSDA em cinco alvos | COFF x64 e arm64: `.xdata` + `.pdata` (a LSDA vai no `.seh_handlerdata`, dentro do `.xdata`); ELF: `.gcc_except_table` + `.eh_frame`; Mach-O arm64: `__gcc_except_tab`, `__compact_unwind`, `__eh_frame` |
| `x3` | rótulo do registro do stack map em relação à chamada | o registro aponta o rótulo logo depois do `callq` (`.Ltmp0-concat`) |
| `x4` | o RS4GC mantém no `gc-live` o argumento que morre na chamada? | **sim**, nos três casos, com e sem O2 antes (§12.3) |
| `x7` | B1 de ponta a ponta: `gc "statepoint-example"`, `O2 → RS4GC → llc`, coletor de brinquedo lendo `.llvm_stackmaps` da imagem, percurso por `RtlVirtualUnwind`, exceção atravessando quadros com raízes | **passou** com coleta em toda alocação: 351 coletas, 3.681 quadros percorridos, nenhum uso depois de liberar; a sabotagem é detectada |
| `x8` | o mesmo do `x7` em ELF x86-64 | **não concluído**: o binário foi ligado, mas só há a primeira linha da saída registrada |
| `x9` | `Smi`, null, objeto estático em `addrspace(1)` e campo por GEP sob `-ni:1`, com O2 e RS4GC | o `opt` aceita e o `llc` gera objeto; os `gc-live` só trazem valores `ptr addrspace(1)` (§12.4) |
| `x10` | ThinLTO com statepoints no `lld-link` | **não concluído**: os objetos `full`/`thin` existem, o executável não foi produzido |
| `x12` | conversor `.llvm_stackmaps` → mapa compacto no **objeto COFF**, e o runtime lendo o compacto | **passou**: 1.280 → 104 bytes (98,5 → 8,0 bytes por registro), ida e volta conferida, mesmas contagens do `x7` |
| `x13` | pilha consumida pelo lançamento por SEH | 6.384 bytes até a primeira chamada da personalidade; com 32 KiB livres o lançamento completa, com 16 KiB o processo morre (§13.7) |
| `x14` | o `lli` (LLJIT) registra o `.pdata` do código JIT? | **sim**: `RtlLookupFunctionEntry` acha a função JIT e o lançamento é pego (código de saída 42) |
| `x16` | o lowering A1 inteiro das funções de `ir-hoje\exc.dart`, com pilha-sombra, porta Rust→Dart, `finally` com `break`/`continue`/exceção em laço | **passou** em O0 e O2, saída igual à esperada; a sabotagem (pouso sem restaurar o topo) é detectada |
| `efeitos\` | classificação das externs do runtime em coleta/lança/roda Dart, por fecho transitivo do grafo de chamadas | 722 externs classificadas; 113 divergências contra a tabela do emissor, todas do lado conservador (§13.8) |

### 12.2 Itens do §11 resolvidos

| item do §11 | estado | evidência |
|---|---|---|
| 1 — argumento de chamada não folha entra no `gc-live` | **confirmado** em três formas | `x4` |
| 2 — `llvm.fake.use` sobrevive ao O2 | confirmado que não atrapalha; **deixou de ser necessário** para o C1 | `x4`, função `concat_fake` |
| 3 — global em `addrspace(1)` na seção `.dfimg$m` | **confirmado** (compila e liga como constante `linkonce_odr` com `comdat`) | `x9` |
| 4 — fórmula SP para os locais `Indirect [SP+off]` no Windows x64 | **confirmado**: o SP é o `Rsp` do contexto **depois** de `RtlVirtualUnwind` desenrolar o quadro chamado, isto é, o SP do quadro Dart no ponto da chamada | `x7`, `x12` |
| 5 — personalidade de ponta a ponta no `-msvc` | **confirmado** | `x1`, `x16` |
| 5 — idem no Linux e no macOS | continua pendente | — |
| 6 — ThinLTO distribuído na escala do backend | pendente | `x10` não concluído |
| 7 — `--lto-newpm-passes` no `ld.lld` | pendente | — |
| 10 — o LLJIT registra o `.pdata` no Windows | **confirmado para o `lli` 22.1.8**; a configuração do nosso `crates/jit` precisa do mesmo teste | `x14` |
| 16 — relação linhas de IR × bytes de `.text` | pendente (primeira medida da Etapa 1) | — |

Os itens 8, 9 e 11 a 15 continuam como estavam.

### 12.3 `x4`: o argumento vivo durante a chamada (contrato C1)

Entrada (`x4\args.ll`), com `gc "statepoint-example"` e `-ni:1`:

```llvm
define ptr addrspace(1) @concat(ptr addrspace(1) %a, ptr addrspace(1) %b) gc "statepoint-example" {
  %r = call ptr addrspace(1) @rt_concat(ptr addrspace(1) %a, ptr addrspace(1) %b)
  ret ptr addrspace(1) %r
}
```

Depois de `opt -passes=rewrite-statepoints-for-gc` e também de `default<O2>,rewrite-statepoints-for-gc`
(`args.rs.ll`, `args.o2rs.ll`), o statepoint de `@rt_concat` traz
`"gc-live"(ptr addrspace(1) %a, ptr addrspace(1) %b)` e os dois `gc.relocate`, embora `%a` e `%b` não
tenham uso depois da chamada. O mesmo vale para `arg_morto` (a segunda chamada mantém `%b.relocated`) e
para `concat_fake`.

Consequência para o §2 (C1) e o §3.3: **o argumento de uma chamada que é statepoint entra no mapa dessa
chamada**. A "manutenção explícita" do §3.3 fica reduzida a uma conferência: o verificador do §7.4 exige
que todo operando `ptr addrspace(1)` de uma chamada não folha apareça no `gc-live` dela. São três formas
observadas, não uma prova para todo IR; a conferência é o que sustenta o contrato.

### 12.4 `x9`: `Smi`, null e objeto estático

O emissor pode manter as três construções de hoje, com o tipo trocado:

* `Smi`: `inttoptr i64 ((v << 1) | 1) to ptr addrspace(1)`; teste e extração por `ptrtoint`;
* null: `ptr addrspace(1) null`;
* objeto estático: `getelementptr (i8, ptr addrspace(1) @"df.s.abc", i64 2)`, com o global declarado
  `addrspace(1)` na seção `.dfimg$m`;
* campo `k`: `getelementptr i8, ptr addrspace(1) %h, i64 (14 + 8k)`.

Com `default<O2>,rewrite-statepoints-for-gc` o módulo passa no verificador e gera objeto. O conversor do
mapa (`x12\gcmap.py`) descarta os locais `Constant`/`ConstIndex` (null, `Smi` constante). Um `Smi`
**variável** vivo através de uma chamada entra no mapa como qualquer `ptr addrspace(1)`; o runtime o
descarta na marcação pela regra do C2 (ímpar). Isso custa um slot, não a correção.

Não verificado: o que o O2 faz com `ptrtoint`/`inttoptr` em laços grandes (o `-ni:1` proíbe as
transformações que criariam ponteiros de inteiros; o caso observado é pequeno).

---

## 13. Etapa 1 em nível de implementação: exceções por tabelas com a pilha-sombra (A1)

Esta seção detalha o §4 e o §6 (Etapa 1) até o ponto de implementar sem reler os experimentos. O
desenho é o do `x16`, que roda.

### 13.1 A ideia em uma frase

O desenrolador nativo só **transfere o controle**. O valor da exceção, o rastro, o teste `on T`, o
`capturavel`, o `finally` e o discriminador de razão continuam exatamente onde estão hoje: na pendência
do runtime e em código Dart comum no bloco de destino. O par `{ptr, i32}` do *landing pad* é ignorado.

### 13.2 Regra por chamada

Para cada instrução da HIR que hoje passa por `emit_call_with_check`
(`EN/lower/fn_builder.rs:940-992`):

| quem é chamado | onde está a chamada | hoje (`checagem`) | em `tabelas` |
|---|---|---|---|
| função Dart | dentro de `try` (há `exception_targets`) | `call` + leitura da pendência + desvio ao alvo | `invoke … unwind label %<bloco>.lp`; sem leitura da pendência |
| função Dart | dentro de `finally_scopes`, sem `catch` local | `call` + pendência + desvio à entrada do `finally` com razão 2 | `invoke`; o pouso desvia à entrada do `finally` com razão 2 |
| função Dart | fora de qualquer `try` | `call` + pendência + `ret` do valor padrão | `call` puro |
| extern do runtime que **lança** (§13.8) | dentro de `try` | `call` + pendência + desvio | igual a hoje (a extern não desenrola) |
| extern do runtime que **lança** | fora de `try` | `call` + pendência + `ret` padrão | `call` + pendência + `call void @df.lancar()` + `unreachable` |
| extern do runtime que **não lança** | qualquer | `call` + pendência (hoje é conservador) | `call` puro |
| `throw e` com tratador na mesma função | — | `dartforge_exception_throw` + desvio | igual a hoje |
| `throw e` sem tratador local | — | `dartforge_exception_throw` + `ret` padrão | `dartforge_exception_throw` + `call void @df.lancar()` + `unreachable` |
| `rethrow` | dentro de `catch` | devolve a exceção guardada à pendência + desvio ou `ret` | idem, com `@df.lancar` quando não há tratador externo na função |

A função que contém pelo menos um `invoke` ganha `personality ptr @dartforge_personalidade`. As demais
não ganham nada: não têm *landing pad*, nem LSDA, nem limpeza.

### 13.3 O pouso

Cada `invoke` tem o **seu** bloco de pouso (o `landingpad` tem de ser a primeira instrução não-φ do
bloco e só pode ser alcançado por arestas de desenrolamento):

```llvm
<bloco>.lp:
  %lpN = landingpad { ptr, i32 } catch ptr null
  store ptr <topo desta função>, ptr %ctxtopo, align 8
  br label %<alvo de exceção da HIR>
```

* `catch ptr null` é "pega tudo". No modo mapas o tipo é `token` e a cláusula é `cleanup` (§4.4; é a
  forma que o `x7` usa).
* `<topo desta função>` é `%gcq` quando a função tem quadro de raízes, ou o `%topo0` lido na entrada
  quando não tem (caso `df.aninhado` do `x16`). Uma função com `try` e sem quadro passa a ler o topo no
  prólogo só para isso.
* `<alvo de exceção da HIR>` é o mesmo bloco para onde a checagem de hoje desviava: o despacho do
  `catch` ou a entrada do `finally`. Quando o alvo é a entrada do `finally`, o pouso entra nos φ de razão
  e de valor de retorno com `(2, valor padrão)`, como a aresta `fin.incoming.push((curr_b, 2, default_ret))`
  de hoje (`fn_builder.rs:968-970`).

A sabotagem do `x16` (retirar o `store` do topo dos pousos e da porta) faz a coleta seguinte achar um
quadro de raízes abaixo do SP e abortar. É o teste D3 do §7.3.

### 13.4 Antes e depois: `try`/`on T catch (e, s)`

Dart:

```dart
int converte(String s) {
  try { return int.parse(s); }
  on FormatException catch (e, s2) { print(e); print(s2); return -1; }
}
```

**Hoje** (`ir-hoje\exc-agente-memoria-release.ll`, função `@df.exc$2edart..converte`; o prólogo do quadro
de raízes é igual nos dois modos e foi omitido):

```llvm
  %v4 = call i64 @df.dart$3acore.int.parse(i64 %v0, i64 0, i64 0)
  %v5 = load i8, ptr %ctx, align 8            ; pendência
  %c0 = zext i8 %v5 to i64
  %v6 = icmp ne i64 %c0, 0
  br i1 %v6, label %b3, label %b4
b3:                                           ; despacho do catch
  %v8 = call i64 @dartforge_exception_peek_ref()
  store i64 %v8, ptr %gcs0
  %v9 = call i8 @dartforge_exception_capturavel()
  ...
b4:                                           ; caminho normal
  %xcp7 = load i8, ptr %ctx, align 8          ; segunda leitura da pendência antes do ret
  %xcn7 = icmp ne i8 %xcp7, 0
  %xce7 = call i1 @llvm.expect.i1(i1 %xcn7, i1 false)
  br i1 %xce7, label %xc7.limpar, label %xc7.fim
xc7.limpar:
  call void @dartforge_exception_clear()
  br label %xc7.fim
xc7.fim:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 %v4
b8:                                           ; corpo do catch
  ...
  call void @df.dart$3acore..print(i64 %v8)
  %v26 = load i8, ptr %ctx, align 8           ; checagem depois de CADA chamada do corpo
  %c5 = zext i8 %v26 to i64
  %v27 = icmp ne i64 %c5, 0
  br i1 %v27, label %b11, label %b10
b11:                                          ; saída por exceção: ret do valor padrão
  %gcvolta11 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta11, ptr %ctxtopo, align 8
  ret i64 0
```

**Em `tabelas`** (`x16\a1.ll`, função `@df.converte`):

```llvm
define i64 @df.converte(i64 %v0) personality ptr @dartforge_personalidade {
b0:
  ; prólogo do quadro de raízes: igual
  store i64 %v0, ptr %gcs0
  %v4 = invoke i64 @df.int.parse(i64 %v0) to label %b4 unwind label %b0.lp
b0.lp:
  %lp0 = landingpad { ptr, i32 } catch ptr null
  store ptr %gcq, ptr %ctxtopo, align 8
  br label %b3
b3:                                           ; despacho do catch: idêntico ao de hoje
  %v8 = call i64 @dartforge_exception_peek_ref()
  store i64 %v8, ptr %gcs0
  %v9 = call i8 @dartforge_exception_capturavel()
  %v10 = icmp ne i8 %v9, 0
  br i1 %v10, label %b6, label %b9
b4:                                           ; caminho normal: sem leitura da pendência
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 %v4
b6:
  %v17 = call i1 @rt_e_format(i64 %v8)        ; o teste `on T` (hoje df.classe + df.subclasse)
  br i1 %v17, label %b8, label %b9
b8:                                           ; corpo do catch (e, s2)
  %v18 = call i64 @dartforge_stack_trace_get()
  store i64 %v18, ptr %gcs1
  call void @dartforge_exception_clear()
  call void @df.print_txt(i64 %v8)            ; fora de try: call puro
  call void @df.print_txt(i64 %v18)
  %gcvolta12 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta12, ptr %ctxtopo, align 8
  ret i64 -1
b9:                                           ; nenhuma cláusula casou, ou não capturável: relança
  call void @df.lancar()
  unreachable
}
```

O que some: as leituras da pendência depois de cada chamada Dart, as segundas leituras antes de cada
`ret` (`xcp`/`xcn`/`xce`), os blocos `xc*.limpar` e as saídas por exceção com `ret` do valor padrão
(`b11`, `b13` do IR de hoje). O que aparece: um bloco de pouso por `invoke` e a LSDA.

Note em `b9`: a saída "ninguém pegou" **não** desempilha o quadro de raízes antes de `@df.lancar`. Quem
pegar restaura o próprio topo.

### 13.5 `finally`, `break`/`continue` e laço

O `finally` continua uma sub-rotina com discriminador de razão (`EN/lower/comandos.rs:740-849`). A única
mudança é a origem da razão 2. Caso do `x16\a1b.ll`:

```dart
int laco(int n, int k) { var soma = 0;
  for (var i = 0; i < n; i++) {
    try { if (i == 3) break; if (i == 1) continue; soma += fora(i * k); }
    finally { soma += 100; }
  }
  return soma; }
```

```llvm
t2:
  %a = mul i64 %i, %k
  %f = invoke i64 @df.fora(i64 %a) to label %t3 unwind label %t2.lp
t2.lp:                                           ; exceção: razão 2
  %lp = landingpad { ptr, i32 } catch ptr null
  store ptr %gcq, ptr %ctxtopo, align 8
  br label %fin
fin:                                             ; entrada do finally
  %razao = phi i64 [ 5, %sai_break ], [ 6, %sai_cont ], [ 2, %t2.lp ], [ 0, %t3 ]
  %soma2 = phi i64 [ %soma, %sai_break ], [ %soma, %sai_cont ], [ %soma, %t2.lp ], [ %soma1, %t3 ]
  %g = call i64 @dartforge_exception_peek_ref()  ; guardada (0 se a razão não é 2)
  store i64 %g, ptr %gcs0
  call void @dartforge_exception_clear()
  %cap = call i8 @dartforge_exception_capturavel()
  %ncap = icmp eq i8 %cap, 0
  br i1 %ncap, label %desenrola, label %corpo_fin
corpo_fin:
  ; corpo do finally
  switch i64 %razao, label %inc [ i64 0, label %inc
                                  i64 2, label %relanca
                                  i64 5, label %fim_break
                                  i64 6, label %inc ]
desenrola:                                       ; desenrolar de isolado: o corpo do finally não roda
  call void @df.lancar()
  unreachable
relanca:                                         ; devolve a exceção à pendência e relança
  call void @dartforge_exception_throw(i64 %g, i8 3)
  call void @df.lancar()
  unreachable
```

Regras que saem do exemplo:

1. Um valor Dart vivo no pouso tem de ser o valor **antes** do `invoke` (`%soma`, não `%soma1`): o φ da
   entrada do `finally` recebe do pouso o valor que dominava a chamada. É o que o LLVM exige de qualquer
   φ com aresta de `invoke`, e o emissor já tem esse valor: é o mesmo que a aresta da checagem levava.
2. A exceção guardada pelo `finally` fica num slot do quadro de raízes (`%gcs0`), como hoje.
3. `relanca` e `desenrola` terminam em `@df.lancar`. Se o `finally` está dentro de outro `try` da mesma
   função, o `call` vira `invoke void @df.lancar() to label %nunca unwind label %<externo>.lp`, com
   `%nunca: unreachable` (caso `aninhado` do `x1`). Alternativa equivalente e mais barata, permitida:
   desvio direto ao alvo externo, como hoje, já que a pendência está ligada. A regra do §4.1
   ("`throw`/`rethrow` dentro de `try` na mesma função continuam como hoje") escolhe o desvio direto.
4. O resultado medido: `laco(5,2)=406`, `aninhado(5,6)=-7`, `aninhado(5,2)=406`, iguais em O0 e O2.

### 13.6 `@df.lancar` e a porta

Gerados em IR, uma vez por módulo de ligação (`linkonce_odr`), nunca em Rust (C9):

```llvm
; Windows
declare dllimport void @RaiseException(i32, i32, i32, ptr)
define linkonce_odr void @df.lancar() noreturn noinline {
  call void @RaiseException(i32 -532396462, i32 1, i32 0, ptr null)   ; 0xE0444652, EXCEPTION_NONCONTINUABLE
  unreachable
}
```

`noinline` importa: o endereço de retorno dentro da função que lança é o que a LSDA dela cobre, e uma
só cópia do `RaiseException` mantém o `.text` menor.

No Linux e no macOS (não executado aqui; desenho do §4.3):

```llvm
declare i32 @_Unwind_RaiseException(ptr)
declare ptr @dartforge_objeto_de_desenrolamento()        ; campo do Contexto, classe "DARTFRGE"
declare void @dartforge_desenrolamento_sem_tratador() noreturn
define linkonce_odr void @df.lancar() noreturn noinline {
  %o = call ptr @dartforge_objeto_de_desenrolamento()
  %r = call i32 @_Unwind_RaiseException(ptr %o)
  call void @dartforge_desenrolamento_sem_tratador()   ; só chega aqui com _URC_END_OF_STACK: defeito
  unreachable
}
```

A **porta**, uma por assinatura usada pelo runtime (`x16`, `@df.porta.r0` para `fn() -> i64`):

```llvm
define i64 @df.porta.r0(ptr %f) noinline personality ptr @dartforge_personalidade {
entrada:
  %ctx = call ptr @dartforge_contexto()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %topo0 = load ptr, ptr %ctxtopo, align 8
  %r = invoke i64 %f() to label %ok unwind label %lp
ok:
  ret i64 %r
lp:
  %e = landingpad { ptr, i32 } catch ptr null
  store ptr %topo0, ptr %ctxtopo, align 8        ; desempilha de uma vez os quadros pulados
  ret i64 0                                      ; a pendência continua ligada: o runtime a confere
}
```

No `x16`, o `main` em C chama `df_porta_r0(dart_main)`; `dart_main` deixa escapar a segunda exceção, a
porta a converte em pendência, e o runtime imprime `Unhandled exception:` e sai com 255. O topo da
pilha-sombra termina nulo.

### 13.7 Pilha: a margem do lançamento (`x13`)

O lançamento por SEH roda **na pilha da thread que lança**. Medido no Windows x64 com quadros de 1 KiB:

| pilha livre no lançamento | resultado |
|---|---|
| 64 KiB | pego; a personalidade rodou 6.384 bytes abaixo do SP do lançamento |
| 32 KiB | pego |
| 16 KiB, 12, 8, 4 | o processo termina sem chegar ao tratador |

Consequências:

* A folga do `StackOverflowError` é de 256 KiB hoje (`RT/gc_raizes.rs:39`). Ela cobre o lançamento com
  sobra; **não reduzir abaixo de 64 KiB** sem repetir o `x13`.
* A medida é de um caso (916 a 947 quadros de 1 KiB, sem quadros de sistema no meio). O custo do
  `RtlUnwindEx` cresce com a profundidade em tempo (§13.11), não em pilha: o mesmo teste com 32 KiB
  livres atravessou 947 quadros.
* O teste D8 do §7.3 vira: recursão até o `StackOverflowError`, pego por um `try` no topo, mil vezes,
  com e sem `--gc-stress`.

### 13.8 A tabela de efeitos das externs

Levantamento por script (`r2\efeitos\efeitos.py`): grafo de chamadas das funções do runtime fora de
testes (2.137 funções, 7.466 arestas), sementes marcadas à mão, fecho transitivo. Métodos são resolvidos
só pelo **nome**; onde um nome tem mais de um tipo dono, vale a união (lista em `resumo.txt`).

| classe | externs citadas pelo emissor |
|---|---:|
| folha pura (não coleta, não lança, não roda Dart) | 285 |
| só coleta | 169 |
| coleta e lança | 18 |
| roda código Dart do SDK (ajudantes registrados) | 215 |
| roda código Dart do usuário | 7 |

São 722 externs definidas, 694 delas citadas pelo emissor. Nenhuma extern só lança sem coletar: lançar
aloca o erro.

Contra a tabela que o emissor usa hoje, o script achou 113 linhas de divergência
(`efeitos\divergencias.tsv`): 48 em que o emissor supõe coleta e a análise não acha, 65 em que supõe
lançamento e a análise não acha. Todas do lado conservador; nenhuma em que o emissor suponha menos que a
análise.

Formato proposto, um arquivo versionado `crates/runtime/efeitos.tsv`, lido pelo `build.rs` do runtime e
pelo emissor:

```text
# nome                          coleta  lanca  roda_dart
dartforge_alocar                1       0       0
dartforge_exception_throw       1       1       0
dartforge_nativo_Function_apply 1       1       1
```

* `roda_dart = 1` implica `coleta = 1` e `lanca = 1`.
* Uma extern ausente da tabela é erro de compilação do emissor, não "conservador por omissão".
* O emissor usa `lanca` para decidir a checagem (§13.2) e `coleta` para o `"gc-leaf-function"` (§3.3).

**Como a tabela é conferida** (a análise por nome não basta para confiar):

1. `DARTFORGE_EFEITOS=conferir` no runtime de teste: cada extern marcada `coleta = 0` roda com a
   coleta **proibida** (um contador no `Heap`; alocar com ele ligado aborta dizendo o nome da extern),
   e cada extern marcada `lanca = 0` confere, na saída, que a pendência não mudou.
2. O corpus nativo e o e2e do `new_sali/backend` rodam uma vez assim no Pesado.
3. Sabotagem: marcar `dartforge_string_concat` como `coleta = 0` tem de abortar no primeiro programa.

A primeira versão da tabela é a união conservadora: o que o emissor supõe hoje, **menos** as
divergências do tipo `emissor_conservador_lanca` que passarem no item 1. É a medida E1.1 do §6: quantas
das checagens somem só com a tabela, ainda no modo `checagem`.

### 13.9 As portas: onde o runtime chama Dart

Levantadas pelo mesmo script (`efeitos\sementes.tsv`). São três tipos.

**Ponteiro de função recebido ou `transmute`** — código Dart do usuário entra por aqui:

| ponto | arquivo:linha | o que chama |
|---|---|---|
| `dartforge_iniciar` | `RT/nucleo.rs:36` | a entrada do programa e o `para_texto` |
| `dartforge_nativo_Function_apply` | `RT/closures.rs:171`, `transmute` em `:211` | o código de uma closure, `fn(i64, *const i64, *const i64) -> i64` |
| `dartforge_laco_de_eventos` | `RT/eventos.rs:119` | `chamar: fn(i64) -> i64`, cada microtarefa e timer |
| `rodar_laco_do_isolado`, `rodar_isolado` | `RT/isolados.rs:331`, `:386` (`transmute` em `:391-392`) | `preparar: fn()` e `chamar: fn(i64) -> i64` |
| `dartforge_nativo_Isolate_spawnFunction` | `RT/isolados.rs:437` (ajudantes em `:418-419`) | `_dartforgeMensagemDePronto`, `_dartforgeIniciarIsolado` |
| `despachar_proxima` | `RT/portas.rs:1230` | `chamar: fn(i64) -> i64`, a mensagem de uma porta |
| `materializar` | `RT/portas.rs:626` (`transmute` em `:662`) | getters `fn() -> i64` de constantes |
| `dartforge_ponto_seguro`, `dartforge_parar_isolados` | `RT/isolados.rs:353`, `RT/portas.rs:1106` | pedidos no ponto seguro |
| `dartforge_publicar_geracao` | `RT/seletores.rs:212` | `area`, `registrar`, `rti` da geração nova (recarga) |
| `dartforge_encaminhar_nsm`, `invocar_no_such_method` | `RT/seletores.rs:411`, `:374` (`transmute` em `:399`) | o `noSuchMethod` do usuário |
| `dartforge_lista_de_tabela_g` | `RT/nativos_listas.rs:468` | getters de literais |
| `finalizar_programa` | `RT/nucleo.rs:69` | `PARA_TEXTO` da exceção não tratada |
| `relatar_erro_nao_tratado` | `RT/isolados.rs:270` (`transmute` em `:277`) | `_dartforgeDescreverErro` |
| `refazer_indices_copiados` | `RT/portas.rs:791` (`transmute` em `:795`) | rehash depois de copiar entre isolados |

**Ajudantes do SDK registrados por nome** — construtores de erro e conversões escritos em Dart, chamados
pelo runtime para montar um objeto: `RT/excecoes.rs:311-548` (doze pontos: `FormatException`,
`StateError`, `ArgumentError`, `RangeError`, `UnsupportedError`, `AssertionError`, `LateError`,
`StackOverflowError`, `NoSuchMethodError`, e o genérico `erro_da_fonte_com_texto`), `RT/ffi.rs:248`,
`RT/io_arquivos.rs:126`, `RT/io_diretorios.rs:345`, `RT/io_plataforma.rs:69`, `RT/io_processos.rs:61`,
`RT/io_servico.rs:300`, `RT/io_soquetes.rs:566`, `:944`, `:962`, `:1054`, `:1073`,
`RT/isolados.rs:519`, `RT/nativos_listas.rs:41`, `RT/tipos.rs:1502`.

**Tabelas geradas, não Dart** (`fn() -> *const i64`): `RT/seletores.rs:147`, `:247`, `RT/ffi.rs:803`,
`RT/typed_data.rs:85`, `:104`. Não lançam e **não** precisam de porta.

O molde único no runtime:

```rust
// RT/chamar_dart.rs (novo)
/// A única maneira de o runtime chamar código gerado. Em `tabelas`, a porta da assinatura pega
/// qualquer desenrolamento e devolve com a pendência ligada; em `checagem`, chama direto.
#[inline]
pub fn chamar_dart1(f: usize, a: i64) -> i64 {
    match portas_registradas().r1 {
        Some(porta) => porta(f, a),                                     // extern "C" fn(usize, i64) -> i64
        None => { let g: extern "C" fn(i64) -> i64 = unsafe { core::mem::transmute(f) }; g(a) }
    }
}
// uma por assinatura: r0, r1, r2, r3, v0 (fn()), closure (i64, *const i64, *const i64), nsm (7 × i64)
```

As portas são registradas por `@df.preparar_isolado` numa tabela do runtime
(`dartforge_registrar_portas(ptr)`), uma entrada por assinatura; em `checagem` a tabela fica vazia. O
`grep` do CI recusa `transmute` para `extern "C" fn` fora de `chamar_dart.rs` e das cinco tabelas
geradas acima.

As assinaturas a gerar saem da tabela acima: `fn()`, `fn() -> i64`, `fn(i64) -> i64`,
`fn(i64, i64) -> i64`, `fn(i64, i64)`, `fn(i64, i64, i64) -> i64`,
`fn(i64, *const i64, *const i64) -> i64`, `fn(i64 × 7) -> i64`. Oito portas.

### 13.10 A LSDA, byte a byte

O que o `llc` 22.1.8 emite para a função `b_catch` do `x2\lsda.ll` (um `landingpad … catch ptr null`
compartilhado por dois `invoke`, com uma chamada comum antes e outra depois), no COFF x64:

```text
GCC_except_table1:
  .byte 255                 ; LPStart encoding = DW_EH_PE_omit: os pousos são relativos ao início da função
  .byte 0                   ; TType encoding = DW_EH_PE_absptr  (ELF PIC: 155 = indirect|pcrel|sdata4)
  .uleb128 ttbase-ref       ; distância até o fim da tabela de tipos (presente porque TType != omit)
  .byte 1                   ; call-site encoding = DW_EH_PE_uleb128
  .uleb128 tamanho          ; tamanho da tabela de call sites em bytes
  ; tabela de call sites, ordenada por início; cada entrada = quatro uleb128
  .uleb128 0                ;   início (relativo ao início da função)
  .uleb128 Ltmp0-inicio     ;   comprimento
  .byte 0                   ;   pouso = 0: sem landing pad
  .byte 0                   ;   ação = 0
  .uleb128 Ltmp0-inicio     ;   2ª faixa: do primeiro invoke ao fim do segundo
  .uleb128 Ltmp3-Ltmp0
  .uleb128 Ltmp4-inicio     ;   pouso (relativo ao início da função)
  .byte 1                   ;   ação = 1 (índice+1 na tabela de ações)
  .uleb128 Ltmp3-inicio     ;   3ª faixa: o resto
  .uleb128 fim-Ltmp3
  .byte 0
  .byte 0
  ; tabela de ações
  .byte 1                   ;   filtro de tipo 1
  .byte 0                   ;   sem próxima ação
  .p2align 2
  .quad 0                   ; tabela de tipos: TypeInfo 1 = null = pega tudo (ELF PIC: .long 0)
ttbase:
```

Fatos para o decodificador:

1. Com `catch ptr null` o segundo byte **não** é `0xff`; segue um uleb128 que o decodificador pula. Com
   `cleanup` puro (modo mapas) o segundo byte é `0xff` e não há esse uleb128. O decodificador trata os
   dois casos (é o que `achar_pad` do `x1` faz).
2. O primeiro byte é sempre `0xff` e o terceiro campo é sempre `0x01` nos alvos testados. O decodificador
   **aborta** com mensagem se achar outra coisa: é sinal de versão ou alvo não previsto.
3. A busca é pelo endereço de retorno **menos 1** (o `ControlPc − 1` do `x1`): o endereço de retorno é
   o da instrução seguinte ao `call`, que pode já estar na faixa seguinte.
4. Uma função com personalidade e **sem** `invoke` (caso `a_sem_pad`) tem `.seh_handler` e uma tabela
   vazia. O emissor não põe personalidade nessas funções (§13.2), então o caso não deve ocorrer; a
   personalidade devolve "continue a busca" se ocorrer.
5. Ações e tabela de tipos são ignoradas: todo pouso pega tudo.
6. Onde fica: no COFF, dentro do `.xdata` (via `.seh_handlerdata`), apontada pelo `HandlerData` do
   `DISPATCHER_CONTEXT`; no ELF, `.gcc_except_table`, apontada pelo `.cfi_lsda` da FDE
   (`_Unwind_GetLanguageSpecificData`); no Mach-O, `__gcc_except_tab`.
7. Personalidade no ELF: `.cfi_personality 155, DW.ref.dartforge_personalidade` em PIC (indireta, por um
   símbolo `DW.ref.` em seção `.data` com `comdat`) e `.cfi_personality 3, dartforge_personalidade` em
   código estático.

### 13.11 A personalidade

Uma função, em `RT/excecoes_tabelas.rs`, com o decodificador da §13.10 compartilhado.

```rust
/// Deslocamento do pouso para `ip_rel` (relativo ao início da função), ou 0.
fn achar_pouso(mut p: *const u8, ip_rel: u64) -> u64 {
    unsafe {
        if ler_u8(&mut p) != 0xff { abortar("LSDA: LPStart inesperado"); }
        if ler_u8(&mut p) != 0xff { let _ = uleb(&mut p); }
        if ler_u8(&mut p) != 0x01 { abortar("LSDA: codificação de call site inesperada"); }
        let tam = uleb(&mut p); let fim = p.add(tam as usize);
        while p < fim {
            let ini = uleb(&mut p); let len = uleb(&mut p); let pouso = uleb(&mut p); let _acao = uleb(&mut p);
            if ip_rel < ini { break; }
            if ip_rel < ini + len { return pouso; }
        }
        0
    }
}

// Windows x64 e arm64
const DF_CODIGO: u32 = 0xE044_4652;
#[unsafe(no_mangle)]
pub unsafe extern "system" fn dartforge_personalidade(
    rec: *mut EXCEPTION_RECORD, quadro: *mut c_void, _ctx: *mut CONTEXT, disp: *mut DISPATCHER_CONTEXT,
) -> EXCEPTION_DISPOSITION {
    // 1. Exceção estrangeira (violação de acesso, C++): não é nossa.
    if (*rec).ExceptionCode != DF_CODIGO { return ExceptionContinueSearch; }
    // 2. Segunda passagem (EXCEPTION_UNWINDING | EXCEPTION_EXIT_UNWIND = 0x6): não há limpeza a fazer.
    if (*rec).ExceptionFlags & 0x6 != 0 { return ExceptionContinueSearch; }
    // 3. Busca: este quadro tem pouso para o ponto da chamada?
    let inicio = (*disp).ImageBase + (*(*disp).FunctionEntry).BeginAddress as u64;
    let pouso = achar_pouso((*disp).HandlerData as *const u8, (*disp).ControlPc - 1 - inicio);
    if pouso == 0 { return ExceptionContinueSearch; }
    // 4. Achou: desenrola até este quadro e continua no pouso. Não volta.
    RtlUnwindEx(quadro, (inicio + pouso) as *mut c_void, rec, core::ptr::null_mut(),
                (*disp).ContextRecord, (*disp).HistoryTable);
    abortar("RtlUnwindEx voltou")
}
```

Conferido no `x1`:

* os contadores impressos (1 busca aceita, 0 chamadas de desenrolar em quadros intermediários, 1 chamada
  com `EXCEPTION_TARGET_UNWIND` no quadro alvo) correspondem a **um** lançamento: quadros sem
  personalidade no caminho não chamam nada. A
  personalidade é chamada de novo no quadro alvo durante a segunda passagem; o item 2 a faz devolver
  "continue", e o `RtlUnwindEx` transfere o controle;
* o quarto argumento de `RtlUnwindEx` (`ReturnValue`) chega no `RAX`, que é o primeiro campo do par do
  `landingpad` (`pega_e_rax(4)` devolveu `0xABCD`). Não é usado; passa-se nulo;
* um valor vivo num registrador *callee-saved* da função que pega (`%aa` em `pega`) está correto no
  pouso;
* uma violação de acesso atravessando um quadro com a nossa personalidade chega intacta ao filtro de
  topo do processo.

Custo medido do lançar-e-pegar no `x1` (20.000 repetições por profundidade, máquina em uso; duas
execuções deram números bem diferentes, então só a ordem de grandeza vale):

| quadros atravessados | ns por lançar+pegar |
|---:|---|
| 1 | ~2.800 |
| 10 | ~4.800 |
| 100 | ~29.000 |
| 1.000 | 65.000 a 223.000 |

É o custo que o critério de abandono do §6 compara com o modo `checagem` ("mais de 50× mais lento"); a
medida do modo `checagem` no mesmo programa ainda não foi feita.

Itanium (Linux, macOS; **não executado aqui**):

```rust
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_personalidade(
    versao: c_int, acoes: c_int, classe: u64, _obj: *mut _Unwind_Exception, ctx: *mut _Unwind_Context,
) -> _Unwind_Reason_Code {
    if versao != 1 { return _URC_FATAL_PHASE1_ERROR; }
    if classe != u64::from_be_bytes(*b"DARTFRGE") { return _URC_CONTINUE_UNWIND; }   // estrangeira
    let mut antes: c_int = 0;
    let ip = _Unwind_GetIPInfo(ctx, &mut antes) as u64;
    let ip = if antes == 0 { ip - 1 } else { ip };
    let inicio = _Unwind_GetRegionStart(ctx) as u64;
    let pouso = achar_pouso(_Unwind_GetLanguageSpecificData(ctx) as *const u8, ip - inicio);
    if pouso == 0 { return _URC_CONTINUE_UNWIND; }
    if acoes & _UA_SEARCH_PHASE != 0 { return _URC_HANDLER_FOUND; }
    // fase de limpeza, no quadro que a busca aceitou (_UA_HANDLER_FRAME) ou num desenrolar forçado
    if acoes & _UA_FORCE_UNWIND != 0 { return _URC_CONTINUE_UNWIND; }   // pthread_cancel etc.: não paramos
    _Unwind_SetGR(ctx, REG_DADO_0, 0); _Unwind_SetGR(ctx, REG_DADO_1, 0);
    _Unwind_SetIP(ctx, (inicio + pouso) as usize);
    _URC_INSTALL_CONTEXT
}
```

`REG_DADO_0/1` são os registradores de dados da exceção do alvo (x86-64: 0 e 1; aarch64: 0 e 1). O
runtime e o SDK precisam de tabelas de desenrolamento (`-C force-unwind-tables=yes`), como o §3.6 já diz.

### 13.12 O que muda no runtime

| arquivo | mudança |
|---|---|
| `RT/excecoes_tabelas.rs` (novo) | `achar_pouso`, a personalidade por sistema, `dartforge_objeto_de_desenrolamento`, `dartforge_desenrolamento_sem_tratador` |
| `RT/chamar_dart.rs` (novo) | os ajudantes `chamar_dart*`, a tabela de portas registradas |
| `RT/heap.rs` (`Contexto`, `:247-305`) | campo novo **no fim** para o `_Unwind_Exception` (só não-Windows); o `assert` de deslocamentos de `:315-332` ganha a linha; `pendente` em 0 e `topo` em 8 não mudam |
| `RT/gc_raizes.rs:109-121` | `dartforge_estouro_de_pilha` não muda; quem muda é o prólogo gerado, que em `tabelas` chama `@df.lancar` depois dela |
| os 14 pontos da §13.9 | trocar o `transmute` + chamada por `chamar_dart*` |
| `RT/lib.rs` | marcador de ABI `dartforge_abi_excecoes` (0 = checagem, 1 = tabelas), conferido na ligação (§5.4) |

Nada muda em `dartforge_exception_throw`, `peek_ref`, `capturavel`, `clear` e `stack_trace_get`
(`RT/excecoes.rs:130`, `:208`, `:214`, `:231`, `:289`).

### 13.13 `async`, isolados e JIT

* **`async`.** O corpo `f$async` é chamado pelo laço de eventos por `chamar_dart1`. O `try` sintético do
  topo (`EN/lower/async_sm.rs:333-352`) pega tudo, logo nenhuma exceção chega à porta em operação
  normal; a porta é a garantia. A leitura do código de retomada (`código == 1`: a exceção chega por um
  `await`) continua como hoje: é um `throw` local. **Não lido nesta rodada:** os pontos exatos de
  `async_sm.rs` em que a máquina de estados reentra num `try` do usuário depois de um `await`; a regra
  é que cada chamada dentro da região protegida, em qualquer estado, vira `invoke` para o pouso daquele
  estado.
* **Isolados.** Cada isolado é uma thread; a personalidade não tem estado global. O "não capturável"
  (`Isolate.exit`) sobe de pouso em pouso por `@df.lancar` até a porta de `rodar_isolado`.
* **JIT.** O `lli` 22.1.8 registra o `.pdata` do código JIT (`x14`). O `crates/jit` continua em
  `checagem` na Etapa 1 (§6); o passo E4 repete o `x14` com a configuração do `crates/jit` (ORCv2,
  camada de ligação própria) antes de ligar `tabelas` nele.

### 13.14 Rastro no formato da VM

Sete das doze divergências do placar do pub são o texto do rastro. Hoje `dartforge_stack_trace_get`
devolve o rastro corrente ou o texto fixo `#0      main (dart:native)\n` (`RT/excecoes.rs:130-135`).

O formato da VM, lido em `E:\references\dart-sdk\runtime\vm\object.cc` (ramo main; a tag 3.6.2 não tem
`runtime/` na cópia local, então a igualdade com o 3.6.2 é **não verificada**):

```text
"#%-6" Pd  " %s (%s"  [":%" Pd  [":%" Pd]]  ")\n"
   índice     função   url     linha   coluna
```

* o índice é alinhado à esquerda em 6 colunas depois do `#` (`PrintSymbolicStackFrameIndex`, `:26846`);
* a função é o nome qualificado visível ao usuário (`QualifiedUserVisibleNameCString`): `Classe.metodo`,
  `main`, `Classe.metodo.<anonymous closure>`;
* a url é a do script (`package:x/y.dart`, `file:///…`, `dart:core`); uma url `data:application/dart;…`
  vira `<data:application/dart>`;
* a linha e a coluna só saem quando conhecidas; a coluna só com a linha;
* entre trechos assíncronos sai a linha `<asynchronous suspension>` (`:27050`).

O que a implementação precisa, em qualquer dos quatro modos:

1. **Tabela pc → (função, linha, coluna)** por imagem, na seção `.dfpcl$m` (COFF), `SHF_GNU_RETAIN`
   (ELF): por função, o símbolo, o índice do nome e da url numa tabela de textos, e um fluxo de
   `(delta do endereço de retorno, delta da linha, coluna)` em varints, um por ponto de chamada. Os
   pontos são os mesmos dos registros do mapa de pilha (§14); em `mapas`, as duas tabelas podem
   compartilhar o índice de funções.
2. **Captura** no `throw`: percorrer os quadros pelo desenrolador (o percorredor do §14.6, sem visitar
   raízes) e guardar só os endereços de retorno, num vetor do objeto de rastro. Simbolizar só quando o
   texto for pedido (`toString`). Em `sombra`/`checagem` no Windows x64 o percorredor é o mesmo
   (`RtlVirtualUnwind`); ele não depende dos mapas.
3. **Opção de omitir**: `--rastro=simbolico|nenhum`. Em `nenhum`, a tabela não é emitida e o texto é o
   de hoje.

Não medido: o tamanho da tabela no `new_sali/backend`. É a primeira medida do passo.

**Estado em 2026-10-05 (escrito, não compilado).** O rastro existe com `--rastro=simbolico`
(`DARTFORGE_RASTRO_VM=simbolico`; `alvo::rastro_simbolico`), nos quatro modos de raízes e exceções, no
Windows x86-64 e no Linux e no macOS em x86-64 e aarch64. O padrão continua `nenhum`, com o IR de sempre
byte a byte e o texto de hoje: o padrão só muda depois da medida do tamanho e de um teste que rode.

*Posições.* As posições do J05 passam a valer sem o `--depuracao` quando o rastro está ligado
(`Context::depuracao` indexa as linhas; `Context::dwarf` e `hir::Module::dwarf` dizem se viram tabelas
de linha). Cada expressão que chama marca a posição que a VM dá a ela, o `fileOffset` do front_end
(`offsetForToken(selector.token)`): o nome chamado em `f()`, `o.m()` e `o.p`; o operador em `a + b`,
`a += b` e `e as T`; o `[` em `a[i]`; o começo em `throw`, `await`, criação, unário e identificador
(`lower/expressoes.rs`, `posicao_da_expressao`). A posição volta à de fora quando a expressão termina.
A url é a da VM (`Context::url_do_rastro`): `file:///…`, `package:…`, `dart:core` para a unidade que
define a biblioteca do SDK, `dart:core/list.dart` para as partes e `dart:core-patch/…` para as da VM e
da sobreposição nativa.

*A tabela* (`EN/llvm/rastro.rs`). Não é a do item 1 (deltas em varints por função): esta é a forma que
sai sem ler o objeto depois da emissão. Antes de cada chamada rotulável (a indireta, a de função Dart e
a extern que a tabela de efeitos marca `lanca` ou `roda_dart`; nunca um intrínseco), um
`asm sideeffect` com `"gc-leaf-function"` (o RS4GC não trata `asm` como folha) e
`memory(inaccessiblemem: readwrite)` (o otimizador não o move nem o apaga, e ele não esconde memória
visível) define o rótulo `42:` e empurra para a seção uma entrada de 12 bytes: o rótulo e o registro
da função como deslocamentos a partir do próprio campo, e `linha << 12 | coluna`. O registro da função
é uma constante do módulo: `i32` até a url (um texto por unidade) e o nome terminado em zero. As
seções: `.dfpcl$m` no COFF, com sentinelas `.dfpcl$a`/`$z` em `comdat`; `dfpcl` no ELF com `aR?`
(`SHF_GNU_RETAIN`, e no grupo da função `comdat`); `__DATA,__dfpcl,regular,no_dead_strip` no Mach-O,
com um símbolo `l…${:uid}` por entrada (o escritor Mach-O do aarch64 só reloca subtração com símbolo
não temporário antes; o `l…` não chega ao executável). As diretivas foram conferidas na fonte do LLVM
22.1.8 (`E:\references\llvm-project-22`: `COFFAsmParser`, `ELFAsmParser`, `DarwinAsmParser`, o `s`
genérico de `TargetLowering` e o `c` do `AsmPrinter`). Uma função `linkonce_odr` (as entradas de
tear-off e as constantes canônicas) não é rotulada, e as chamadas dela saem `noinline`: uma entrada
copiada para ela pelo inlining apontaria para a cópia que o ligador descarta. Uma âncora em
`module asm` foi recusada: um símbolo local em `module asm` tira da importação do ThinLTO toda função
com `asm` do módulo. O registro (`dartforge_registrar_rastro(começo, fim)`) vai na função de registro
de cada módulo (`seletores.rs`, `emitir_registro`), com os mapas, que agora também saem pelo formato
da imagem (antes o módulo do SDK chamava a forma do Windows em todo sistema).

*O runtime* (fragmento `RT/rastro.rs`). O `throw` (`dartforge_exception_throw`) guarda os endereços
de retorno e o começo da função de cada quadro (`heap::enderecos_de_retorno`, o percorredor do §14.6
sem ler raízes), até 1024 quadros, sem tocar no heap. O `dartforge_stack_trace_get` transforma os
endereços num `_StackTrace` com o campo 0 nulo, o número de quadros e os pares brutos, e o deixa como
rastro corrente (o `_stackTrace` do erro e o `catch (e, s)` veem o mesmo objeto). O texto é feito no
primeiro `toString` (`dartforge_rastro_texto`, agora o corpo do `_StackTrace.toString`) e guardado no
campo 0; a exceção não capturada e a descrição do runtime o fazem do mesmo jeito. Cada endereço de
retorno acha a entrada do maior rótulo abaixo dele, que tem de estar na função dele (o começo que o
desenrolador dá); o quadro sem entrada (o runtime, o sistema, função sem posição) fica de fora. Sem
quadro nenhum, o texto de hoje. O `StackTrace.current` captura a pilha dele.

*Não feito.* A linha `<asynchronous suspension>`; os quadros das funções copiadas pelo inlining (o
quadro da função de fora some; a VM os mostra pela tabela de inlining); a forma compacta do item 1
(deltas por função, no conversor de objeto); a tabela no JIT. O `StackTrace.current` chamado pelo
getter do SDK mostra o quadro do getter quando ele tem posição.

---

### 13.15 Estado da implementação (2026-10-04)

A Etapa 1 está **escrita e não executada**: o código abaixo foi escrito por leitura e revisão, sem
compilar nem rodar. O padrão continua `checagem`, com o IR de sempre byte a byte. No runtime, a única
mudança que vale nos dois modos é a chamada a código Dart passar por `dart_r<n>`/`dart_v<n>`, que sem
porta registrada chamam direto, como antes.

**O que existe.**

| peça | onde | passo |
|---|---|---|
| a chave: `--excecoes=checagem\|tabelas` em `compile-native` e `aot`, e a variável `DARTFORGE_EXCECOES` | `crates/cli/src/nativo.rs` (`definir_modelo_de_excecoes`), `EN/alvo.rs` (`excecoes_por_tabelas`) | E1.0 |
| recusa por alvo (só Windows x86-64), recusa com `DARTFORGE_RASTRO=1`, recusa no JIT | `EN/alvo.rs`, `EN/lib.rs` (`emitir_ir_interno`) | E1.0 |
| o modo na chave do SDK compilado (o padrão não entra na chave) | `EN/sdk_modulo.rs` (`chave_do_sdk`) | E1.0 |
| o passe sobre a HIR já otimizada: sítios, pousos, saídas por exceção | `EN/otimizar/tabelas.rs`, `EN/hir.rs` (`TabelasDaFuncao`, `SaidaPorExcecao`, `Module::tabelas`) | E1.2 |
| `invoke`/`landingpad`, restauração do topo, `@df.lancar`, a saída guardada, o estouro de pilha | `EN/llvm/mod.rs` (`tornar_invoke`, `EXCECOES_POR_TABELAS`, os terminadores) | E1.2 |
| as 16 portas, a tabela `@df.portas`, o registro e a entrada do programa pelas portas | `EN/llvm/mod.rs` (`emitir_portas`, `chamada_de_entrada`, `emit_entry`, `chamar_main`) | E1.4 |
| a entrada C do callback do `dart:ffi` com pouso próprio | `EN/llvm/mod.rs` (`emit_callbacks_ffi`) | E1.4 |
| a personalidade SEH, o leitor da LSDA, o registro das portas e as chamadas `dart_r<n>`/`dart_v<n>` | `RT/excecoes_tabelas.rs` | E1.3, E1.4 |
| os pontos em que o runtime chama Dart, todos pelas portas | os 34 `transmute` de ajudantes, `RT/closures.rs`, `RT/nucleo.rs`, `RT/eventos.rs`, `RT/portas.rs`, `RT/nativos_listas.rs` | E1.4 |
| `RaiseException` e `RtlUnwindEx` nas importações do `kernel32` | `EN/ligador_windows.rs` | E1.2, E1.3 |
| `optsize` antes da personalidade na linha do `define` | `EN/sdk_modulo.rs` (`com_optsize`) | E1.2 |

**Como ficou diferente do texto acima.**

1. **O lowering não muda.** O §15.1 previa `emit_call_with_check` seguindo a tabela do §13.2. Em vez
   disso há um passe (`otimizar::excecoes_por_tabelas`) que roda **depois** de `otimizar::otimizar` e
   reescreve a forma de conferência que sai dele. Os passes existentes (`efeitos`, `simplificar`,
   `inline`, `escape`) continuam vendo só a forma de conferência, e a máquina de estados `async` (§13.13)
   cai na regra geral sem código próprio. A decisão por chamada é a do §13.2, com um caso a mais:
   * chamada direta (`CallStatic`, `ChamadaTipada`) conferida, tratador que só devolve o padrão:
     a conferência some, `call` puro;
   * chamada direta conferida, tratador de verdade: a conferência some, `invoke`, o pouso desvia ao
     tratador (e entra nos φ dele no lugar do bloco da chamada);
   * chamada de closure ou por seletor (podem voltar pendentes: a entrada inválida e a de
     `noSuchMethod` são do runtime), tratador que só devolve o padrão: nada muda;
   * **qualquer outra forma**: emulação exata. A chamada vira `invoke`, o pouso continua no ponto
     seguinte à chamada, e um φ dá ao resultado o valor padrão no caminho do pouso — o que a chamada
     devolvia em `checagem`, com a exceção pendente.
2. **Como cada `Return` sai** é decidido por fluxo de dados da pendência (limpa, ligada, talvez), não
   pela posição no lowering: ligada vira `@df.lancar`; talvez vira a leitura da pendência seguida de
   `@df.lancar` ou `ret`. A entrada da função é "limpa" e a de um pouso, "ligada".
3. **`dartforge_exception_clear` que não limpa** (o desenrolar do isolado) é seguido de `@df.lancar`:
   é o `desenrola:` do §13.5, posto no próprio ponto da limpeza.
4. **`@df.lancar` é `internal`**, uma cópia por módulo (e por parte de um módulo dividido), em vez de
   `linkonce_odr`: não precisa de `comdat` nem de regra nova no particionador.
5. **Dezesseis portas, por aridade**, em vez de oito por assinatura: `@df.porta.v<n>` e `@df.porta.r<n>`,
   `n` de 0 a 7, no índice `2n + (1 se devolve)`. Ponteiros passam como palavras. As portas são
   registradas uma vez, no começo de `dartforge_entry` (são endereços de código do processo inteiro).
   O arquivo do runtime é um só (`excecoes_tabelas.rs`); não há `chamar_dart.rs`.
6. **Sem features do Cargo e sem marcador de ABI** (§5.1, §5.4). O runtime é o mesmo nos dois modos: a
   personalidade está sempre compilada e sem porta registrada as chamadas vão direto. A mistura de
   modos entre o programa e o SDK é impedida pela chave do SDK.
7. **O getter de tipo** (`df.rti.…`, chamado sem conferência em `lower/rti.rs`) fica `call` puro: se
   lançar, o desenrolamento atravessa quem chamou.

**O que falta da Etapa 1.**

* **Rodar.** Nada foi compilado. A ordem de verificação é a do §15.1: os testes de unidade de
  `pouso_da_lsda` (escritos em `RT/excecoes_tabelas.rs`), os programas dirigidos de E1.2 e E1.4, o corpus
  nativo com e sem `--gc-stress`, o e2e do `new_sali/backend`, e as medidas do §8.
* **E1.1** está escrito: `crates/runtime/efeitos.tsv` (uma linha por extern, conferida pelo `build.rs`
  do runtime e lida pelo emissor), a conferência da pendência só depois da extern que lança
  (`emit_call_with_check`; `DARTFORGE_SEM_EFEITOS_NA_CONFERENCIA=1` volta atrás) e o modo
  `DARTFORGE_EFEITOS=conferir` (`RT/efeitos_conferir.rs`). As marcas são as que o emissor já supunha:
  nenhuma passou de 1 para 0, porque isso exige a execução que a confere (§13.8).
* No modo `tabelas`, a limpeza da pendência onde ela está comprovadamente limpa sai
  (`EN/otimizar/tabelas.rs`), e a função que chama código Dart continua conferindo a pilha no prólogo
  (`TabelasDaFuncao::confere_pilha`).
* **O `grep` do CI** contra `transmute` para `extern "C" fn` fora de `excecoes_tabelas.rs`: escrito em
  2026-10-05 como teste do runtime (`crates/runtime/tests/portas_dart.rs`: todo `transmute` para
  ponteiro de função fora das portas está numa lista com o motivo), que o `cargo test --workspace` do
  `ci.yml` roda.
* **Os perfis do diferencial** (`aot-tabelas`): o nome do perfil na matriz já sai do ambiente
  (`aot-tabelas`, `aot-mapas`, `aot-mapas-tabelas`, com `-gc-stress`); **estado em 2026-10-05 (escrito,
  não executado)**: o job `nativo-modos` do `pesado.yml` roda A1, B0 e B1, cada uma com e sem
  `--gc-stress`, no corpus nativo inteiro (§7.1); A0 são os jobs `nativo` e `nativo-gc-stress`.
* **O rastro no formato da VM** (§13.14): escrito em 2026-10-05, atrás de `--rastro=simbolico`
  (veja o estado no §13.14).
* **Linux e macOS** (a personalidade Itanium do §13.11): Etapa 4.

---

## 14. Etapa 2 em nível de implementação: raízes por mapas (B0/B1)

Detalha o §3. O desenho é o do `x7` (formato do LLVM) e do `x12` (formato compacto), que rodam.

### 14.1 O IR que o emissor passa a gerar

Cabeçalho do módulo e de cada função gerada:

```llvm
target datalayout = "…-S128-ni:1"                       ; -ni:1 = addrspace(1) é não integral
define ptr addrspace(1) @f(ptr addrspace(1) %x) gc "statepoint-example" { … }
declare void @rt_confere(ptr addrspace(1)) "gc-leaf-function"     ; extern com coleta = 0 na tabela de efeitos
```

Regras, cada uma exercitada no `x7` ou no `x9`:

| valor | sombra (hoje) | mapas |
|---|---|---|
| referência (`Ref`) | `i64` | `ptr addrspace(1)` |
| null | `i64 0` | `ptr addrspace(1) null` |
| `Smi` de `v` | `(v << 1) \| 1` | `inttoptr i64 ((v << 1) \| 1) to ptr addrspace(1)` |
| é `Smi`? | `and 1` | `ptrtoint` + `and 1` |
| valor do `Smi` | `ashr 1` | `ptrtoint` + `ashr 1` |
| objeto estático | `ptrtoint (@g) + 2` | `getelementptr (i8, ptr addrspace(1) @g, i64 2)`, com `@g` em `addrspace(1)` |
| campo `k` | aritmética em `i64` + `inttoptr` | `getelementptr i8, ptr addrspace(1) %h, i64 (14 + 8k)` + `load` |
| `identical` | `icmp eq i64` | `icmp eq ptr addrspace(1)` |
| chamada que coleta | `call` + slots da pilha-sombra | `call` comum; o RS4GC a transforma em statepoint |
| chamada que não coleta | `call` | `call` com `"gc-leaf-function"` no destino |

`inttoptr` para `addrspace(1)` só aparece nos três casos do C2. Um verificador do IR do emissor confere
(§7.4): varre o módulo antes do RS4GC e recusa qualquer outro.

O emissor faz isso por **funções de conversão** (passo E2.0 do §6): cada ponto de `EN/llvm/*.rs` que
hoje escreve `i64` para um `Ref` chama `ctx.tipo_ref()`, `ctx.smi(v)`, `ctx.campo(h, k)` etc., que no
modo sombra devolvem exatamente o texto de hoje. O critério do passo é **IR idêntico byte a byte** no
modo sombra, conferido pelo determinismo do corpus (`dartforge-diferencial determinismo --nativo`).

### 14.2 O pipeline

O do `x7\run.sh`, que roda:

```sh
opt -passes='default<O2>,rewrite-statepoints-for-gc,verify' prog.ll -o prog.rs.bc
llc -O2 -filetype=obj prog.rs.bc -o prog.obj
python gcmap.py prog.obj prog_v1.obj          # conversor do mapa (x12); no produto, em Rust
clang rt.obj prog_v1.obj -o x.exe -fuse-ld=lld
```

* RS4GC **depois** do O2. As variantes `o0`/`o0b` do `x7` (sem O2 antes) também passam e deixam mais
  raízes no mapa (3.251 visitas contra 3.201). A medida das duas ordens em programa real é o passo E2.3.
* No produto, `opt` e `llc` são chamadas em processo pela API C (`crates/llvm`): `LLVMRunPasses` com a
  mesma cadeia de passes e `LLVMTargetMachineEmitToMemoryBuffer`. **Não lido nesta rodada:** se
  `crates/llvm` já expõe as duas chamadas.
* ThinLTO: pendente (`x10` não concluído; item 6 do §11).

### 14.3 `.llvm_stackmaps` versão 3, byte a byte

Como o `llc` 22.1.8 o emite no COFF x64 (`llvm-readobj --stackmap x7\prog.obj`), little-endian:

```text
cabeçalho (16 bytes)
  u8  versão = 3;  u8 reservado = 0;  u16 reservado = 0
  u32 n_funcoes;   u32 n_constantes;  u32 n_registros
n_funcoes × StkSizeRecord (24 bytes)
  u64 endereço da função        ; relocação ADDR64 no objeto (IMAGE_REL_AMD64_ADDR64 = 1), uma por função
  u64 tamanho do quadro         ; 0xFFFF…FFFF = dinâmico
  u64 n_registros da função
n_constantes × u64
registros, na ordem das funções; cada um:
  u64 id                        ; 2882400000 (0xABCDEF00), o id padrão do RS4GC; inútil
  u32 deslocamento da instrução ; relativo ao início da função: o endereço de retorno
  u16 reservado
  u16 n_locais
  n_locais × local (12 bytes)
    u8 tipo; u8 reservado; u16 tamanho; u16 registrador DWARF; u16 reservado; i32 deslocamento ou constante
  enchimento até múltiplo de 8
  u16 enchimento; u16 n_live_outs
  n_live_outs × { u16 registrador; u8 reservado; u8 tamanho }
  enchimento até múltiplo de 8
```

Tipos de local: 1 `Register`, 2 `Direct`, 3 `Indirect` (`[reg + desloc]`), 4 `Constant`, 5 `ConstIndex`.

Num registro de statepoint, os locais são:

1. três `Constant`: convenção de chamada, bandeiras, **número de locais de `deopt`**;
2. os locais de `deopt` (zero no nosso caso);
3. pares **(base, derivado)**, um par por ponteiro vivo.

Exemplo real (`constroi`, quadro 72): `Constant 0`, `Constant 0`, `Constant 0`, `Indirect [R#7 + 48]`,
`Indirect [R#7 + 48]`, `Indirect [R#7 + 40]`, `Indirect [R#7 + 40]`. `R#7` é o RSP. São duas raízes,
cada uma com base igual ao derivado.

No módulo do `x7`: 5 funções, 13 registros, 1.280 bytes, **98,5 bytes por registro**.

Vários objetos ligados deixam vários blobs concatenados na seção, com enchimento de zeros entre eles;
o leitor do `x7` avança enquanto o byte é 0 e exige `versão == 3`. Na imagem PE o nome da seção sai
cortado, `.llvm_st` (o `x7` a acha por esse prefixo).

### 14.4 O mapa compacto DFGM v1 (substitui o layout do §3.5)

O layout do §3.5 foi ajustado no `x12`: o tamanho do quadro e o número de registros foram para dentro do
fluxo, e o cabeçalho ganhou o tamanho total do blob (para saltar de um blob ao seguinte). Vale este:

```text
cabeçalho (16 bytes)
  "DFGM"
  u8  versão = 1
  u8  alvo                 ; 1 = x86-64
  u16 bandeiras = 0
  u32 tamanho total do blob em bytes (múltiplo de 4)
  u32 n_funcoes
índice: n_funcoes × 8 bytes
  u32 rva da função        ; relocação relativa à base da imagem (COFF: IMAGE_REL_AMD64_ADDR32NB = 3)
  u32 início no fluxo      ; relativo ao primeiro byte depois do índice
fluxo de varints (LEB128 sem sinal), por função:
  varint n_registros
  varint tamanho_do_quadro / 8          ; 0 = dinâmico
  n_registros ×
    varint delta do deslocamento de retorno (em relação ao registro anterior da função; o 1º, ao início)
    varint cabeçalho
       bit 0 = 1: o conjunto de raízes é o do registro anterior (nada mais a ler)
       bit 0 = 0: (cabeçalho >> 1) = n_raizes, seguido de
          n_raizes × varint s
             bit 0 de s = base: 0 = SP, 1 = FP
             s >> 1 = zigzag(delta do slot em palavras de 8 bytes em relação ao slot anterior; o 1º, a 0)
enchimento com zeros até múltiplo de 4
```

Regras do conversor (as do §3.5, agora exercitadas):

* de cada par (base, derivado) só a base entra; bases repetidas no registro são deduplicadas;
* locais `Constant` e `ConstIndex` são descartados;
* local `Register` ou `Direct`, ou tamanho diferente de 8, ou registrador que não seja o SP nem o FP do
  alvo: **erro de compilação** ("local recusado");
* as raízes de um registro são ordenadas por (base, deslocamento) antes de codificar, o que maximiza a
  repetição entre registros vizinhos;
* dois registros no mesmo deslocamento de retorno: erro;
* o deslocamento do slot tem de ser múltiplo de 8: erro se não for;
* depois de codificar, decodifica e compara com o original (ida e volta); diferença é erro.

Medido no `x7\prog.obj`: 1.280 → 104 bytes, **8,0 bytes por registro** (o módulo é pequeno; o índice de
8 bytes por função pesa 40 dos 104).

No objeto COFF, o conversor (`x12\gcmap.py`, a portar para Rust em `EN/gcmap.rs`):

1. lê a seção `.llvm_stackmaps` e as relocações dela (uma `ADDR64` por função, no campo de endereço do
   `StkSizeRecord`);
2. sobrescreve o conteúdo com o blob (sempre menor) e ajusta `SizeOfRawData`;
3. renomeia a seção para `.dfgcm$m` (8 bytes exatos) e põe alinhamento 4;
4. troca cada relocação por uma `ADDR32NB` no campo `rva` da função correspondente no índice.

O runtime declara os marcadores `.dfgcm$a` e `.dfgcm$z`; o `lld-link` ordena as contribuições entre
eles. O leitor percorre de um marcador ao outro, salta de 4 em 4 bytes enquanto não acha `"DFGM"`
(enchimento entre contribuições), e avança pelo campo de tamanho total.

### 14.5 Índice e busca em tempo de execução

```rust
// RT/gc_mapas.rs (novo)
struct Funcao { inicio: usize, fluxo: *const u8 }          // 16 bytes por função gerada
struct Imagem { base: usize, funcoes: Vec<Funcao> }         // ordenado por `inicio`

/// Visita as raízes do quadro da função que começa em `inicio`, parado no endereço de retorno `ret`.
/// Devolve false se a função não tem mapa (runtime, C, sistema).
fn visitar_quadro(img: &Imagem, inicio: usize, ret: usize, sp: usize, fp: usize, f: &mut dyn FnMut(*mut i64)) -> bool
```

* O índice é montado uma vez por imagem em `dartforge_registrar_mapa(inicio, fim)`; o fluxo é
  decodificado sob demanda, por função.
* A busca é por **início da função**, que o percorredor já tem (`ImageBase + FunctionEntry.BeginAddress`
  no Windows; `_Unwind_GetRegionStart` no Itanium). Não há busca por endereço de retorno em tabela
  global.
* Dentro da função, percorre os registros somando os deltas até o deslocamento do retorno.
* **A regra do `nop`.** No Windows x64 foi observado que, em parte das chamadas, o byte no endereço de
  retorno é um `nop` (`0x90`) e o registro do mapa aponta **depois** dele (a causa, o `nop` que o backend
  põe depois de certos `call` para o desenrolador do Windows, não foi lida na fonte do LLVM). O
  leitor aceita `deslocamento == alvo`, ou `deslocamento == alvo + 1` quando o byte no endereço de
  retorno é `0x90`. No `x12`, 50 das consultas casaram pela segunda forma. Sem ela, o `x12` aborta com
  "função com mapa sem registro exato". Isto corrige o §3.5 ("registro exato, sem tolerância"): a
  tolerância é de um byte e condicionada ao `nop`.
* Função com mapa e sem registro para o endereço de retorno: aborta. É defeito do emissor ou do
  conversor.

### 14.6 O percorredor no Windows x64

O do `x7`/`x12`, que roda:

```rust
#[inline(never)]
fn percorrer_pilha(f: &mut dyn FnMut(*mut i64)) {
    let mut ctx: CONTEXT = zeroed(); RtlCaptureContext(&mut ctx);
    let (lo, hi) = limites_da_pilha();                       // GetCurrentThreadStackLimits
    loop {
        let mut base = 0u64;
        let fe = RtlLookupFunctionEntry(ctx.Rip, &mut base, null_mut());
        if fe.is_null() { break; }                           // folha sem .pdata ou fim da pilha
        let sp_antes = ctx.Rsp;
        RtlVirtualUnwind(UNW_FLAG_NHANDLER, base, ctx.Rip, fe, &mut ctx, &mut hd, &mut est, null_mut());
        // agora ctx descreve o quadro CHAMADOR: Rip = endereço de retorno nele, Rsp = SP dele na chamada
        if ctx.Rip == 0 || ctx.Rsp <= sp_antes || ctx.Rsp < lo || ctx.Rsp >= hi { break; }
        let fe2 = RtlLookupFunctionEntry(ctx.Rip, &mut base2, null_mut());
        if fe2.is_null() { continue; }
        let inicio = base2 + (*fe2).BeginAddress as u64;
        visitar_quadro(imagem_de(inicio), inicio, ctx.Rip, ctx.Rsp, ctx.Rbp, f);
    }
}
```

* Os locais `Indirect [RSP + d]` são relativos ao `Rsp` do quadro Dart **no ponto da chamada**, que é o
  `ctx.Rsp` depois de desenrolar o quadro chamado. Foi isso que o `x7` usou e as 3.201 raízes visitadas
  estavam todas corretas (nenhuma "raiz inválida", nenhum "uso depois de liberar").
* Os quadros Rust do runtime entre a coleta e o primeiro quadro Dart não têm mapa: `visitar_quadro`
  devolve `false` e o percurso segue.
* As condições de parada são as do `x7`. Uma anomalia (SP que não cresce, fora dos limites) encerra o
  percurso; no modo de verificação, aborta.
* O quadro-sombra residual (§3.7) e os frames de `com_raizes` são visitados à parte, como hoje.

No `x7`, com coleta em toda alocação: 351 coletas, 3.681 quadros percorridos, 1.876 com mapa, 3.201
raízes visitadas, 300 objetos mortos e envenenados, nenhum acessado depois. Em B1 (`teste_excecao`), a
exceção atravessa 50 quadros com raízes vivas, o pouso aloca (coleta) e depois usa a lista: resultado
1.232, o esperado.

### 14.7 As sabotagens que os testes têm de pegar

| sabotagem | efeito observado |
|---|---|
| marcar `"gc-leaf-function"` numa extern que coleta (`x7\sab.ll`: `@rt_novo`) | o mapa cai de 13 para 9 registros e a primeira coleta aborta com "raiz para objeto morto" |
| pouso sem restaurar o topo da pilha-sombra (`x16\a1_sab.ll`) | "quadro de raízes morto na pilha-sombra", código 3 |
| leitor sem a regra do `nop` (`x12`, antes da correção) | "função com mapa sem registro exato", código 5 |

Cada uma vira um teste do §7.3 que **tem de falhar** com a sabotagem ligada.

### 14.8 A forma implementada: raízes por mapas sem trocar a representação (2026-10-04)

O §14.1 troca a representação de `Ref` para `ptr addrspace(1)` no emissor inteiro. O que está escrito no
código é uma forma menor, que chega ao mesmo mapa de pilha e **não** mexe na representação. Ela só é
possível porque o coletor não move objetos.

**A ideia.** O valor `Ref` continua `i64` em todo o código gerado. Cada valor SSA enraizado (vivo na
entrada de algum ponto de coleta, a mesma análise de `EN/llvm/raizes.rs`) ganha, logo depois da
definição, um ponteiro com os mesmos bits:

```llvm
%raiz7 = inttoptr i64 %v7 to ptr addrspace(1)
```

Depois de cada ponto de coleta, o emissor mantém vivo o que estava vivo na **entrada** dele (os
operandos inclusive, que é o contrato C1):

```llvm
%v9 = call i64 @dartforge_string_concat(i64 %v7, i64 %v8)
call void (...) @llvm.fake.use(ptr addrspace(1) %raiz7)
call void (...) @llvm.fake.use(ptr addrspace(1) %raiz8)
```

A função com algum valor enraizado sai com `gc "statepoint-example"`. O `rewrite-statepoints-for-gc`
vê `%raiz7` vivo através da chamada, transforma a chamada em statepoint e põe `%raiz7` no `gc-live`; o
gerador de código o derrama num slot do quadro e o registra no `.llvm_stackmaps`. O código continua
lendo `%v7`, que pode ficar em registrador preservado: o `gc.relocate` só alimenta o `fake.use`, que
não gera instrução.

**O experimento que sustenta a forma** (`E:\dftemp\spec-mapas\r3\h1`, LLVM 22.1.8, só `opt` e `llc`;
é o experimento E2.1 do §15.2): com `default<O2>,rewrite-statepoints-for-gc,verify`, o `llvm.fake.use`
sobrevive ao O2, o `inttoptr` para `addrspace(1)` com `-ni:1` passa pelo verificador, e os três
statepoints do exemplo saem com os valores esperados no `gc-live`. O objeto COFF tem os registros com
`Indirect [RSP + 32]` e `[RSP + 40]`. No código, cada raiz custa um `mov` para o slot antes da primeira
chamada e uma carga morta depois de cada chamada; o valor usado pelo programa fica em `rsi`/`rdi`, sem
recarga.

**O que existe.**

| peça | onde |
|---|---|
| a chave `--raizes=sombra\|mapas` e `DARTFORGE_RAIZES`; recusa fora do Windows x86-64, sem o gerador embutido e no JIT; o modo na chave do SDK | `crates/cli/src/nativo.rs`, `EN/alvo.rs` (`raizes_por_mapas`), `EN/lib.rs`, `EN/sdk_modulo.rs` |
| a vivacidade por ponto de coleta, por fim de bloco e por entrada de bloco | `EN/llvm/raizes.rs` (`analisar`, `Raizes`) |
| `-ni:1` no datalayout, `gc "statepoint-example"`, `%raiz<v>`, `llvm.fake.use`, `"gc-leaf-function"` nas externs que não coletam e em cinco ajudantes `@df.*`, `landingpad token cleanup` nas funções `gc` | `EN/llvm/mod.rs` (`raiz_no_mapa`, `manter_vivos`, `ajudantes_folha`) |
| o passe depois do pipeline, com o verificador, e o gerador de código nunca em `-O0` nas funções `gc` | `crates/llvm/src/lib.rs` (`gerar`, `MARCA_DE_GC`) |
| recusa do gerador Clang (não roda o passe: o objeto sairia sem mapa); o conversor do mapa no objeto | `EN/gerador.rs`, `EN/gcmap.rs` |
| o registro da imagem (`dartforge_registrar_mapa(@__ImageBase)`) em cada módulo | `EN/llvm/seletores.rs` (`emitir_registro`), `RT/gc_raizes.rs` |
| os leitores do mapa compacto (`.dfgcm`) e do `.llvm_stackmaps` v3 (`.llvm_st`) da imagem, e o percorredor (`RtlVirtualUnwind`), com a regra do `nop` | `RT/heap.rs` (`ler_dfgm`, `ler_stackmaps`, `visitar_quadros_por_mapas`) |

**Regras da forma.**

1. **Mistura por função.** Só os valores SSA vão para o mapa. Os `alloca` `Ref` (o local cujo endereço
   escapa, e o que o `mem2reg` da HIR não promoveu) continuam no quadro da pilha-sombra da função, que é
   o quadro residual do §3.7. A coleta visita as duas fontes sempre.
2. **Folhas.** A extern com `coleta = 0` e `roda_dart = 0` em `efeitos.tsv` é declarada
   `"gc-leaf-function"`; também `@df.corpo`, `@df.barreira`, `@df.barreira_elemento`, `@df.e_objeto`,
   `@df.filho_jovem` e `@df.lancar`. Toda outra chamada numa função `gc` vira statepoint.
3. **Pouso.** Numa função `gc` o pouso é `landingpad token cleanup` (a forma do `x7`), e os valores
   vivos na entrada do pouso recebem `fake.use` nele: é o que os deixa vivos através do `invoke` no
   caminho de exceção.
4. **Sem inlining depois do passe.** Na produção com LTO o passe roda no fim do `lto-pre-link<O2>`; daí
   em diante uma chamada que pode coletar é um statepoint e não é mais embutida. É correto (a chamada a
   uma função sem `gc` também é statepoint em quem chama), e custa o inlining entre o programa e o SDK
   na ligação. O ThinLTO distribuído do §3.4 é o que devolve isso.
5. **O mapa compacto.** O objeto que o gerador embutido emite passa pelo conversor
   (`EN/gcmap.rs`, o porte do `gcmap.py` do `x12`): o `.llvm_stackmaps` vira `.dfgcm$m` no formato DFGM v1
   do §14.4, com a ida e volta conferida. O runtime lê a seção `.dfgcm` (`RT/heap.rs`, `ler_dfgm`) e
   também a `.llvm_st` crua, que é o que sobra nos objetos que não passam pelo conversor: os que o
   `lld` gera na LTO da produção (cerca de 98 bytes por registro, §14.3). Tirar isso da produção é o
   ThinLTO distribuído da Etapa 3. `DARTFORGE_SEM_MAPA_COMPACTO=1` deixa o mapa do LLVM (medida).
6. **Uma imagem compilada em `mapas` sem a seção `.llvm_st`** encerra o processo na partida, com
   mensagem: sem o mapa, toda raiz daquela imagem estaria perdida.

**O que falta da Etapa 2.**

* **Rodar.** Nada foi compilado nem executado além do experimento `h1`.
* O orçamento `vivos × safepoints` por função (§3.4) e a queda para a pilha-sombra acima do teto.
* A conferência cruzada mapa × pilha-sombra do E2.5 e as sabotagens do §14.7.
* O ThinLTO distribuído (Etapa 3) e os demais alvos (Etapa 4).

---

### 14.9 Etapa 3, E3.1: o ThinLTO distribuído (2026-10-04, escrito sem compilar)

A produção em partes com `--raizes=mapas` deixou de ser recusada e passou a ter caminho próprio.

| peça | onde |
|---|---|
| a entrada que recebe o bitcode **depois** do backend do ThinLTO e roda só `rewrite-statepoints-for-gc,verify` e o gerador de código (nunca o do `-O0`); `bitcode_com_mapas` | `crates/llvm/src/lib.rs` (`gerar_de_bitcode`, `Contexto::ler_bytes`) |
| os quatro passos do §3.4: índices (`lld-link /thinlto-index-only` com a linha de ligação inteira), backend por parte (`clang -x ir <parte> -fthinlto-index=<índice> -emit-llvm -c`), fecho em processo com o conversor do mapa compacto, objetos nativos em paralelo (`tarefas_de_geracao`) | `EN/lto_distribuida.rs` (`objetos`, `Pedido`) |
| o cache por parte, com a chave no bitcode depois da importação, na CPU e no modo do mapa | `EN/cache_objeto.rs` (`chave_de_bytes`), `EN/lto_distribuida.rs` |
| o Clang aceita IR com `gc "statepoint-example"` quando a saída é bitcode (a parte que vai para o ThinLTO distribuído) | `EN/gerador.rs` |
| a escolha do caminho e a ligação final só com objetos, sem LTO no ligador | `EN/driver.rs` (`ligar_distribuida`, `SEM_LTO_NA_LIGACAO`) |

Com isso a produção em partes sai com o mapa compacto DFGM em todos os objetos (o conversor roda em
cada um), e o passe dos mapas roda depois do inlining entre o programa e o SDK: são as duas perdas que a
regra 4 e a regra 5 do §14.8 registravam.

**Não verificado** (nada foi executado): que o `clang -x ir` preserva o `-ni:1` do datalayout do módulo
no backend do ThinLTO; o que o `lld-link /thinlto-index-only` faz com um bitcode sem resumo (o do
gerador embutido) — o fecho o trata sem índice, otimizado sozinho; o tempo com as dezenas de partes de
um programa grande; o caminho pelo `/thinlto-distributor:`. O passo 2 não tem cache: o índice de uma
parte não resume o conteúdo do que ela importa.

**Continua faltando da Etapa 3:** E3.3 (as medidas no `new_sali/backend`, que decidem se o modo vira
padrão). A decodificação preguiçosa foi escrita em 2026-10-05 (§14.10).

### 14.10 Etapas 2, 3 e 4: o que foi escrito em 2026-10-05 (sem compilar nem executar)

**Etapa 2, o que faltava.**

| peça | onde |
|---|---|
| o orçamento `vivos × pontos de coleta` por função (§3.4): acima do teto a função fica inteira na pilha-sombra, sem `gc` (o quadro residual do §3.7); `DARTFORGE_ORCAMENTO_MAPAS=<n>` (padrão 250 000, na chave do SDK); `DARTFORGE_RELATORIO_MAPAS=1` lista as funções fora | `EN/llvm/mod.rs` (`emit_function`), `EN/alvo.rs` (`orcamento_dos_mapas`) |
| o build de conferência do E2.5: com `DARTFORGE_RAIZES_CONFERIR=1` na compilação, cada função `gc` ganha, no fim do quadro da pilha-sombra, um slot por vivo do maior ponto de coleta, gravados com **exatamente** os vivos de cada ponto antes dele (zero nos demais); o campo `n` do quadro leva o número deles nos bits 40 em diante. Com `DARTFORGE_GC_PERCURSO=conferir` na execução, cada coleta exige que toda raiz desses slots esteja entre as que o percurso por mapas visitou, e uma anomalia do percurso aborta | `EN/llvm/mod.rs` (`gravar_conferencia`, `cabecalho_do_quadro`), `RT/heap.rs` (`conferir_percurso`, `visitar_slots_de_conferencia`) |
| as sabotagens (`DARTFORGE_SABOTAGEM`, na chave do SDK): `folha:<extern>`, `pouso_sem_topo`, `sem_uso_ficticio`, `bruto_no_mapa` no emissor; `sem_nop` no runtime | `EN/alvo.rs` (`sabotagem`), `EN/llvm/mod.rs`, `RT/heap.rs` |
| a recusa do conversor e do leitor para constante par não nula como raiz (§3.5) | `EN/gcmap.rs`, `RT/heap.rs` (`decodificar_funcao`) |
| os instrumentos do §7.5: a prova de que coletou (`dartforge_gc_estatisticas`, e `DARTFORGE_GC_STATS=exigir`, que sai com 70 sem coleta ou sem raiz lida de mapa), a coleta agendada por semente (`DARTFORGE_GC_AGENDA=<semente>,<taxa>`) e o veneno com quarentena (`DARTFORGE_GC_VENENO=1` ou `=<n>` blocos) | `RT/gc_raizes.rs`, `RT/nucleo.rs`, `RT/heap.rs` (`Agenda`), `RT/espaco.rs` (`Quarentena`) |
| os testes dirigidos D1, D2, D3, D4, D6, D7, D10, D11, D13 e D14, cada um com a saída calculada e, quando o §7.3 dá uma sabotagem implementável, o teste que exige a falha com ela; a conferência do E2.5 e a agenda | `corpus/nativo/gc_d*.dart`, `crates/cli/tests/mapas_dirigidos.rs` (com `DARTFORGE_TESTES_MAPAS=1`) |

Do veneno: o bloco morto recebe a palavra `0xDFDF_DFDF_DFDF_DFDE` (o estado lido é `0xDE`, que a
validação de handle recusa com "bloco envenenado") e só volta à lista livre depois que outros `n` blocos
morreram; a varredura completa trata o bloco em quarentena como ocupado (não o devolve e segura a página).
O veneno liga a validação de handle.

Não ficou: D5 (a sabotagem `undef` no lugar de `null` não existe no emissor), D8 e D9 (as sabotagens
"reduzir a folga" e "chamar Dart sem a porta"), D12 (o registro dos caches do runtime), e o contador por
extern do D7 (o D7 usa a sabotagem `folha:` sobre o D1). O verificador do IR do §7.4 não foi escrito.

**Etapa 3, E3.2: a decodificação preguiçosa.** O índice de cada imagem guarda, por função, o começo e onde
está a descrição dela (o fluxo DFGM ou o primeiro registro do `.llvm_stackmaps`); os registros de uma
função só são decodificados na primeira consulta a ela (`OnceLock`). A busca é pela função de maior começo
que não passa do endereço de retorno; o percorredor passa também o começo exato que o desenrolador conhece
(o `BeginAddress` do `.pdata`, o `_Unwind_GetRegionStart`), e uma função com mapa sem registro para o
endereço aborta (§14.5), com a regra do `nop` no Windows x64.

**Etapa 4, Linux e macOS (x86-64 e aarch64).**

* **O percurso:** `_Unwind_Backtrace`; o SP de um quadro no ponto da chamada é o CFA do quadro anterior
  do percurso (o chamado), o FP é o `x29`/`rbp` restaurado nele (`_Unwind_GetGR`), o começo da função é o
  `_Unwind_GetRegionStart`. O runtime passa a ser compilado com `-C force-unwind-tables=yes` nesses alvos
  (`EN/build.rs`), também nas variantes com `panic=abort`.
* **Linux:** o conversor reescreve o objeto ELF (`EN/gcmap.rs`, `converter_elf`): a seção vira `dfgcm`
  com `SHF_ALLOC | SHF_GNU_RETAIN`, alinhamento 4, e o índice passa a ter, por função, o endereço dela
  menos o do próprio campo (bandeira 1 do DFGM; `R_X86_64_PC32`, `R_AARCH64_PREL32`), sem relocação
  dinâmica no executável PIE. A conversão é obrigatória no ELF: o mapa do LLVM tem relocações absolutas de
  64 bits numa seção só de leitura, que o `ld.lld` recusa no PIE. O módulo registra a seção com
  `dartforge_registrar_mapa_secao(@__start_dfgcm, @__stop_dfgcm)`. A biblioteca compartilhada do SDK de
  desenvolvimento liga com `-Bsymbolic-functions` (a relocação relativa contra função exportada exige
  ligação local). A produção vai sempre pelo ThinLTO distribuído, que o `ld.lld` aceita
  (`--thinlto-index-only`, conferido na ajuda do 22.1.8), inclusive o programa de um módulo só; no
  bitcode da produção o passe dos mapas sai da pré-ligação e roda no fecho do ThinLTO distribuído.
* **macOS:** o `ld64.lld` 22.1.8 não tem `--thinlto-index-only` (conferido na ajuda), mas tem
  `--lto-newpm-passes`: a produção usa a LTO do ligador com `lto<On>,rewrite-statepoints-for-gc,verify`
  (`EN/ligador_macos.rs`), e o passe sai da pré-ligação do bitcode. O mapa fica no formato do LLVM
  (`__LLVM_STACKMAPS,__llvm_stackmaps`, com os endereços absolutos que o carregador ajusta), cada módulo
  com função `gc` leva `module asm ".no_dead_strip __LLVM_StackMaps"` para o `-dead_strip` não o tirar (a
  solução do Perry), e o módulo registra a seção com `dartforge_registrar_mapa_llvm` pelos símbolos
  `section$start$…`/`section$end$…`. Custo conhecido: o mapa no macOS tem o tamanho do formato do LLVM.
* **O `-ni:1`** entra também nos módulos sem cabeçalho (macOS, Linux aarch64), pelo gerador embutido,
  quando o módulo tem função `gc` (`crates/llvm`, `completar_alvo`).
* `alvo::raizes_por_mapas` aceita esses alvos.

**Não verificado** (nada foi executado): a fórmula do SP pelo CFA nos quatro alvos (§3.6 pede derivá-la por
teste); que o `.no_dead_strip` sobrevive à LTO do `ld64.lld`; que `lto<On>` vale em `--lto-newpm-passes` e
roda nos módulos do ThinLTO; que o `ld.lld --thinlto-index-only` aceita a linha de ligação inteira do
Linux; a codificação do `dfgcm` contra o `--icf=safe`; e tudo o que só a execução mostra.

**Etapa 4, o JIT (§3.8).** Com `DARTFORGE_RAIZES=mapas`, a sessão do `LLJIT` (`crates/jit/src/ffi.rs`):

* gera código com `CodeGenLevelLess` (nunca o FastISel do `-O0`);
* roda `rewrite-statepoints-for-gc,verify` em cada módulo pela camada de transformação de IR, e no
  objeto em cache (`compile_object`) antes da emissão;
* liga os objetos pela camada RTDyld (`LLVMOrcCreateRTDyldObjectLinkingLayerWithMCJITMemoryManagerLikeCallbacks`)
  com um gerenciador de memória próprio (`ObjetoNoJit`): uma região de 256 MiB de endereço por objeto, o
  código numa metade e os dados na outra; acha a seção do mapa pelo nome na alocação (`.llvm_stackmaps`,
  `__llvm_stackmaps`) e a registra no runtime depois das relocações (o formato do LLVM, com endereços
  absolutos); no Windows x64 registra o `.pdata` do objeto (`RtlAddFunctionTable`, com a base da região,
  que é o menor endereço das seções, como o RTDyld calcula a base da imagem), sem o que o percurso não
  atravessa um quadro do JIT; ao soltar a memória do objeto (a geração aposentada da recarga) tira o mapa
  (`dartforge_desregistrar_mapa`) e a tabela;
* registra no runtime que coleta: o do processo, ou o da biblioteca do SDK da fonte, pelas funções que ela
  exporta.

O módulo do JIT sai sem o `-ni:1` (o `LLJIT` recusa um módulo com camada de dados diferente da dele; o JIT
não roda otimização de IR) e sem a chamada de registro (`EN/llvm/mod.rs`, `mapas_no_jit`). As exceções por
tabelas continuam recusadas no JIT, como o §3.8 pede até um teste provar uma exceção atravessando dois
quadros do JIT. Não verificado: que a camada RTDyld da API C registra o `.eh_frame` no Linux e no macOS
(o gerenciador herda o `RTDyldMemoryManager`); o `CreateContext` é chamado com a assinatura do cabeçalho C
(o `llvm-sys` 221 declara o tipo sem o retorno, e o ponteiro de função é convertido).

**Windows arm64.** Não há AOT nativo nesse alvo: o cabeçalho do módulo no Windows fixa o triple
`x86_64-pc-windows-msvc` (`EN/alvo.rs`, `cabecalho_ir`). O percorredor por `RtlVirtualUnwind` com o
`CONTEXT` do arm64 fica para quando o alvo existir; até lá `alvo::raizes_por_mapas` o recusa, como o §6
pede.

**O conferidor depois do RS4GC (§7.4), escrito em 2026-10-05.** `crates/llvm/src/conferir_rs4gc.rs`,
ligado por `DARTFORGE_CONFERIR_RS4GC=1` em `gerar` e `gerar_de_bitcode` (o texto do módulo é impresso
depois do passe). Na forma do §14.8 o `Ref` é `i64` e só a raiz é `ptr addrspace(1)`, então o
conferidor não procura `ptr addrspace(1)` sem relocação (o RS4GC os reloca por construção): procura o
`%v<n>` usado depois de um statepoint, ou passado a ele (C1), sem a raiz `%raiz<n>` (ou uma relocação
dela) no `"gc-live"` desse statepoint. É o defeito que o otimizador causa ao esticar a vida de um `Ref`
por cima de uma chamada e que o emissor causa ao esquecer o uso fictício. Por bloco: o valor que
atravessa o statepoint para um `phi` de outro bloco não é conferido; só os pares com os nomes do
emissor entram (`%v5.i`/`%raiz5.i` do inlining contam). As violações plantadas estão nos testes do
arquivo. O Mach-O (o passe roda na LTO do `ld64.lld`) e o JIT não passam por ele.

**D5, D8 e D9 (§7.3), escritos em 2026-10-05.** Os programas `corpus/nativo/gc_d05_slot_antes_de_escrito.dart`
(saída `1729 140`), `gc_d08_estouro_de_pilha.dart` (`3003 0`) e `gc_d09_fronteira_rust.dart` (`925`),
com as saídas calculadas à mão, e as sabotagens:

| caso | modo | sabotagem | onde |
|---|---|---|---|
| D5 | `sombra` | `quadro_sujo`: o quadro nasce com 4098 (forma de handle, nenhum bloco) em vez de zero | `EN/llvm/mod.rs`, a abertura do quadro |
| D5 | `mapas` | `sem_uso_ficticio` | — |
| D8 | `mapas`, `checagem` e `tabelas` | `folga` (runtime): a folga da pilha de 4 KiB em vez de 256 KiB | `RT/gc_raizes.rs`, `limite_da_pilha_da_thread` |
| D9 | `mapas`, `tabelas` | `sem_porta` (runtime): as portas não são registradas e o runtime chama Dart direto | `RT/excecoes_tabelas.rs`, `dartforge_registrar_portas` |

O D9 usa `Function.apply`, que o runtime faz pela porta `dart_r3` (`RT/closures.rs`): a exceção do
closure sai certamente por um quadro Rust. O `then` que lança fica no programa, mas o tratador da
microtarefa é Dart e não prova a fronteira sozinho. O harness (`crates/cli/tests/mapas_dirigidos.rs`)
ganhou o campo `sombra` e liga o conferidor do RS4GC em toda compilação em modo mapas, inclusive nas
das sabotagens (onde ele pode ser o primeiro a acusar). Não feito: o callback da FFI que lança (pede
uma biblioteca C no teste). O `Isolate.exit` dentro de `finally` está escrito desde 2026-10-05
(`gc_d09_isolate_exit_em_finally.dart`, saída `870`, calculada à mão: a soma de `0² + … + k² + k`
para `k` de 0 a 9; mesma sabotagem `sem_porta`).

O D12 (cache estático do runtime como raiz): `gc_d12_cache_do_runtime.dart` (saída `1000 1000 4000`)
pede a cada volta o objeto `Type` canônico (`OBJETOS_TIPO`, raiz global permanente) e o texto de um
caractere (`UM_CARACTERE`, literal permanente), sem que o programa os segure entre as voltas. A
sabotagem `tipo_sem_raiz` (runtime, `RT/tipos.rs`) tira o registro da raiz: a volta seguinte recebe
do cache um handle de bloco liberado, que o veneno e a validação de handle recusam. Os demais caches
com handle levantados (`LITERAIS_POR_ENDERECO`, `DESPACHANTE`, `ATUAL` de `portas.rs`,
`CURRENT_STACK_TRACE`) seguem a mesma regra e não têm sabotagem própria.

## 15. Roteiro de implementação detalhado

Cada passo traz: arquivos e funções (linha atual), o que entra, o teste dirigido e a medida. A ordem é a
dos commits. Nenhum passo muda o padrão (`sombra` + `checagem`).

### 15.1 Etapa 1

**E1.0 — a chave e o marcador de ABI.**

* `EN/context.rs`: `pub enum ModoExcecoes { Checagem, Tabelas }` no `Context`; `EN/lib.rs`: a opção;
  `crates/cli/src/nativo.rs`: `--excecoes=checagem|tabelas`.
* `EN/alvo.rs`: recusa por alvo (só `x86_64-pc-windows-msvc` aceita `tabelas` na Etapa 1; os outros dão
  erro com a mensagem "`--excecoes=tabelas` ainda não existe para <alvo>").
* `EN/cache_objeto.rs`, `EN/sdk_modulo.rs`: o modo entra na chave do cache.
* Teste: `compile-native --excecoes=tabelas --emit-ir` num alvo recusado dá o erro; no aceito, por
  enquanto, IR idêntico ao de `checagem` (nada usa a chave ainda).

**E1.1 — a tabela de efeitos, ainda em `checagem`.**

* `crates/runtime/efeitos.tsv` (novo), gerado uma vez de `r2\efeitos\efeitos.tsv` e revisado.
* `crates/runtime/build.rs`: confere que toda extern `#[no_mangle]` tem linha; gera a tabela para o
  emissor.
* `EN/lower/fn_builder.rs:940-992` (`emit_call_with_check`): só emite a leitura da pendência quando o
  destino é função Dart ou extern com `lanca = 1`.
* `RT/`: o modo `DARTFORGE_EFEITOS=conferir` (§13.8).
* Teste: o corpus nativo inteiro com `DARTFORGE_EFEITOS=conferir`; a sabotagem do §13.8.
* Medida: número de leituras da pendência no IR do `new_sali/backend` antes e depois; `.text` antes e
  depois. É a medida que fica mesmo se a Etapa 1 for abandonada.

**E1.2 — o lowering.**

* `EN/hir.rs`: terminador novo `Invoke { chamada, normal, pouso }` e instrução `LandingPad`; ou, mais
  simples, um atributo `pouso: Option<BlockId>` nas instruções de chamada, que o emissor de IR baixa
  como `invoke`. A segunda forma não mexe nos consumidores da HIR e é a recomendada.
* `EN/lower/fn_builder.rs`: `emit_call_with_check` segue a tabela do §13.2; cria o bloco de pouso
  (`novo_pouso(alvo)`), que registra a aresta `(pouso, 2, default_ret)` no `finally_scopes` quando o
  alvo é um `finally`.
* `EN/lower/comandos.rs:579-861`: sem mudança de lógica; os "relança" sem tratador externo terminam em
  `Lancar` em vez de `Return`.
* `EN/llvm/mod.rs`: emissão de `invoke`/`landingpad`, do `store` do topo no pouso, de `@df.lancar`, das
  oito portas e do `personality` nas funções com `invoke`; o prólogo de estouro de pilha
  (`:2456-2481`) chama `@df.lancar` depois de `dartforge_estouro_de_pilha`.
* `EN/lower/verificador.rs`: todo bloco de pouso tem um só predecessor, que é um `invoke`; nenhuma
  função sem `invoke` tem personalidade; em `tabelas`, nenhum `Return` é alcançável só por caminho de
  exceção.
* Testes dirigidos (cada um um programa do `corpus/nativo`, comparado com a VM): os cinco de
  `ir-hoje\exc.dart`; `laco`/`aninhado` do §13.5; exceção atravessando 1.000 quadros sem `try`; `throw`
  dentro de `catch` dentro de `try`; `finally` que lança outra exceção; `return` dentro de `finally`;
  `try` em função sem quadro de raízes; `try` em laço com alocação no `catch` sob `--gc-stress`.

**E1.3 — a personalidade.**

* `RT/excecoes_tabelas.rs` (§13.11), só Windows x64 nesta etapa.
* Testes de unidade de `achar_pouso` com as LSDAs reais do `x2` (os bytes do `.xdata` de `b_catch`,
  `c_cleanup`, `d_aninhado`, copiados para o teste) e com entradas truncadas.
* Teste de exceção estrangeira: um programa que faz FFI para uma função C que provoca violação de
  acesso dentro de um `try` Dart tem de morrer com o código do sistema, não entrar no `catch`.

**E1.4 — as portas.**

* `RT/chamar_dart.rs` e os 14 pontos da §13.9; `EN/lower/ffi.rs` e `RT/ffi_callbacks.rs`: a entrada C de
  um callback FFI vira uma porta.
* CI: o `grep` que recusa `transmute` para `extern "C" fn` fora dos lugares permitidos.
* Testes: exceção não tratada em microtarefa, em timer, em mensagem de porta, em isolado filho, em
  `noSuchMethod`, em comparador de `sort` chamado por `Function.apply`, em callback FFI; em todos, a
  saída e o código de saída iguais aos da VM, e o topo da pilha-sombra nulo no fim
  (`assert` em `desempilhar_quadro`, `RT/heap.rs:379-385`).

**Fechamento da Etapa 1.** As medidas do §8 nas duas combinações (A0, A1) e a decisão pelos critérios
do §6: `.text` + `.xdata` + `.pdata` do `new_sali/backend` pelo menos 3% menor; `bench/desempenho` sem
piora; o lançar-e-pegar dentro do limite.

### 15.2 Etapa 2

**E2.0 — funções de conversão no emissor** (§14.1). Critério: IR idêntico em `sombra`.

**E2.1 — experimentos.** Feitos: (a) a seção sai no COFF, com o nome `.llvm_stackmaps` no objeto e
`.llvm_st` na imagem; (d) `landingpad token` com `invoke` de statepoint e personalidade própria no
`x86_64-pc-windows-msvc` roda (`x7`). Restam: (b) não é mais necessário (§12.3); (c)
`statepoint-max-registers-for-gc-values` no 22.1.8 — nos casos rodados nenhum local `Register` apareceu,
e o conversor recusa se aparecer; o valor padrão da opção não foi lido na fonte.

**E2.2 — emissão em `mapas`.** `EN/llvm/mod.rs`, `EN/llvm/externs.rs` (o atributo
`"gc-leaf-function"` pela tabela de efeitos), `EN/alvo.rs` (`-ni:1`), `EN/llvm/raizes.rs` (não emite
slots; mantém a análise para o verificador). O verificador do IR do §7.4: nenhum `inttoptr` fora dos três
casos; todo operando `ptr addrspace(1)` de chamada não folha no `gc-live` (conferido **depois** do
RS4GC).

**E2.3 — pipeline em processo.** `EN/gerador.rs`, `crates/llvm`: a cadeia
`default<O2>,rewrite-statepoints-for-gc,verify` e a emissão do objeto. Medir as duas ordens.

**E2.4 — o conversor.** `EN/gcmap.rs`: porte de `x12\gcmap.py` sobre o crate `object` (COFF primeiro).
Testes: ida e volta em todo objeto do corpus; os erros de "local recusado" com objetos fabricados.
Nota: o §6 previa o mapa v0 "sem compactação" primeiro; como o DFGM v1 já roda no `x12`, o passo pode
ir direto a ele.

**E2.5 — o percorredor.** `RT/gc_mapas.rs` (§14.5, §14.6), `RT/gc_raizes.rs` (a fonte "quadros do
código gerado" passa a ser o percorredor em `mapas`). `DARTFORGE_GC_PERCURSO=conferir` não tem segundo
percorredor no Windows x64; lá a conferência é contra a **pilha-sombra**: um build de teste que emite as
duas coisas (mapa e slots) e exige que o conjunto de handles visitados pelos mapas contenha o dos slots.

**E2.6 — o quadro-sombra residual** (§3.7), para as funções que o modo mapas recusa.

**Fechamento da Etapa 2.** Corpus nativo em B0 com e sem `--gc-stress`; as três sabotagens do §14.7
falham; `bench/desempenho` nas quatro combinações; a decisão pelos critérios do §6.

### 15.3 O que este roteiro não cobre

* Linux, macOS e arm64 (Etapa 4): a personalidade Itanium está escrita no §13.11 e não foi executada.
* ThinLTO com statepoints (Etapa 3): `x10` não concluído.
* O JIT em `tabelas` ou `mapas`.
* A máquina de estados `async` foi tratada só pela regra geral (§13.13).
