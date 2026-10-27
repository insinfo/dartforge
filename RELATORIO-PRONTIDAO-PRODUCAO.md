# Prontidão do DartForge para produção (2026-10-20)

Medido no `main` em `8fe161bf`, macOS arm64 (Mac mini), Dart 3.6.2 como referência.
Os números de tamanho e de árvores que circularam antes deste relatório (explorador AOT
de 25,22 MB, árvores 1,75× mais lentas) são do commit `eb6d25ab` e já não valem.

## Resumo

| uso | situação | recomendação |
|---|---|---|
| Servidor HTTP e CLI nativos (AOT) | correto no corpus, memória estável em 10 milhões de pedidos e menor que a do Dart, vazão ~70% do Dart com keep-alive | pode ir para produção com implantação gradual e o Dart como plano B |
| Desktop nativo (dart_ui, File Explorer) | smoke AOT passa; executável 1,59× o do Dart | pode ir para produção onde o tamanho do instalador não é crítico |
| Web (JS de produção) | correto no corpus e num projeto real, saída idêntica à VM; arquivo ~2× o do dart2js | serve para sistemas internos; para sites públicos o tamanho ainda pesa |
| Modo A1 (`--exceptions tables`) | correto no corpus; desenrolar um `StackOverflowError` profundo é lento | usar o modo padrão (A0) em produção |

## O que foi verificado hoje

**Correção.**

- Corpus nativo contra a VM: **139/139** em `--optimize` com o verificador de raízes, em
  `--optimize --gc-stress`, em dev, em dev com `--gc-stress` e com exceções por tabela.
- Corpus JS contra a VM: **241/241** em desenvolvimento e em produção.
- Projeto real `new_sali/core` (cópia no scratchpad, dependências resolvidas offline): quatro
  testes web compilados pelo perfil de produção JS e executados no Node têm a saída
  **byte a byte igual** ao `dart run`.
- File Explorer AOT: abre, renderiza dois quadros e o smoke do menu de contexto passa.

**Servidor HTTP sob carga** (`bench/comparison/server.dart`, rota `/json`).

Com o `ab` (HTTP/1.0, sem reaproveitar conexão por causa do corpo `chunked`) os dois
ficam em 26–35 mil pedidos por segundo, mas o número mede mais o cliente que o servidor.
Com um gerador HTTP/1.1 com keep-alive (4 isolados × 8 conexões, compilado pelo Dart AOT):

| | DartForge AOT | Dart AOT |
|---|---:|---:|
| pedidos por segundo | 42 mil | 61 mil |
| CPU do servidor por pedido | 29,7 µs | 22,8 µs |
| RSS no início | 15,2 MB | 23,3 MB |
| RSS depois de 1,2 milhão de pedidos | 17,6 MB | 52,7 MB |
| RSS depois de 2, 5, 8 e 10 milhões de pedidos | 19,0 / 19,1 / 19,1 / 18,7 MB | não medido |

As chamadas de sistema custam o mesmo nos dois (`write` e `read` ≈ um terço do thread
principal). A diferença é o código: a pilha `_http` em Dart, a máquina `async` e o runtime
em Rust somam cerca de 20 µs por pedido contra 12 µs no Dart, num perfil achatado em que
nenhum item passa de 5% (RTI em sítios do SDK com muitos tipos, crescimento de listas,
coleta jovem, acesso a variáveis por thread no macOS). O modo A1 dá o mesmo resultado.

Não há vazamento. Depois de cada coleta completa ficam cerca de 50 KiB ocupados e 2,3 a
3,2 MB de páginas (`DARTFORGE_GC_MEMORY=1`), e o `malloc` não cresce entre os instantâneos
do `heap`. O RSS sobe nos primeiros 2 milhões de pedidos com 32 conexões simultâneas, por
buffers médios do runtime que crescem até o pico e por mais blocos de páginas do heap
(`vmmap`), e depois fica estável até 10 milhões. Sem concorrência ele nem sobe.

**Desempenho nativo** (melhor de várias rodadas):

