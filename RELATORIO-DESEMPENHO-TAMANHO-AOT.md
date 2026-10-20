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

## 3. O que foi corrigido

A onda 1 planejada aqui foi interrompida pelo limite de sessão; boa parte dela entrou no
`main` por outra sessão entre 18 e 20/10 (contexto em linha em todos os modos Unix,
asserts desligados e `dart.vm.product` em produção, mapa de referências na alocação,
registro em lote das tabelas, checagem de pilha por `sp`, `-enable-merge-functions` e
`-no_function_starts`, dobra de `Platform.operatingSystem` e propagação de argumentos
constantes, downcasts dos literais). Em 20/10 restava a pendência registrada no ESTADO: o
teste `platform_constant_prunes_the_other_backends` falhava com seis definições estrangeiras.

### 3.1 Poda das tabelas de classes do programa sem instância (`9eb257dc`)

**Causa:** a propagação já deixava só `MacBackend` instanciado, mas `df.register.program`
registrava a tabela de métodos de toda classe do programa na partida, e o registro é raiz da
poda: a tabela mantinha vivos os getters e `toString` de `WindowsBackend`, `LinuxBackend` e,
no dart_ui, de todos os backends win32/x11/wayland/vulkan/gl/d3d11/d3d12/android/ios. A poda
fina por classes existente (`DARTFORGE_CLOSED_CLASS_REACHABILITY=1`) exige provar as
alocações do SDK e recusa já em `df.register.dart:_internal`.

**Correção** (`pruning/class_reachability.rs`): uma classe declarada numa biblioteca do
programa só ganha instância por uma alocação do próprio módulo que cita a sua tabela; o SDK
não a nomeia, e o runtime só copia instâncias existentes (mensagens entre isolados) ou cria
structs FFI (mantidas). A tabela dessas classes passa a ficar viva só se uma alocação viva a
citar; as demais (SDK, runtime, records, quadros async, FFI) continuam registradas na
partida. Os braços dos `switch` de devirtualização para classes sem instância são cortados
(com o `default` redirecionado quando só servia a elas), até um ponto fixo. Fora de ARC e
quando alguma alocação tem id de classe não constante. Ablação:
`DARTFORGE_NO_PROGRAM_CLASS_PRUNING=1`.

| File Explorer, dart_ui `15aff70`, macOS arm64 | bytes |
|---|---:|
| mesmo compilador, poda desligada | 25.241.760 |
| poda ligada | 19.487.024 (−22,8%) |
| com as duas correções desta rodada | 19.470.512 |
| `dart compile exe` 3.6.2 | 12.257.440 |

A razão contra o Dart caiu de 2,06× para 1,59×. O explorador AOT abriu, renderizou dois
quadros e fechou, e o smoke do menu de contexto passou (`scripts/validate-native-desktop.py
--mode aot`).

### 3.2 Chamadas a funções que nunca coletam deixam de ser pontos de coleta (`64b08cdc`)

**Causa:** toda chamada Dart direta era ponto de coleta, então uma função recursiva que não
aloca (`Node.count` do kernel de árvores) abria o quadro da pilha-sombra e gravava as raízes
a cada ativação.

**Correção:** o emissor calcula, por ponto fixo máximo sobre o módulo (lista de trabalho nas
arestas inversas de chamada), as funções sem nenhum ponto de coleta, contando as chamadas
entre elas; em AOT (sem corpos de fora, logo sem recarga), memória tracing e raízes pela
pilha-sombra, a chamada a uma delas com argumentos sem conversão não é ponto de coleta. A
comparação de `Ref`s também deixou de contar (`coerce` só reinterpreta bits). A única
alocação que tal chamada ainda alcança é o `StackOverflowError` do prólogo de um chamado: o
runtime agora o constrói com a coleta adiada (`Heap::defer_collection`). O verificador de
raízes (`DARTFORGE_CHECK_SHADOW=1`) recalcula o mesmo conjunto a partir do texto do IR, sem
confiar no emissor. Ablação: `DARTFORGE_NO_NON_COLLECTING_CALLS=1`.

### 3.3 Clamp e multiplicação primitivos certificados ligados por padrão (`4d848ce0`)

Os dois passes só reescrevem quando as fontes do SDK escolhido batem com o certificado
revisado e mantêm o caminho por seletor como recuo guardado. O validador dedicado bate com a
VM e o AOT oficiais com os passes ligados e desligados, inclusive sob GC stress. O kernel de
cor do dart_ui foi de 31,1 ms para 8,6 ms (Dart AOT 5,2 ms). Desligam com
`DARTFORGE_PRIMITIVE_CLAMP=0` e `DARTFORGE_PRIMITIVE_MULTIPLY=0`. O clamp ainda recusa
programas com FFI e o modo A1 (`closed()` em `optimize/primitive_clamp.rs`), o que inclui o
File Explorer.

