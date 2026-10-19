# Desempenho e tamanho do AOT nativo: diagnóstico e correções (2026-10-18)

Investigação pedida pelo proprietário: por que o executável AOT do DartForge é bem maior e
mais lento que o `dart compile exe` em aplicações reais (servidor HTTP, JSON, alocação de
objetos, árvores, kernels do dart_ui), e corrigir o compilador priorizando desempenho,
preservando a semântica do Dart e reduzindo o tamanho onde possível.

Método: sete investigações independentes por dimensão (alocação, prólogo/chamadas, despacho,
JSON/strings, HTTP/async, tamanho, representação numérica), cada uma verificada por um
revisor cético contra o código em `9fcaa9a0`, com IR (`compile-native --emit-ir`), código de
máquina (`otool -tv` de builds `--optimize --debug`, idênticos aos de produção fora os
símbolos), perfis `sample`, o mapa de ligação (`DARTFORGE_LINK_MAP=1`), o relatório da poda
(`DARTFORGE_WHY`) e a desmontagem do snapshot upstream pelo REA/Ghidra já registrada em
`bench/results/2026-10-14-rea-*`. Em seguida, quatro implementadores em arquivos disjuntos,
cada um revisado por um cético e corrigido quando necessário, e a validação final pelo
coordenador (corpus nativo contra a VM, com e sem `--gc-stress`, A0 e A1, benchmarks pareados).

## 1. Linha de base no HEAD (macOS arm64, Mac mini, máquina quieta)

`bench/comparison/bench.dart`, mínimo das rodadas (ms):

| núcleo | A0 (padrão) | A1 (`--exceptions tables`) | Dart 3.6.2 AOT |
|---|---:|---:|---:|
| fib | 7 | 14 | 9 |
| trees | 21 | 25 | 12 |
| numeric | 96 | 96 | 95 |
| strings | 72 | 73 | 44 |
| json | 100 | 106 | 69 |

Tamanhos: bench A0 3.368.048, A1 3.318.224, Dart 5.379.136 bytes. Servidor HTTP
(`server.dart`, 1 conexão keep-alive, `/json`): 23,8 ms de CPU de usuário por mil
requisições contra 15,2 do Dart (1,57×); a parte de kernel é igual ou menor (12,3 vs 14,4).
Kernel de cor do dart_ui (`color_numeric_performance.dart`): 74,7 ms contra 5,1 (14,7×).
File Explorer do dart_ui: 28.293.424 bytes contra 11.682.880 (2,42×), com 25,09 MB de
`__text`.

## 2. Onde está o problema (verificado)

O alcance do mundo fechado **não** é o problema do tamanho: o File Explorer tem ~9,9 mil
corpos vivos, igual ao snapshot upstream (9.828). O problema é a **forma do código por
operação** (2.828 corpos casados pelo nome ocupam 7,80 MB aqui contra 1,61 MB lá, 4,85×;
funções pequenas 8,5×) e, no tempo, o **custo fixo por chamada, por alocação e por chamada
dinâmica** que o Dart AOT não paga.

### 2.1 Prólogo e chamadas

- Toda função lê o contexto da thread (`mrs TPIDRRO_EL0` + slot pthread + teste de nulo, 9
  instruções e um desvio, mais um bloco frio `@df.context.slow` por cópia inline: 4.867 no
  bench), confere a pilha com um `alloca` marcador (4 instruções e 16 bytes de quadro), abre o
  quadro da pilha-sombra quando há `Ref` vivo (≈9 + 2 por raiz + 2 por saída) e confere a
  exceção pendente depois de cada chamada (2–3). `fib`: 35 instruções por chamada contra 23 da
  VM; `Node.count` 172 contra 55 na comparação REA.
- **`--exceptions tables` (A1) estava 2× mais lento em `fib`** não pelo modelo de tabelas,
  mas porque `with_context_in_tls` excluía `table_exceptions` (`lib.rs:602`,
  `sdk_module.rs:393`): cada prólogo chamava a função Rust `dartforge_context`
  (`thread_local!` + `_tlv_get_addr`), 57% + 15% do tempo de `fib` no perfil. Em A1 o caminho
  feliz já não tem `invoke`/`landingpad` (só nas portas `df.port.*`).