| núcleo | DartForge | Dart AOT |
|---|---:|---:|
| fib | 4–5 ms | 9 ms |
| árvores (alocação) | 13,7 ms | 10,3–11,5 ms |
| strings / JSON | 42 / 70 ms | 44 / 70 ms |
| microbenchmark `async` | 326 ms | 248 ms |
| despacho por função (OIDs aleatórios) | 9,9 ns | 9,4 ns |
| `switch` denso 0..19 | 2,5 ns | 10,6 ns |
| comparação de enum (`==`) | 4,7 ns | 2,9 ns |

**Tamanho.**

| artefato | DartForge | Dart | razão |
|---|---:|---:|---:|
| File Explorer AOT | 19,50 MB | 12,26 MB | 1,59× |
| JS, testes do `new_sali/core` | 455–486 KB | 266–290 KB (`-O2`) | 1,6–1,8× |
| JS, hello | 47 KB | 34 KB (`-O2`), 5 KB (`-O4`) | 1,4× / 9× |

O DartForge compila o JS de produção cerca de duas vezes mais rápido que o dart2js.

## Rodada de 2026-10-21

| medida | antes | agora | Dart AOT |
|---|---:|---:|---:|
| listas pequenas por `add` (400 mil `int` e `String`) | ~300 ms | 50 ms | 37 ms |
| microbenchmark `async` | 326 ms | 310 ms | 242 ms |
| servidor `/json` com keep-alive | 42,0 mil/s | 45,2 mil/s | 60,8 mil/s |
| JS, testes do `new_sali/core` | 459–486 KB | 419–447 KB | 266–290 KB (`-O2`) |

- `b84e37f7`: listas crescem e recebem referências em código gerado; o runtime reaproveita
  os quadros de raízes e compartilha o armazenamento vazio por forma.
- `4c902208`: o JS de produção prova parâmetros sempre nulos (`testOn`) e elimina
  `boolean_selector`, `source_span` e `string_scanner`.
- `7a716b5d`: continuações de `await` reaproveitadas por ativação; `clear()` não tira mais a
  forma compacta de um `List<int>` (o parser HTTP perdia o caminho em linha a cada pedido).
- **Contexto em registrador, medido e adiado:** com o contexto lido de uma global (o teto de
  um registrador) o kernel de árvores não muda e o servidor ganha 2,8%. Não compensa o risco
  de recarregar o registrador em todo caminho Rust→Dart.

**Backend do `new_sali` (`bin/prod.dart`), macOS arm64:**

| | DartForge | Dart AOT |
|---|---:|---:|
| compilação com cache quente | 51 s | 18 s |
| primeira compilação (cache frio) | 237 s | 18 s |
| executável | 39,7 MB | 26,5 MB |

Os dois executáveis sobem com a mesma saída. Do tempo do DartForge, 20,6 s são lowering de
200 mil funções que depois a poda reduz a 59 mil, embora o mundo fechado já saiba em 0,1 s
que só 37,7 mil funções do programa vivem: baixar só o que o mundo marca é o próximo ganho de
tempo de compilação.

O que falta para passar o Dart no servidor é a convenção das chamadas virtuais: toda chamada
por seletor passa os argumentos num vetor e devolve o resultado em caixa (`bytes[i]` no
`_CopyingBytesBuilder` é uma dessas por byte). Uma entrada com argumentos diretos por aridade,
com fallback para a uniforme, é o próximo passo estrutural.

## Rodada de 2026-10-27: desempenho do AOT nativo

Medido no `main`, de `28a109bc` (início) a `3153448f` (fim), num Mac mini M4
com outras sessões compilando e testando ao mesmo tempo (carga de 4 a 20). Por isso,
além do tempo de parede, a comparação usa os contadores do `time -l`
(`instructions retired`, `cycles elapsed`): processos alternados, mediana de 5
execuções. O tempo de parede só vale nas rodadas sem carga, indicadas abaixo.

