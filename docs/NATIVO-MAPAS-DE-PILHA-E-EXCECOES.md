# Mapas de pilha e exceções por tabelas no nativo: especificação da escolha chaveável

Escrito em 2026-10-02. Especificação; nada aqui está implementado.

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

**Seções pendentes de escrita:** nenhuma. O documento está completo como especificação. A §3.9 (colocação
tarde, como no Julia) está registrada como alternativa, fora do escopo.