- Em A0 a checagem após chamada é estruturalmente irremovível: `effects::in_cycle` marca toda
  função recursiva como lançadora (`StackOverflowError` do prólogo) e a propriedade sobe aos
  chamadores; só 112 de 4.181 funções de `dart:core` são folhas.

### 2.2 Alocação e layout

- A alocação em linha já é um bump de TLAB, mas cada sítio confere o **registro preguiçoso da
  tabela de métodos** (`ctx.n_registered > cid && registered[cid]`: 3 loads + 2 desvios), grava
  um zero morto no primeiro campo e cada gravação de campo faz **leitura-modificação-escrita do
  mapa de referências** no cabeçalho (3–4 instruções), embora o bit seja estático por
  (classe, índice) para as classes nominais. No IR do File Explorer: 174.351 RMW do mapa,
  14.071 alocações com a conferência de registro, 93.763 sequências de endereço de campo com
  `select` de null/Smi e teste do bit de corpo de fora (que só existe na migração do JIT).
- **As alocações feitas pelo próprio runtime (strings, listas, typed data, `_index`/`_data` de
  `_Map`) nunca usam a TLAB**: `Heap::allocate_block` paga `needs_collection`, contagem,
  `take_from_region(c, 1)` com `memset` por objeto, `young_set.push` e, na varredura,
  `release_range` por objeto. É a maior fatia dos perfis: 44% de `int.toString`, 25% do literal
  de mapa, 16,5% do kernel `strings`, 12,8% do `json`. A construção de strings valida o handle
  cinco vezes por string (`new_string` → `write_string` → `one_units_mut` → `words` →
  `words_mut`) e copia ≤ 20 bytes por `_platform_memmove`.
- A coleta menor é barata nas árvores pequenas (8% dos objetos marcados), mas o limite jovem é
  por contagem (`YOUNG_COUNT` = 256 mil alocações ≈ 8 MiB) e estruturas acima de 2 MiB em
  construção são marcadas (com envelhecimento, duas vezes): numa árvore de 4 MiB a coleta é
  30% + 14% do tempo; `YOUNG_KB=16384` + `AGING=0` deu −37% nesse kernel.

### 2.3 Despacho e RTI

- O DartForge só devirtualiza um membro do SDK quando a **biblioteca** o fecha (`closed_member`:
  nome privado, classe privada, `final`/`sealed`), nunca pelo mundo fechado do programa:
  `Map`, `List`, `Iterable`, `Iterator` são interfaces públicas, então `m['id']`,
  `list.length`, `moveNext()` passam pelo cache em linha `@df.selector` (~16 instruções no
  acerto + 11 na falha, 2 palavras da área por sítio; 2.290 sítios no bench, 34.598 no File
  Explorer) e pela entrada uniforme `$c`/`$tc` com vetor de argumentos na pilha, prólogo
  completo, conferência de aridade e **duas verificações covariantes por `[]=`**
  (`_Map.[]=$c`: 408 instruções). A prova de receptor exato (`; df.selector-exact 151`) existe
  e só é usada pela poda.
- `as int`/`String`/`bool`/`List` passam pelo bitmap `df.subclass` (~20 instruções) em vez de
  um teste de etiqueta/cid, e `as Map<String,dynamic>` é chamada **incondicional** a
  `dartforge_rti_as_with_context` (661 instruções) por elemento, sem o cache em linha que o
  `is` já tem. `dartforge_rti_set` (397 instruções, `RefCell`) é chamado por literal de
  coleção.
- A devirtualização por `switch` sobre todas as implementadoras não tem teto e o inliner copia
  cada braço: `RenderObjectElement.performRebuild` tem 22.160 bytes contra 140 no upstream
  (137 braços); 84 funções com fan-out ≥ 6 somam 772 KB.

### 2.4 JSON, strings e HTTP