| medida | início | fim | Dart AOT 3.6.2 |
|---|---:|---:|---:|
| árvores (`bench/comparison/bench.dart`), ms por rodada | 15 [14] | 14 [13] | 12 [11] |
| árvores, 40 rodadas, ciclos / instruções | 2,44 G / 11,62 G | 2,34 G / 10,97 G | 2,15 G / 9,97 G |
| `async` (`bench/comparison/async_await.dart`), ms | 320 [316] | 259 [253] | 253 [249] |
| `async`, ciclos / instruções | 6,48 G / 39,4 G | 5,26 G / 30,5 G | 5,14 G / 14,2 G |
| JSON, ms / ciclos | 74,5 / 3,69 G | 77,5 / 3,70 G | 78 / 4,57 G |
| strings, ms / ciclos | 45 / 2,17 G | 43 / 2,10 G | 47 / 2,30 G |
| servidor `/json` com keep-alive, pedidos/s (rodadas sem carga) | 46,7 mil | 49,7 mil | 69,7 mil |
| CPU do servidor por pedido | 25,7 µs | 24,3 µs | 19,2 µs |
| instruções / ciclos por pedido (todo o processo) | ~306 mil / 91 mil | ~298 mil / 87,5 mil | ~215 mil / 72 mil |
| RSS do servidor depois da carga | 15,3 MB | 17,6 MB | 17,2 MB |

Entre colchetes, o mínimo. O gerador de carga agora está no repositório
(`bench/comparison/load_keepalive.dart`, 4 isolados × 8 conexões, compilado pelo
Dart AOT), assim como o microbenchmark `async`.

**O que entrou** (cada um validado com o corpus nativo, 160/160 em `--optimize` com
`DARTFORGE_CHECK_SHADOW=1` e em `--optimize --gc-stress`, e com os testes de
`dartforge-emit-native` e `dartforge-runtime`):

- `0215a534`: o cache de `is` no sítio de uso também responde para `null` e Smi
  (chaves `-16 - cid`), que antes iam sempre ao runtime (`_Uint8List.[]=` a cada
  byte, `value is Future` no `complete` de toda função `async` que devolve `int`).
  `async` −3,2% de instruções; servidor −0,9%.
- `db4ebe6d`: `await` de um `_Future` já concluído ou de um valor que não é
  futuro segue o `_SuspendState._await` da VM: um microtask próprio retoma o
  corpo, sem `_FutureListener`, futuro de resultado e `_propagateToListeners`.
  `async` −15% de instruções, −13% de ciclos.
- `e813b03d`: o quadro de cada chamada `async` (`_AsyncFrame`) é alocado em linha
  na TLAB, como as instâncias, em vez de `dartforge_object_new_m`. `async` −5,5%.
- `77a66079`: `DARTFORGE_FRAME_POINTERS=1` na compilação mantém os registros de
  quadro, e o `sample` passa a mostrar as pilhas Dart inteiras (só para perfil, com
  um `DARTFORGE_NATIVE_CACHE` próprio).
- `783c2ee5`: coleta jovem a cada 4 MiB em vez de 2 MiB. Árvores −4,3% de ciclos,
  `objects_in_collections` −4%, `fresh_list_append` −14%, JSON e strings −1 a −2%,
  servidor −3% de instruções e −5% de ciclos por pedido, ao custo de ~2 MB de RSS
  (igual ao Dart). `temporary_objects` +1%.
- `3153448f`: a fusão das faixas livres usa a ordenação estável, que aproveita as
  sequências já ordenadas (o quicksort era 1,2% das árvores). Árvores −0,8%.

**Testado e descartado:** cache de duas vias nos sítios de `is` e de avaliação de
tipo. As faltas que sobram não alternam entre duas chaves (são instanciações
genéricas novas), e o servidor não mudou (−0,4% de instruções).

**Onde vai o tempo do servidor agora** (perfil com registros de quadro, thread
principal): `write` 19,5%, `read` 7,3%, `ioctl` 2,3%; código Rust do runtime cerca de
25%, dos quais RTI ~6,5% (faltas dos caches de `is`, de avaliação e de `rti_set` em
sítios genéricos polimórficos) e alocação e coleta ~8% (as duas `Uint8List` de 8 KiB
por pedido vão por `allocate_block_outside_tlab`; na coleta jovem, a varredura pesa
mais que a marcação); as entradas uniformes `$c`/`$tc` somam 8,4% em tempo próprio.
O laço byte a byte de `_CopyingBytesBuilder.add` sobre `String.codeUnits` (cabeçalhos)
faz `bytes[i]` e `bytes.length` por seletor, com o argumento num vetor e o resultado
em caixa: ~3,3%. O `async` está no nível do Dart em ciclos, com o dobro de
instruções; nas árvores, a coleta (marcação e varredura) ainda é ~14% e o `memset`
das faixas entregues ~7%.