### 3.4 Globais enraizados pelos slots e cache de `rti_set` por sítio (`7a3c178a` e `a0ead32d`)

**Globais:** na área enxuta de produção com memória tracing, o store num global `Ref` virou
um store simples. O descritor da área lista os slots `Ref` (`[chave, -n, m, slots…]`) e o
coletor os lê a cada coleta, em vez de uma chamada ao runtime que atualizava um `HashMap` a
cada escrita. JIT e ARC mantêm o registro por store.

**`rti_set`:** o tipo reificado de um objeto genérico novo passa por um cache de duas palavras
do sítio na área (`classe << 32 | tipo` e o metadado). Um objeto da mesma classe com o mesmo
tipo pedido recebe o metadado com um store; listas e estáticos da imagem seguem pelo runtime
(`dartforge_rti_set_cache`, que também preenche o cache). O microbenchmark async foi de 456
para 352 ms (Dart AOT 247 ms).

A primeira versão expandia o acerto em linha em cada um dos 4.351 sítios do File Explorer e
custava cerca de 200 KB. O commit seguinte move o acerto para um helper `noinline` por
módulo, `@df.rti_set_cached`: o sítio vira um `getelementptr` e uma chamada, sem perda
medida no microbenchmark (348 ms). Ablação só deste cache: `DARTFORGE_NO_RTI_SET_CACHE=1`;
`DARTFORGE_NO_RTI_CACHE=1` desliga os três caches de RTI.

### 3.5 Getters de tipo compactos (`706531ad`)

**Causa:** cada receita de tipo do programa tem um getter preguiçoso `df.rti.<hash>.<lib>`
(3.490 no File Explorer). O corpo lia a flag da área, devolvia o tipo guardado ou analisava a
receita uma vez. Com o `df.get_area` em linha, cada um custava cerca de cem bytes.

**Correção** (`llvm/rti_getters.rs`): em AOT com strings estáticas, o getter com exatamente a
forma que `recipe_rti` constrói vira uma chamada de cauda a um helper por módulo,
`@df.rti_fill(slot do valor, slot da flag, receita)`, com as mesmas leituras, escritas e
chamada ao runtime. Qualquer outra forma sai como antes. Ablação:
`DARTFORGE_NO_COMPACT_RTI_GETTERS=1`.

| File Explorer, dart_ui `15aff70`, macOS arm64 | bytes |
|---|---:|
| depois da poda e das chamadas que não coletam (3.1 e 3.2) | 19.470.512 |
| mais 3.3 e 3.4 com o cache de `rti_set` em linha | 20.081.456 |
| cache de `rti_set` num helper compartilhado | 19.883.312 |
| getters de tipo compactos | 19.437.488 |
| `dart compile exe` 3.6.2 | 12.257.440 |

O ganho de velocidade das seções 3.3 e 3.4 cabe agora em menos bytes do que antes delas. A
razão contra o Dart ficou em 1,59×.

### 3.6 As duas falhas pré-existentes do corpus

**85, `(x: 1) is Record` dava falso** (`35f3270f`): `Record` do `dart:core` é `abstract
final` e a única classe dele na hierarquia é `_Record`, então o teste virava “classe ==
`_Record`”. Um record com campo nomeado tem a classe da sua forma, criada durante o lowering
e ausente da hierarquia. Agora uma classe diferente de `_Record` cai no mapa de subtipos,
onde cada forma tem a aresta para `Record`. `is`, `is!`, `Record?`, `as`, `whereType` e o
padrão `Record()` batem com a VM, em produção e em dev.

**134 em A1 passava do limite de 5 s** (`1fb44b6e`): a saída estava certa, mas o limite de
pilha do código Dart num isolado era o fundo da reserva de 64 MiB. A recursão sem fim chegava
a 2,1 milhões de quadros, e desenrolá-los por tabela levava 2,9 s; o perfil mostra o dyld
relendo os comandos de carga do Mach-O a cada quadro (`_dyld_find_unwind_sections`). O
código Dart agora usa no máximo 8 MiB da pilha de qualquer thread, o orçamento que a thread
principal já tinha. A VM dá 1 MiB a toda thread Dart (`OSThread::GetMaxStackSize`) e para em
cerca de 58 mil quadros; os quadros daqui são maiores, então o orçamento fica acima do dela.
O limite é só a comparação do prólogo com um valor do contexto, sem custo novo.

| programa 134, `--exceptions tables` | antes | agora |
|---|---:|---:|
| tempo total | 5,06 s | 2,55 s |
| quadros no isolado | 2.089.217 | 262.017 |
| desenrolamento no isolado | 2.905 ms | 375 ms |