- O literal de mapa monta uma lista temporária pelo runtime, insere entrada a entrada por
  seletor (`_dartforgeFillMap` → `_Map.[]=$c`) e a primeira inserção cai no caminho genérico
  `_set` → `_rehash` → `_init`: 450 ns contra 140 (3,2×). A interpolação `'k$i'` custa 43 ns
  contra 23 (98% dentro de `dartforge_string_join_typed`, pela alocação do runtime).
  `jsonEncode` 1,4× (`_sink.write` por seletor → `StringBuffer.write$c`, `List.filled` +
  closure por mapa, `int.toString` com string intermediária); `jsonDecode` já está a 1,1×.
- No servidor HTTP a E/S é igual à do upstream (um `read`, um `write`, `kqueue` em
  `EV_CLEAR`); toda a diferença é código de usuário: RTI (5,8%), alocação de `Uint8List(8192)`
  (3,8%), literais de lista pelo runtime (2,3%), `GrowableList_reserve` (2,7%), coleta (2,4%),
  raízes de estáticos por `HashMap` (0,9%), leituras de relógio (1,4%); a máquina async do
  overlay (modelo dart2js) aloca 7 objetos por ativação e 5 por `await`, 2,4× a VM num
  micro-benchmark.
- **Dois defeitos de paridade com `dart compile exe`**: os `assert` são compilados e
  executados no AOT de produção (≈1.400 sítios só no SDK em cache) e `dart.vm.product` não é
  definido; e um `async` que retorna sem suspender completa o futuro numa microtarefa, enquanto
  a VM completa sincronamente (a ordem observável de microtarefas diverge).

### 2.5 Representação numérica (kernel de cor 14,7×)

- `num` é sempre `Ref`: cada operação encaixota (`_Double` por iteração), reclassifica
  (`df.class` + `switch`) e desencaixota com conferência de exceção. A cadeia de `num.clamp`
  do SDK (`_Double.compareTo` → `this < other` **por seletor**) soma ~39%; as 15 chamadas por
  iteração aos **getters preguiçosos das constantes de enum** (um por `case` do `switch`
  linear) somam ~31%; `from == to` vai por `Object.==` → `Heap::identical_one` (4,5%).
  As chaves opt-in já existentes (`DARTFORGE_PRIMITIVE_CLAMP`, `_MULTIPLY`,
  `_CONSTANT_SPECIALIZATION`) baixam 74,7 → 37,6 ms, ainda 7,4×.
- Os operadores intrínsecos do SDK carregam `@pragma('vm:never-inline')`, honrada como
  `noinline`, o que impede o inliner HIR e o LLVM de absorvê-los.

### 2.6 Tamanho: o que não é expansão por função

- **Falta `@pragma('vm:platform-const')`**: os backends win32/x11/wayland/vulkan/gl/d3d11/d3d12/
  android/ios do dart_ui ocupam 4.593.644 bytes (16% do File Explorer) e zero bytes no
  snapshot upstream, porque a VM dobra `Platform.operatingSystem` e o TFA propaga o argumento
  constante `operatingSystem ?? Platform.operatingSystem` até `platform == 'windows'`.
- Código que deveria ser dados: getters de constantes 785 KB, getters RTI 412 KB (3.786
  funções de 112 bytes), `df.register.program` 200 KB (16.929 chamadas), stubs `df.mt` 49 KB.
- Ligador: `--icf=safe` daria −1,02 MB mas **quebra a identidade de tear-offs** (corpus 80;
  `safe == all` porque os objetos LTO e Rust não têm `__llvm_addrsig`); `-mllvm
  -enable-merge-functions` dá −293 KB preservando o corpus 80, e `-no_function_starts` −146 KB.
  O runtime Rust é só 1,17 MB (não 3,4 MB: a atribuição anterior somava fragmentos do outliner
  e código gerado). O machine outliner já substitui ~20,7 MB de sequências repetidas por
  1,29 milhão de `bl` (20,5% do `__text`).