**Próximos passos, em ordem:**

1. Entrada direta por aridade para chamadas por seletor (`[]`, `length`, `add`),
   com o cache do sítio apontando para ela e fallback para a uniforme. É o maior item
   isolado do servidor (o laço dos cabeçalhos e boa parte dos 8,4% das entradas `$tc`).
2. Empurrar o quadro de raízes e as conferências de exceção pendente só nos
   caminhos lentos das entradas uniformes e das folhas (hoje todo `$tc` empurra o
   quadro na entrada, mesmo quando só o caminho lento aloca).
3. RTI de alocações genéricas sem ir ao runtime: guardar no objeto, ou no sítio, os
   argumentos de tipo já instanciados do chamador, como o vetor de argumentos de tipo
   compartilhado da VM.
4. Árvores: zerar no momento da alocação (o código gerado já escreve todos os
   campos) em vez de zerar a faixa inteira na entrega, e reduzir o custo da varredura
   jovem.
5. Menores: `String.[]` em linha para Latin-1 (hoje vai ao runtime), acesso a
   variáveis por thread no runtime Rust (`_tlv_get_addr`, 1,3%).

Dois testes de ARC de `dartforge-emit-native`
(`ordinary_nullable_coalesce_with_throwing_inline_fallback_certifies` e
`ordinary_late_generic_notifier_dispatch_prepares_without_erasing_field_guards`) já
falhavam em `28a109bc`, sem estas mudanças.

## Rodada de 2026-10-27 (continuação): testes de ARC, ARC e vazão HTTP

Medido no `main`, de `88e5457a` (início) a `f1b9b043` (fim), no mesmo Mac mini M4, com
carga de 3 a 5 nas medições finais. O início foi recompilado de uma cópia exportada
(`git archive 88e5457a`) e medido lado a lado com o fim e com o Dart AOT 3.6.2:
processos alternados, mediana de 3 rodadas de 5 s (servidor) ou de 5 execuções, com os
contadores do `time -l` de todo o processo.

| medida | início | fim | Dart AOT 3.6.2 |
|---|---:|---:|---:|
| servidor `/json` com keep-alive (`load_keepalive`, 4 × 8), pedidos/s | 51,0 mil | 53,8 mil | 71,7 mil |
| instruções / ciclos por pedido | 293 mil / 86,1 mil | 280 mil / 80,8 mil | 212 mil / 69,7 mil |
| CPU do servidor por pedido (user + sys) | 23,7 µs | 22,3 µs | 18,9 µs |
| RSS do servidor depois da carga | 17,3 MB | 17,0 MB | 17,3 MB |
| `async` (`async_await.dart`), ms por rodada | 253 | 254 | 249 |
| `async`, ciclos / instruções | 5,26 G / 30,5 G | 5,22 G / 30,5 G | 5,30 G / 14,1 G |
| árvores (só o núcleo, 20 rodadas), ms por rodada | 13 | 13 | 11 |
| árvores, ciclos / instruções | 1,153 G / 5,51 G | 1,139 G / 5,51 G | 1,049 G / 5,02 G |
| `bench.dart` inteiro, ciclos | 3,97 G | 3,95 G | 4,20 G |

O servidor passou de 71% para 75% do Dart. `async` continua abaixo do Dart em ciclos
(com o dobro de instruções) e as árvores continuam 8% acima em ciclos; nada desta
rodada mexeu nelas.