O A1 continua cerca de 1,5 µs por quadro desenrolado, contra quase nada em A0: um
`StackOverflowError` profundo é o caso ruim desse modo.

### 3.7 Validação

- `production_profile`: 3/3, incluindo `platform_constant_prunes_the_other_backends`.
- Emissor: 1.020 testes de biblioteca (30 ignorados); runtime: 323 unitários; Clippy estrito
  e fmt nos dois crates.
- Corpus nativo contra a VM, depois de `1fb44b6e`: **138/138** em `--optimize` com o
  verificador de raízes ligado, em `--optimize --gc-stress`, em dev, em dev com
  `--gc-stress` (`--exec-limit 120`) e com `DARTFORGE_EXCEPTIONS=tables`.
- O smoke do File Explorer AOT (`scripts/validate-native-desktop.py --mode aot`) passou em
  `64b08cdc`, em `7a3c178a` e em `1fb44b6e` (abriu, renderizou dois quadros e o menu de
  contexto passou).

## 4. Medições depois das correções

Mínimo de 5 rodadas alternadas, mesma máquina com carga ≈ 3,5 (ruído de alguns %):

| núcleo (ms) | `eb6d25ab` | agora | Dart AOT |
|---|---:|---:|---:|
| árvores A0 | 19 | 17 | 11 |
| árvores A1 | 19 | 16 | — |
| fib | 5 | 5 | 9 |
| strings | 43 | 42 | 44 |
| JSON | 70 | 70 | 70 |

Depois das seções 3.3 a 3.5, medidos na mesma máquina:

| núcleo | antes | agora | Dart AOT |
|---|---:|---:|---:|
| kernel de cor do dart_ui | 31,1 ms | 8,6 ms | 5,2 ms |
| microbenchmark async | 456 ms | 348 ms | 247 ms |
| HTTP `/json`, CPU por 1000 pedidos | — | 34,0–34,5 ms | 33,0 ms |

Os demais núcleos de `bench/performance` ficaram dentro do ruído. O Dart continua à frente em
`utf8_bytes` 32,8 contra 22,3 ms, `encode_large` 49,8 contra 46,2 ms, kernel de cor ~30 ms
contra ~5 ms (medido antes). Não é medida de HTTP nem de aplicação longa.
Evidências: `bench/results/2026-10-20-program-class-pruning/`.

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
1. Comprimento de strings: em 19/10 entrou reutilização dominante no HIR e
   içamento de prefixos de leitura para predecessores que entram incondicionalmente
   no laço, com receptor já disponível fora dele. Não foi acrescentado
   `!invariant.load`; o helper LLVM permanece igual. Controle:
   `DARTFORGE_NO_STRING_LENGTH_REUSE=1`. Passaram 930 testes nativos e fixtures
   AOT/JIT macOS, inclusive GC sob estresse; sem ganho geral de tempo medido.
   Evidências: `bench/results/2026-10-19-string-length-reuse/README.md`.
2. `dartforge_exception_clear` em `break`/`continue` só dentro de regiões `try`/`catch`/`finally`.
3. ~~Raízes de campos estáticos pela varredura dos slots `Ref` da área~~: feito na seção 3.4.
4. Política jovem: limite por bytes até 16 MiB (hoje `YOUNG_COUNT` ≈ 8 MiB), envelhecimento
   adaptativo; medir RSS (árvore de 4 MiB: −37% com `YOUNG_KB=16384` + `AGING=0`).
5. ~~Getters RTI → helper compartilhado~~ (seção 3.5, −446 KB no File Explorer); registro do
   programa como dados; stubs `df.mt` como dados.
6. `rti_set` em linha com a receita da classe concreta (`_Map<K,V>`), não da interface.
7. Devirtualização por hierarquia sobre classes vivas (cone na `CallSelector`) e proveniência de
   retorno através do SDK nos resumos `.poda`.
8. Adaptadores `$c/$tc` só para seletores com sítio dinâmico; máquina `$ent` async.
9. Constantes canônicas como objetos da imagem `__dfimg`.
10. `DARTFORGE_LARGE_PROGRAM=1` para forçar o perfil grande e medir `optsize` no File Explorer;
    perfil por volume de IR.
11. Outliner compatível com A1 (hoje desligado com unwind: File Explorer A1 37,2 MB) e stubs
    que desenrolam para `noSuchMethod`/closure inválida, se A1 virar padrão.
12. A1: desenrolamento a ~1,5 µs por quadro no macOS (o dyld relê o Mach-O a cada quadro);
    um cache próprio das seções de unwind ou um caminho dedicado para `StackOverflowError`.
13. Clamp certificado para programas com FFI e em A1 (hoje recusado em `closed()`), o que
    alcançaria o File Explorer.
14. Estruturais: async no modelo da VM, `num` desencaixotado local, contexto em registrador,
    tabela global de despacho.