- O File Explorer (34.333 funções HIR) fica abaixo do limiar de "programa grande" (50.000) e
  não recebe `optsize` nem o seletor compacto.

### 2.7 Correções trazidas pelos dois últimos veredictos (JSON/strings e tamanho)

Chegaram depois do lançamento da onda 1 de implementação, portanto os implementadores
não as receberam; entram numa onda 1.5 logo após a validação da onda 1.

- **Bug de semântica pré-existente nos literais de mapa e lista**: as entradas de tipo
  estático `dynamic` não recebem o downcast implícito para `K`/`V`/`E` que a CFE insere.
  Verificado contra `dart compile exe`: `<String,int>{'a': 1, 'b': s}` com `dynamic s = 'x'`
  imprime `{a: 1, b: x}` no DartForge e lança `_TypeError` na VM; `{d: 2}` com `int d` idem.
  Causa: `fill_map_at_once`/`append_entry` (`lower/literals.rs`) usam `lower_expr` sem
  `cast_rti_in(..., CastContext::Implicit)`, e o `rti_set` vem depois do preenchimento. É
  pré-requisito para usar `_dfFillFromRange`/`unsafeCast` no literal em lote.
- **`StringBuffer` não é classe fechada** no overlay (`class StringBuffer {`, sem `final`):
  um campo `StringBuffer? _buffer` no encoder JSON continuaria a chamar `write` por seletor.
  A saída é uma classe privada `final` do próprio `dart:convert` (`_JsonBuffer`) sobre os
  natives `DartForge_sb_*`, usada por `stringify`, com `_partialResult` por `toString()`.
- **Não escrever direto em `List<Object?>`** (`write_op = Some(Ref)`): `List<Object?> l =
  <int>[1]; l[0] = 'x'` deve lançar `_TypeError`, e lança hoje na VM e no HEAD.
- **`--icf=safe` no macOS é viável**: a falha anterior do corpus 80 vinha de cache ThinLTO
  contaminado (objetos antigos sem `__llvm_addrsig`). Com subdiretório próprio do cache, o
  ICF seguro preserva a identidade dos tear-offs e rende −737.968 bytes no File Explorer
  (o −1,02 MB citado era o de `--icf=all`). Risco residual a auditar: objetos Rust sem
  `__llvm_addrsig` são dobrados pelo `ld64.lld` em modo `safe` (ao contrário do ELF/COFF).
- Toda medição de religação que muda opções de LTO/ICF/`-mllvm` precisa de cache ThinLTO
  novo (`DARTFORGE_NO_THINLTO_CACHE=1` ou subdiretório), senão os números saem contaminados.
- **Não** retirar as tabelas de unwind do runtime: `build.rs` força `-Cforce-unwind-tables=yes`
  porque A1/B0/B1 atravessam quadros do runtime; a mesma `rtprod` serve os quatro modos.
- O teto da devirtualização por `switch` deve ser medido (4, 8 ou 16): num sítio megamórfico o
  cache em linha monomórfico falha sempre e cai na busca do runtime, enquanto o `switch` denso
  vira jump table; o custo medido é o próprio `switch` (≈160 B por braço), não cópias do inliner.
- Estimativas de "código → dados" revistas: getters RTI −0,25 a −0,33 MB (não −0,40),
  registro do programa como tabelas ≈ −70 a −90 KB (o ganho maior é partida), stubs `df.mt`
  −49 KB; constantes canônicas na imagem −0,5 a −0,7 MB continua plausível.
- Parcela não atacada por nenhum achado: adaptadores `$c/$tc` 2,29 MB, `$g/$s/$ts` 0,70 MB,
  `$tear/$tearm` 0,52 MB, `$ent` async 1,0 MB e corpos `$async` 0,3 MB no File Explorer
  (≈ 4,8 MB, 19% do `__text`); e o perfil `strings` tem 18,8% nos próprios natives de sonda
  (`hash_map_lookup` 11,8%, `hash_map_store` 7,0%), não só no `$c`.

## 3. O que foi corrigido nesta rodada