**Testes de ARC.** `ordinary_nullable_coalesce_with_throwing_inline_fallback_certifies` e
`ordinary_late_generic_notifier_dispatch_prepares_without_erasing_field_guards` falhavam
por uma regressão real, não por expectativa velha: `0e8cf6c7` (25/10) fez a entrada
uniforme de todo método chamar `dartforge_nsm_mismatch(recv, nome, código, args, desc)`
quando a aridade não bate, e os verificadores de vetores e de portas do ARC recusavam
esse uso do descritor. Com isso nenhum módulo com método preparava sob ARC. Os demais
testes dados como falhos na nota anterior (`trivial_phis`, `class_cases`) são estes
mesmos dois. Com o LLVM 22.1.8 no `PATH` (`ld64.lld`), os 1.038 testes não ignorados de
`dartforge-emit-native` passam, e também os de `dartforge-runtime`.

**O que entrou** (cada um validado com os testes de `dartforge-emit-native` e
`dartforge-runtime`, `cargo clippy -D warnings` e o corpus nativo: 160/160 em
`--optimize` com `DARTFORGE_CHECK_SHADOW=1`, em `--optimize --gc-stress` e em
desenvolvimento; o último também no corpus em Wasm, 160/160):

- `1818759f`: os verificadores de ARC aceitam exatamente a forma de
  `dartforge_nsm_mismatch` (lê o vetor e o descritor declarados e empresta os `Ref`, como
  um adaptador de encaminhamento); o resultado passa pelo thunk Owned genérico.
- `2d870798`: a preparação ARC publicada (`DARTFORGE_ARC_PREPARED=1`) não compilava
  nenhum programa: a preparação do SDK parava na primeira função do runtime sem contrato
  de posse acrescentada desde a última vez. Contratos auditados para
  `dartforge_rti_tuple_len`, `dartforge_rti_bound`, `dartforge_args_set_tuple` (com o novo
  tipo `mut:native`, endereço que o runtime escreve) e `dartforge_canonical_tearoff`. Dos
  152 programas `.dart` do corpus nativo, comparados com `dart run` por um script (o
  harness não tem modo ARC), 150 dão a mesma saída em ARC simples e em preparado (60
  deles conferidos também sob `DARTFORGE_GC_STRESS=1`); os outros dois diferem do mesmo
  jeito no tracing de desenvolvimento pelo mesmo script e passam no harness (`158` liga
  asserts; `33` depende de tempo).
- `45327774`: a fusão de caixas de inteiros anuláveis (`a?.m() ?? 0`) agora também roda
  em código ARC antes da preparação. Árvores em ARC preparado: 371 → 351 ms (−7% de
  ciclos); em ARC simples, −1,5%.
- `7a574e3e`: cada mensagem postada a um isolado (cada prontidão de soquete da thread de
  E/S) sinalizava a `Condvar` e fazia um `kevent(EVFILT_USER)` segurando o mutex da fila,
  que o laço de eventos toma várias vezes entre eventos: a thread principal do servidor
  ficava 2% bloqueada nesse mutex. Agora a fila sabe se a thread está esperando em
  `wait_message` e só então a acorda, depois de soltar o mutex (a bomba nativa continua
  sempre avisada). Servidor +4% de pedidos/s, −4,7% de ciclos por pedido.
- `0490da9f`: a entrada uniforme de um método genérico resolvia a tupla de argumentos de
  tipo com `dartforge_rti_tuple_join(0, 0, própria, limites)`, que copiava e reinternava a
  tupla já internada, e chamava `dartforge_rti_tuple_len` mesmo sem tupla. Agora usa a
  tupla passada como está e só monta os limites quando a chamada não passou nenhuma; a
  entrada tipada (`tc:`) nem confere a aridade, que o tipo estático garante
  (`Future.then` e o resto da maquinaria de streams e futures). Servidor +2,5%, −2,3% de
  ciclos por pedido.
- `f1b9b043`: `actual_selected_sdk_inventory_loads_without_authorizing_opaque_startup`
  (ignorado, pesado) falhava desde `c29dfd3c` — também em `88e5457a`: o registro de
  `dart:mirrors` passou a entregar ao runtime os endereços dos tear-offs dos seus
  métodos, obrigação que o inventário corretamente não certifica.