O lote medido também inclui as implementações herdadas: contexto TLS em A1, prólogo sem
marcador `alloca`, registro inicial das tabelas, mapas de referências estáticos e endereços
de campo simplificados; TLAB compartilhada entre código gerado e alocações do runtime,
construção e formatação de strings mais diretas; cache nos casts RTI, testes por cid e
operações intrínsecas adicionais; flags de produção e constantes de plataforma; literal
de mapa em lote e acumulador próprio no JSON; ajustes de inlining e ligação. Essa relação
é um inventário do lote, não uma alegação de validação isolada de cada otimização.

### 3.1 Retomada e correções de integração (2026-10-18)

A retomada encontrou o lote anterior ainda sem commit em `main`, com alterações no
compilador, runtime e overlays. A existência dessas alterações **não equivale à aprovação da
onda 1**: a validação abaixo distingue o que foi executado das pendências encontradas.

- **Downcasts implícitos nos literais:** o lowering agora usa os argumentos de tipo
  inferidos do literal para conferir cada expressão `dynamic` antes de avaliar a seguinte.
  Isso cobre listas diretas, o mapa em lote, conjuntos e elementos `if`/`for`, incluindo
  argumentos genéricos e tipos vindos do contexto. O corpus novo
  `145_literal_implicit_casts.dart` verifica tanto os erros quanto a ordem dos efeitos,
  casos válidos, nulabilidade e a proteção covariante de `List<Object?>`. O caminho de mapa
  em lote já existente passa a receber entradas conferidas. Não foi introduzida escrita
  direta genérica em `List<Object?>`.
- **Propagação e simplificação de strings:** uma string materializada por `Const` agora
  permite dobrar o teste contra `null`. A simplificação de igualdade entre strings também
  roda no fluxo normal de limpeza e antes da poda, pois o inlining pode eliminar o parâmetro
  antes da propagação interprocedural. Os locais do corpo especializado são promovidos
  para SSA antes de dobrar as condições: no lowering real, `platform` ainda ocupava um
  slot de pilha, caso não representado pelo teste sintético original. Dois testes do lote anterior que tentavam comparar
  `Terminator` sem `PartialEq` foram corrigidos para usar casamento de padrões. A verificação
  da poda considera corpos executáveis: o catálogo de ids de classe continua no IR como
  comentário mesmo para uma classe morta.
- **Regressão de recursão causada pela leitura de `sp`:** a primeira passagem do corpus
  encontrou timeout em `134_estouro_de_pilha.dart`, em A0 e A1. Um IR mínimo confirmou que
  LLVM 22 transformava recursão de cauda em laço e içava a checagem de pilha. As funções
  emitidas agora recebem `"disable-tail-calls"="true"`, preservando o crescimento da pilha
  e o `StackOverflowError` capturável. O teste do emissor verifica esse atributo juntamente
  com a leitura de `sp`/`rsp`; o corpus 134 é a porta comportamental.
- **Certificação numérica:** o catálogo das fontes revisadas ainda continha os hashes
  anteriores de `async_patch.dart` e `convert/json.dart`, fazendo a certificação recusar
  inclusive o SDK selecionado pelo próprio projeto. Após conferir essas duas mudanças,
  somente suas entradas foram atualizadas; a impressão do grafo de `dart:core` permaneceu
  igual. Os testes que alteram fontes ou expõem observadores de identidade continuam
  exigindo recusa da otimização.

### 3.2 Evidência e limites da validação

Artefatos em `bench/results/2026-10-18-aot-continuation/`, sobre `9fcaa9a0` mais o lote
local sem commit, usando Dart 3.6.2 e Clang 22.1.8 para macOS arm64.

- Runtime: **317 aprovados**, três microbenchmarks ignorados.
- Emissor LLVM: **33 aprovados**; propagação de argumentos constantes: **5 aprovados**;
  CFG antes da poda: **3 aprovados**; identidade de double: **4 aprovados**, um gerador
  manual de catálogo ignorado.
- Otimizador: **451 aprovados e 3 falhas**. Permanecem
  `ordinary_late_generic_notifier_dispatch_prepares_without_erasing_field_guards`,
  `ordinary_simple_enum_lowering_certifies_current_getter_and_rejects_enhanced_shape` e
  `actual_color_context_reports_post_prune_specialization_eligibility`. Elas já apareciam
  na primeira execução e impedem dar a onda 1 inteira como aprovada. Não foram convertidas
  em sucesso nem tiveram as exigências de prova afrouxadas.
- Suíte do backend com Clang explícito: **760 aprovados, 4 falhas e 9 ignorados**.
  Além das três acima, falha
  `actual_literal_clamp_uses_selected_sources_and_preserves_fallback_checks`: a premissa
  ainda exige `State::Runtime` para `Double_doubleFromInteger`, promovido a intrínseco pelo
  lote anterior. Os dois testes de multiplicação primitiva passam. A execução integral
  antecede o último ajuste de promoção de locais, validado separadamente pelos cinco
  testes de propagação. Na fixture real de plataforma, somem os construtores e
  tear-offs de Windows/Linux, mas seus getters `name` e stubs de tabela permanecem vivos
  pelo seletor genérico; a saída é `macos backend`, `posix separator`, `1`. O teste
  `platform_constant_prunes_the_other_backends` ainda não passa: a poda completa dos
  métodos dos backends inativos continua pendente, além das quatro falhas unitárias.
- A primeira passagem selecionou 16 programas, compilados em A0 e A1 com `--optimize
  --enable-asserts`, cada executável com e sem `DARTFORGE_GC_STRESS=1`. Foram **60 execuções
  iguais à VM e dois timeouts** no corpus 134, antes da proteção contra chamadas de cauda.
  O perfil usa `dart.vm.product=true`; o oráculo é `dart run --enable-asserts`, portanto
  essa seleção não substitui testes específicos das flags de produção/desenvolvimento.
- A repetição final da seleção, com o último compilador, passou em **64/64 execuções**:
  16 programas × A0/A1 × com/sem GC stress. Os dois timeouts do corpus 134 desapareceram.
  `targeted-corpus.initial.json` preserva a primeira passagem; `targeted-corpus.json`
  registra a repetição final. Essa seleção não é o corpus nativo completo.
- Após a proteção de recursão, os corpus **134 e 145 passaram nas oito execuções**
  (A0/A1 × com/sem GC stress), incluindo os casos genéricos válidos adicionados ao 145.
- `cargo doc` dos crates afetados concluiu, com dois avisos Rust já existentes.
  `cargo fmt --all -- --check` encontrou diferenças também fora do lote; Clippy com
  `-D warnings` falhou em avisos do runtime e frontend. `git diff --check` passou.
- Uma execução inicial dos testes do backend usou o Clang padrão do ambiente e foi
  interrompida; não conta como validação. As execuções subsequentes relevantes apontam
  explicitamente para o Clang 22.1.8 do projeto.

## 4. Medições depois das correções

Executáveis finais reconstruídos com `--optimize`, incluindo os downcasts, a proteção
contra eliminação recursiva e a promoção de locais na propagação de constantes. Cinco
rodadas externas alternadas de A0/A1/Dart 3.6.2, sem compilações ou testes concorrentes.
Cada execução do bench faz quatro amostras por núcleo: a tabela mostra o mínimo de 20
amostras por variante, em milissegundos. Todos os checksums coincidem.

| núcleo | A0 (padrão) | A1 (`--exceptions tables`) | Dart 3.6.2 AOT |
|---|---:|---:|---:|
| fib | 5 | 4 | 9 |
| trees | 19 | 19 | 11 |
| numeric | 95 | 95 | 95 |
| strings | 44 | 44 | 44 |
| json | 71 | 71 | 70 |

O lote completo reduz, em relação à linha histórica da seção 1, `strings` de 72 para
44 ms e `json` de 100 para 71 ms em A0; `fib` de A1 cai de 14 para 4 ms. A comparação
entre o compilador anterior e o novo **não foi alternada**: esses deltas são históricos;
a alternância desta rodada compara os executáveis finais com o Dart upstream. Não há
ablação que atribua o ganho individualmente a cada alteração.