**Testado e descartado: entrada direta por aridade para chamadas por seletor.** Feita por
inteiro: entrada `$dc` (`i64 (this, a0…an-1, cache)`, só as conferências da entrada
tipada) para métodos com até quatro parâmetros posicionais obrigatórios e sem
parâmetros de tipo, registrada sob o seletor `dtc:m`; o sítio tipado posicional procura
esse seletor num cache de quatro palavras e, quando a classe não tem a entrada, recebe a
ponte da aridade (`@df.dbridge.<n>`), que monta o vetor e chama a entrada uniforme
guardada no cache; a poda e o runtime conhecem a cadeia `dtc:m` → `tc:m` → `c:m`. Medido:
no laço de `CodeUnits.[]` −0,9% de instruções e +3,9% de ciclos; no servidor 50,2 contra
50,9 mil pedidos/s (sem ganho) e +260 KB de executável (4,42 → 4,68 MB). O vetor não é o
custo: em `CodeUnits.[]$tc`, de ~45 instruções no caminho rápido, cerca de 20 são o
contexto da thread por TLS, a verificação de pilha, o quadro de raízes empurrado na
entrada e as conferências de exceção pendente, e o resto é desencaixotar o índice,
encaixotar o resultado e o corpo, que a entrada direta mantém. Não entrou no repositório.

**ARC continua longe do tracing**: árvores em 250 ms (ARC simples) e 351 ms (preparado)
contra 14 ms. O tempo está no runtime — contagens numa tabela à parte
(`Table::get_mut`, `discard_located`, `drain_zeros`), escrita de campo por chamada
(`dartforge_object_set`), `retain_code_owner`/`release_code_owner` —, não em pares
retain/release do código gerado. O próximo passo de ARC é a contagem no cabeçalho do
objeto com retain/release em linha, um redesenho do runtime.

**Onde vai o tempo do servidor agora** (thread principal, com registros de quadro):
`write` 18%, `read` 7%, `ioctl` (`FIONREAD`) 2,3%, `memset` 2,3% (as duas `Uint8List` de
8 KiB por pedido); RTI ~6,5%, quase todo em faltas dos caches por sítio no código
genérico de streams e futures (`_StreamController.add`, `_FutureListener.handleValue`,
`Future.then`, `Completer`), cujas instanciações se alternam no mesmo sítio;
`_CopyingBytesBuilder.add` com `CodeUnits.[]` ~3%. O mutex da fila sumiu do perfil.

**Próximos passos, em ordem:**

1. O prólogo das funções: contexto da thread em registrador e o quadro de raízes só nos
   caminhos lentos (com as raízes gravadas antes de cada ponto de coleta). É o que pesa
   em cada chamada pequena; só depois disso a entrada direta por aridade pode compensar.
2. O laço de `_CopyingBytesBuilder.add`: o teste de "lista do núcleo" de `bytes` é refeito
   a cada iteração (desmembrar o laço) e `CodeUnits` pode entrar no caminho rápido das
   listas do núcleo.
3. RTI nos sítios genéricos de streams e futures.
4. O laço de eventos toma o mutex da fila quatro vezes por evento (`serve_safepoints`,
   `next_control`, `next_arrival`, `dispatch_next`): uma basta.

Testes ignorados de `dartforge-emit-native`: os 13 de `tests/contract.rs` passam com
`DARTFORGE_TEST_SDK_LIB` apontando o `lib/` do SDK; os demais pedem os executáveis
gravados da matriz de desktop (`target/desktop-matrix`) e um sysroot Linux
(`DARTFORGE_TEST_LINUX_SYSROOT`), que esta máquina não tem.

## Rodada de 2026-10-27 (segunda continuação): prólogo e cópia de cabeçalhos

Mesma máquina e método da continuação anterior (início `88e5457a`, recompilado de uma
cópia exportada; fim `348d1e94`; processos alternados, medianas; servidor em 5 rodadas
de 5 s com `bench/comparison/load_keepalive.dart`, 4 × 8 conexões). A carga oscilou
entre 3 e 9; os ciclos e as instruções por pedido valem mais que o tempo de parede.

| medida | início | fim | Dart AOT 3.6.2 |
|---|---:|---:|---:|
| servidor `/json` com keep-alive, pedidos/s (rodadas sem carga) | 50,4 mil | 55,6 mil | 72,0 mil |
| instruções / ciclos por pedido | 293,5 mil / 86,8 mil | 270,5 mil / 79,1 mil | 211,9 mil / 69,9 mil |
| `async`, ciclos / instruções | 5,14 G / 30,5 G | 5,16 G / 30,5 G | 5,05 G / 14,1 G |
| árvores (só o núcleo, 20 rodadas), ciclos / instruções | 1,141 G / 5,52 G | 1,150 G / 5,52 G | 1,055 G / 5,02 G |