Tamanhos do bench: **A0 3.162.704 bytes**, **A1 3.261.872 bytes**, Dart **5.379.152 bytes**.
Contra os valores históricos, A0 encolhe 205.344 bytes (6,1%) e A1 56.352 (1,7%). O bench
A0 fica 41,2% menor que o executável Dart desta medição. Isso não é uma nova medida do
File Explorer.

Kernel de cor do dart_ui (`color_numeric_performance.dart`, configuração de pacotes do
projeto, um milhão de iterações): cinco rodadas alternadas A0/Dart, com cinco amostras
internas cada. Mínimos: **29.246 µs (29,25 ms)** no DartForge e **5.287 µs (5,29 ms)** no
Dart, ainda **5,53×** mais lento. O mínimo histórico era 74,7 ms; a redução histórica é de
60,8%, sem atribuição por alteração. Os checksums também coincidem.

Dados brutos e método reproduzível: `performance.json`, `measure.py`, `bench.*.out` e
`color.*.out` em `bench/results/2026-10-18-aot-continuation/`. HTTP e File Explorer continuam
sem novas medições nesta retomada.

## 5. O que fica para as próximas rodadas

Antes de considerar a onda 1 concluída, resolver as quatro falhas unitárias e a falha da
fixture de poda listadas na seção 3.2 e executar o corpus completo, além da seleção abaixo.
HTTP e File Explorer ainda precisam de novas medidas pareadas; os valores da seção 1 não foram substituídos por
estimativas.

### Onda 1.5 (logo após a validação da onda 1)
1. Downcast implícito de entradas `dynamic` em literais de mapa/lista: implementado nesta
   retomada, com o corpus 145. Ampliar a auditoria para spreads e elementos null-aware antes
   de generalizar a cobertura a todas as formas de literal.
2. O JSON herdado já usa um acumulador próprio com chamadas diretas aos natives
   `DartForge_sb_*`, em vez de um campo `StringBuffer`. Auditar o IR e os caminhos de erro
   antes de considerar encerrada a alternativa ao sink privado `final` (`_JsonBuffer`).
3. `--icf=safe` no macOS com subdiretório próprio do cache ThinLTO e o corpus 80 como porta.
4. Teto de fan-out medido (4/8/16) em `calls.dart` e no dart_ui.

### Onda 2 (ordem sugerida)
1. `!invariant.load` no comprimento em `@df.string_len` (içamento fora de laços).
2. `dartforge_exception_clear` em `break`/`continue` só dentro de regiões `try`/`catch`/`finally`.
3. Raízes de campos estáticos pela varredura dos slots `Ref` da área (sem `HashMap` por store).
4. Política jovem: limite por bytes até 16 MiB (hoje `YOUNG_COUNT` ≈ 8 MiB), envelhecimento
   adaptativo; medir RSS (árvore de 4 MiB: −37% com `YOUNG_KB=16384` + `AGING=0`).
5. Getters RTI → helper compartilhado; registro do programa como dados; stubs `df.mt` como dados.
6. `rti_set` em linha com a receita da classe concreta (`_Map<K,V>`), não da interface.
7. Devirtualização por hierarquia sobre classes vivas (cone na `CallSelector`) e proveniência de
   retorno através do SDK nos resumos `.poda`.
8. Adaptadores `$c/$tc` só para seletores com sítio dinâmico; máquina `$ent` async.
9. Constantes canônicas como objetos da imagem `__dfimg`.
10. `DARTFORGE_LARGE_PROGRAM=1` para forçar o perfil grande e medir `optsize` no File Explorer;
    perfil por volume de IR.
11. Outliner compatível com A1 (hoje desligado com unwind: File Explorer A1 37,2 MB) e stubs
    que desenrolam para `noSuchMethod`/closure inválida, se A1 virar padrão.
12. Estruturais: async no modelo da VM, `num` desencaixotado local, contexto em registrador,
    tabela global de despacho.