Em tempo de parede sem carga (continuação anterior), `async` 254 contra 249 ms e
árvores 13 contra 11 ms; nada desta rodada mexeu neles (diferenças de ±1% em ciclos
são ruído). O servidor está em 77% do Dart (era 71% no começo do dia).

**O que entrou** (validado como antes: testes de `dartforge-emit-native` com o LLVM
22.1.8 no `PATH`, corpus nativo 160/160 em `--optimize` com `DARTFORGE_CHECK_SHADOW=1`,
em `--gc-stress` e em desenvolvimento, corpus em Wasm 160/160, `clippy -D warnings`):

- `fd3e882f`: a verificação de pilha no prólogo só fica nas funções com alguma
  chamada que pode executar Dart (ou que gravam um global: inicializadores e caches de
  constantes, cuja saída de falha impede a publicação parcial). Uma função que só chama
  o runtime acrescenta um quadro, coberto pela folga abaixo do limite, e nenhum ciclo
  de chamadas volta a ela. Exceções por tabelas e Wasm mantêm a verificação em todas.
  Servidor +1,3% de pedidos/s; executável 16 KB menor.
- `348d1e94`: `bytes[i]` de uma `List<int>` lê em linha a unidade da string de um
  `CodeUnits` (o `String.codeUnits` que `_CopyingBytesBuilder.add` copia byte a byte
  para cada cabeçalho) antes de cair no seletor. Servidor −3,5% de instruções por
  pedido.

**Chamadas ao sistema por pedido**, contadas com um interpositor de `write`/`read`/`ioctl`
nos dois servidores: as mesmas (2,25 `write`, 2 `ioctl` `FIONREAD`, 1,24 `read` com
dados). O DartForge faz ainda 0,24 `read` por pedido que voltam `EAGAIN`, que o Dart não
faz (não investigado; provavelmente o dreno do pipe de interrupção na thread de E/S).
Ou seja, a diferença é toda código de usuário na thread principal.

**Onde vai o tempo agora** (thread principal): chamadas ao sistema 31%; código Dart
gerado 28% mais 9,5% nas entradas uniformes; runtime 15% (alocação de `Uint8List` de
8 KiB por `dartforge_typed_new_t` ~3%, com `memset` e `take_from_region`; RTI ~6%;
`String.[]` pelo runtime ~1%; `_tlv_get_addr` das variáveis por thread do runtime
1,5%); alocação e coleta ~7%.

**Não feito: o redesenho do ARC.** O cabeçalho do objeto tem 16 bytes inteiramente
ocupados (`state`, `flags`, `n`, `class_id`, `map`, `metadata`), lidos com deslocamentos
fixos pelo código gerado, pelo runtime e pelos objetos estáticos da imagem; a contagem
no cabeçalho exige crescer o cabeçalho para 24 bytes ou liberar `map`/`metadata`, uma
mudança de layout que atravessa compilador, runtime, coletor e imagens. Além disso, no
perfil das árvores em ARC simples 30% do tempo é a morte dos objetos dentro de
`run_collection` (`drain_zeros` → `discard_located` → `take_edges`/`ref_positions`
e `release` de cada filho), trabalho por objeto que a contagem no cabeçalho reduz mas
não elimina; o coletor de ciclos (`candidates.rs`) e as provas de posse do ARC preparado
dependem da tabela atual. É trabalho de vários dias com risco em todos os modos de
memória, e não cabia nesta rodada sem deixar o ARC quebrado. Plano: (1) cabeçalho de 24
bytes com a contagem e os bits de estado do ARC, `MetaArc` só para candidatos a ciclo;
(2) `retain`/`release` e a barreira de escrita de campo como helpers `@df.*` em linha,
com o caminho lento (zero, candidato a ciclo) no runtime; (3) o mapa de referências por
classe guardado em cache para `take_edges`; (4) medir árvores, JSON e HTTP em ARC contra
o tracing.

**Próximos passos do servidor, em ordem:** o quadro de raízes só nos caminhos lentos e
o contexto da thread em registrador (o maior item por chamada); a alocação de blocos
médios (8 KiB) pela TLAB em vez de `allocate_block_outside_tlab`; `String.[]` Latin-1
em linha (a tabela de strings de um caractere no contexto); o laço de
`_CopyingBytesBuilder.add` desmembrado; RTI nos sítios genéricos de streams e futures.

## O que entrou nesta rodada

- `35f3270f`: `(x: 1) is Record` dava falso (programa 85).
- `1fb44b6e`: o código Dart usa no máximo 8 MiB da pilha de cada thread (programa 134 em A1).
- `706531ad`, `a0ead32d`: −644 KB no File Explorer (getters de tipo e cache de `rti_set`).
- `f6f0e0e2`, `e4e00d88`: árvores de 16,8 para 13,7 ms.
- `101af6c9`: tear-offs com corpo tipado; despacho por função de ~16 para ~10 ns.
- `8fe161bf`: cache por sítio das avaliações de tipo genéricas; `async` de 349 para 326 ms.

## Lacunas, em ordem de prioridade

1. **Tamanho do JS (~1,7× o dart2js).** `3dfd321d` tirou até 19% ao manter membros de
   extensão vivos só para receptores do seu tipo `on` (o `&` de `int` mantinha o
   `petitparser` inteiro, e o `window.document` do runtime mantinha o `xml`). O próximo
   ganho é a propagação de valores de argumento do dart2js: ele prova que `testOn` é sempre
   nulo e elimina `boolean_selector`, `source_span`, `string_scanner` e `stream_channel`, que
   aqui somam cerca de 160 KB sem minificação. No hello o piso vem dos membros de `Object`
   tratados como universais (`universal_names`): `toString` puxa o RTI inteiro.
2. **Vazão HTTP ~77% do Dart** (55,6 contra 72,0 mil pedidos/s no fim de 2026-10-27).
   Perfil achatado; os ganhos estruturais são o prólogo das funções (contexto da thread
   em registrador, quadro de raízes só nos caminhos lentos), o modelo `async` da VM e
   menos chamadas ao runtime em Rust.
3. **`async` no nível do Dart** (no fim de 2026-10-27: 254 contra 249 ms, 5,22 contra
   5,30 G ciclos, com o dobro de instruções). O que sobra é a própria máquina de estados no
   estilo do dart2js (`_wrapBody` e o `guarded` por chamada, o corpo chamado pela
   entrada uniforme). A saída é o modelo da VM.
4. **Enums e constantes como objetos da imagem.** Hoje cada `T.a` lê uma flag de
   inicialização e pode chamar o getter; com objetos estáticos a comparação vira um
   endereço constante e o tamanho cai.
5. **Contexto da thread em registrador.** Cada função lê o contexto por TLS (cinco
   instruções na entrada); é a maior parte do que falta nas árvores.
6. **Desenrolamento em A1** a ~1,5 µs por quadro no macOS (o dyld relê o Mach-O a cada
   quadro). Só pesa em exceções que atravessam muitos quadros.

## Como reproduzir

```sh
# corpus nativo e JS
DARTFORGE_CHECK_SHADOW=1 target/p4/release/dartforge-differential --native --optimize --corpus corpus/native --jobs 4 --quiet
target/p4/release/dartforge-differential --production --corpus corpus/js --jobs 6 --quiet

# servidor sob carga
dartforge aot bench/comparison/server.dart srv --optimize
./srv 18081 & ab -k -c 32 -n 200000 http://127.0.0.1:18081/json; ps -o rss= -p $!

# keep-alive HTTP/1.1 (4 isolados × 8 conexões, 5 s) e contadores do processo
dart compile exe bench/comparison/load_keepalive.dart -o load
/usr/bin/time -l ./srv 18081 & ./load 18081 4 8 5; curl -s http://127.0.0.1:18081/quit
dartforge aot bench/comparison/async_await.dart async_await --optimize && /usr/bin/time -l ./async_await
```
