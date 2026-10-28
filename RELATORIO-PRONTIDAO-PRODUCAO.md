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

## ARC: o plano de quatro passos, medido antes de executar

O plano de quatro passos registrado na seção anterior (contagem no cabeçalho, `retain`/
`release` e barreira em linha, mapa de referências em cache, medir) foi conferido com o
perfil das árvores em ARC simples (`sample`, símbolos mantidos, 3.014 amostras da thread
principal, tempo inclusivo) antes de mudar o layout. O resultado mudou a conclusão:

| onde vai o tempo (ARC simples, árvores) | inclusivo |
|---|---:|
| coleta disparada pela alocação (`allocate_slow` → `run_collection` → `drain_arc`) | 59% |
| ↳ morte dos objetos (`drain_zeros` → `discard_located`, `take_edges`, `release` dos filhos, `release_range`) | 36% |
| ↳ decisão do jovem e sincronização das raízes (`decide_young`, `sync_arcs`, `register_live_at`, `transfer_dead`) | ~16% |
| escrita de campo com contagem (`dartforge_object_set` → `set_field`, `arc_swapped`) | 20% |
| alocação fora da TLAB (`dartforge_object_new`, sem a coleta) | 11% |
| código Dart gerado (`tree`, `Node.count`) | ~6% |
| consultas à tabela lateral (`Table::get_mut`, `in_local_mut`, `get`), espalhadas acima | 14% |
| `ref_positions` (o mapa de referências) | 3% próprio |

- **Passo 1 (contagem no cabeçalho):** tira no máximo as consultas à tabela, 14%, e
  exige um cabeçalho de 24 bytes ou liberar `map`/`metadata` em compilador, runtime,
  coletor e objetos estáticos, com risco em todos os modos de memória. Com o resto
  intacto, as árvores iriam de ~250 para ~215 ms contra 14 ms no tracing.
- **Passo 2 (barreira em linha):** a escrita de campo pesa 20%, mas o que custa ali é o
  registro do objeto jovem na primeira ocorrência e a consulta a `snapshots`, que um
  caminho rápido em linha não evita sem o passo 1.
- **Passo 3 (mapa de referências em cache):** sem objeto. `ref_positions` já lê o mapa
  de bits do cabeçalho (`map`); seu tempo próprio é 3%. O plano estava errado neste
  ponto.
- **O que domina** é o desenho de contagem diferida: a pilha não é contada, então cada
  coleta jovem varre as raízes, decide os mortos e solta cada objeto morto
  individualmente, filho a filho. Isso é 59% e não muda com a contagem no cabeçalho.
  Chegar a um fator pequeno do tracing pede outro desenho (por exemplo, liberar em lote
  as faixas jovens mortas, como a varredura do tracing, e só contar o que sobrevive à
  primeira coleta), não a mudança de layout.

Por isso o passo 1 não foi executado: o ganho medido possível (≤14%) não paga uma
mudança de layout com risco em todos os modos, e os passos 2 e 3 dependem dele ou não
têm alvo. Nada de ARC mudou nesta rodada além do que entrou antes
(`1818759f`, `2d870798`, `45327774`). Árvores em ARC continuam em ~250 ms (simples) e
~351 ms (preparado), JSON ~325 e ~685 ms, contra 14 e ~75 ms no tracing.

## ARC com berçário: medido, ainda não padrão

(Superado pela seção seguinte: o berçário foi corrigido e é o padrão.)

O runtime já tem o desenho que o perfil acima aponta, como modo experimental do ARC
simples: `DARTFORGE_ARC_NURSERY=1` em tempo de execução (`HeapArc::pure` falso). Objetos
jovens não são contados; a coleta jovem libera os mortos em bloco, e só os promovidos
entram na contagem (as arestas dos jovens entram na promoção, `Heap::sync_arc`); a
barreira de escrita só conta com dono velho. Medido no `bench/comparison/bench.dart` em
ARC simples (carga 4 a 6, ms por rodada):

| núcleo | ARC puro (padrão atual) | ARC com berçário | tracing |
|---|---:|---:|---:|
| árvores | 255 | 64 | 14 |
| strings | 152 | 55 | 42 |
| JSON | 325 | 283 | 74 |

Seria de 18× para 4,5× o tracing nas árvores, 1,3× nas strings e 3,8× no JSON. Mas o
modo não está correto: comparado com o ARC puro nos 152 programas do corpus nativo, 9
divergem (`14`, `40`, `87`, `91`, `93`, `96`, `97`, `98`, `99`), sempre por
`ARC bug: RcZero` — um `release` de um objeto com contagem zero, vindo de
`Heap::sync_arc` na promoção ou da barreira de `dartforge_object_set`. O rastro
(`DARTFORGE_ARC_TRACE=1`) mostra um objeto registrado na promoção e depois liberado uma
vez a mais do que foi retido: alguma ocorrência escrita num dono enquanto o alvo ainda
era jovem não entra na contagem (nem pela barreira nem pelo conjunto lembrado), e a
sobrescrita posterior a libera. O ARC preparado com berçário falha já na primeira coleta
(o código preparado conta os donos jovens, e a promoção os conta de novo).

Tornar o berçário o padrão do ARC simples ficou num patch fora do repositório: o programa
não preparado chama `dartforge_memory_arc_nursery_v1` (o preparado continua com
`dartforge_memory_arc_v1`, puro), com `DARTFORGE_ARC_NURSERY=0` para medir. Não entrou
porque deixaria 9 programas do corpus quebrados em ARC. O próximo passo do ARC é achar a
ocorrência não contada (as escritas cruas dos natives sobre donos velhos, `arc_raw_write`,
e as que não passam pelo conjunto lembrado são as candidatas), depois ligar o berçário no
ARC simples e, por último, ensinar ao ARC preparado que um jovem não tem contagem até a
promoção.

## ARC com berçário: corrigido, padrão e acelerado (2026-10-27, continuação)

Mesmo Mac mini M4, carga de 5 a 29 (outro agente trabalhando em paralelo): os
ciclos e as instruções de `time -l` valem mais que o tempo de parede. Os núcleos
de `bench/comparison/bench.dart` rodam isolados, 10 rodadas por processo
(mediana em ms); o servidor é `bench/comparison/server.dart` com
`load_keepalive.dart` (4 × 8 conexões, 5 s), ciclos por pedido do processo
servidor. "Antes" é o ARC puro, o padrão até esta rodada, compilado hoje com
`DARTFORGE_ARC_NURSERY=0` (os caminhos do modo puro não mudaram).

| medida | tracing | ARC simples antes (puro) | ARC simples agora | ARC preparado antes (puro) | ARC preparado agora |
|---|---:|---:|---:|---:|---:|
| árvores, ms | 13 | 282 | 24 | 360 | 171 |
| árvores, ciclos | 0,58 G | 11,4 G | 1,05 G (1,8×) | 14,6 G | 6,9 G |
| strings, ms | 42 | 132 | 53 | 370 | 267 |
| strings, ciclos | 1,74 G | 5,3 G | 2,18 G (1,25×) | 14,9 G | 10,5 G |
| JSON, ms | 74 | 300 | 165 | 743 | 582 |
| JSON, ciclos | 3,0 G | 12,6 G | 6,7 G (2,2×) | 30,7 G | 23,5 G |
| servidor `/json`, pedidos/s | 55,1 mil | 14,1 mil | 40,5 mil | 5,6 mil | 8,2 mil |
| servidor, ciclos / instruções por pedido | 80 mil / 271 mil | 275 mil / 1,10 M | 99 mil / 375 mil | 676 mil / 3,61 M | 467 mil / 2,70 M |
| servidor, RSS depois da carga | 18 MB | 77 MB | 20 MB | 78 MB | 20 MB |

O ARC simples ficou entre 1,25× e 2,2× o tracing em ciclos (era 3× a 19×) e o
servidor foi a 73% do tracing (era 26%). O preparado melhorou 1,3× a 2,1×, mas
continua 6× a 12× o tracing.

**A causa do `RcZero`.** Não eram as escritas cruas (`arc_raw_write`) nem o
conjunto lembrado: o slot dono (global, token de código, quadro dono, tabela
canônica, fila de finalização) passava por `arc_swap_owning_root`, que
registrava o destino jovem (`arc_retain_registering`) também no berçário. O
jovem registrado ficava com uma contagem que não via as ocorrências dos donos
jovens; ao morrer por RC antes da promoção (o slot soltou, contagem zero), o
descarte soltava filhos que ele nunca tinha contado. No programa 40, o objeto
registrado na linha 405 do rastro morre na drenagem e solta o filho
`4344588450` de contagem zero. No berçário o slot agora só conta destino já
registrado, e a promoção conta as ocorrências dos donos que ainda seguram o
jovem. Teste: `nursery_owning_slot_leaves_young_object_uncounted`.

**O que entrou** (cada passo validado: os 152 programas `.dart` do corpus
nativo comparados com o ARC puro do mesmo programa em berçário, em berçário com
`DARTFORGE_GC_STRESS=1` e com `DARTFORGE_ARC_CHECK=1` — 152/152, em ARC simples
e preparado; o harness tem 160/160 em `--optimize` com
`DARTFORGE_CHECK_SHADOW=1`, em desenvolvimento e no Wasm, e 158/160 em
`--optimize --gc-stress` com o 91 e o 98 estourando os 5 s sob carga 29, ambos
ok rodados sozinhos, como em rodadas anteriores; testes de
`dartforge-emit-native` (1.040, mais os 13 ignorados de `tests/contract.rs`) e
de `dartforge-runtime`, `clippy -D warnings`):

- `7aa4e87e`: a correção acima. Também: a promoção relia cada objeto lembrado
  inteiro a cada coleta menor (uma lista de 200 mil posições por alocação sob
  `--gc-stress`); agora a varredura de cartões da menor registra as referências
  velho → jovem e a promoção conta essas. O ponto fixo da completa marca o mapa
  de bits em vez de um conjunto por hash, e as arestas que ele percorre espaçam
  as completas sob estresse como no tracing. O 97 sob berçário e estresse foi de
  209 s para 2,6 s; o 91, de mais de 300 s para 14 s.
- `73bd0d11`: o programa não preparado entra por
  `dartforge_memory_arc_nursery_v1`.
- `c90b2da7`: o preparado também. Os tokens do código preparado são raízes da
  coleta menor e donos contados na promoção; com a correção, o corpus inteiro
  passa sem mudar o código preparado. `DARTFORGE_ARC_NURSERY=0` na compilação
  mantém o ARC puro (outra chave do SDK).
- `2a2a2ddf`: no código não preparado (`Module::arc_nursery_writes`) a escrita
  num objeto novo sai em linha, e nos demais depois de testar o estado do dono:
  jovem em linha, sem barreira nem contagem; velho por `dartforge_object_set`.
  As listas do núcleo usam os ajudantes do tracing com a mesma divisão (e o
  crescimento em código gerado); a alocação com campos não chama mais
  `dartforge_arc_initial`. Por isso o ARC puro virou escolha de compilação. Na
  promoção, os promovidos são reconhecidos pela geração do registro (sem
  conjunto por hash) e as raízes que protegem zeros são um vetor ordenado.
  Árvores 61 → 36 ms, JSON 238 → 178 ms.
- `ba08c369`: o berçário envelhece como o tracing: o jovem alcançado sobrevive
  uma vez sem contagem e só a menor seguinte o promove (a completa não
  envelhece, porque a reclamação só guarda contados). As referências de um
  sobrevivente não são contadas até a promoção, então os velhos que ele
  referencia ficam protegidos como alvos de raiz; a referência de um lembrado
  só conta quando o alvo é promovido. Teste:
  `nursery_aging_protects_old_targets_of_survivors`. Árvores 36 → 27 ms.
- `dc20d708`: os mortos das tabelas laterais são reconhecidos por não terem
  metadados (sem conjunto por hash de todo morto); as trocas de slot dono e de
  campo pulam a tabela para jovens (o caso comum dos tokens do código
  preparado); a ação de finalização jovem não é registrada ao entrar na fila.
  Preparado: árvores 198 → 171 ms, JSON 611 → 592 ms.

**O que falta e por quê:**

1. **ARC preparado.** No perfil das árvores, 60% do tempo é a maquinaria dos
   tokens: `retain_code_owner`/`release_code_owner` (um `HashMap` de tokens por
   chamada), `arc_swap_owning_root`, as leituras e escritas de campo pela ABI
   auditada (`dartforge_arc_read_field_ref_v1`, `..._write_field_ref_v1`) e a
   alocação com dono, cada uma uma chamada ao runtime com TLS e empréstimo do
   heap. O berçário tira a contagem dos jovens, mas não os tokens: eles são as
   raízes do código preparado. Baixar isso pede mudar a preparação (não emitir
   token para valor jovem emprestado, operações de token em linha), trabalho de
   compilador com as provas de posse, não de runtime.
2. **JSON a 2,2×.** 37% do tempo é a sincronização, quase toda a morte por RC
   dos objetos promovidos (`drain_zeros` → `discard_located`, `take_edges`,
   `release` de cada filho): os 50 mil mapas vivem o núcleo inteiro, são
   promovidos e morrem um a um, onde o tracing varre em lote.
3. **Escritas que ainda vão ao runtime no ARC simples**: a célula de variável
   capturada (`emit_cell_set`) e a escrita num dono velho. A escrita escalar num
   campo escalar de dono velho poderia sair em linha testando o bit do mapa.
4. `abi::tests_gc_roots::lean_descriptor_gives_n_zeroed_slots_and_same_area`
   termina com SIGBUS no perfil de teste de depuração, também sem as mudanças
   desta rodada (não investigado).
5. O harness não tem modo ARC: a comparação usa um script que confronta cada
   programa com o mesmo programa em ARC puro (que confere com `dart run` em
   150/152, os dois restantes explicados na seção de testes de ARC acima).

## Rodada de 2026-10-27 (noite): vazão HTTP — caches de RTI, `String.[]` e strings

Medido no `main`, de `69c77488` (início: o servidor compilado com o compilador desse
commit no começo da rodada) a `326fce46` (fim), no mesmo Mac mini M4. A carga da máquina
oscilou entre 4 e 30 (outro agente compilando); a tabela usa as rodadas sem
interferência — processos alternados, 4 × 8 conexões com
`bench/comparison/load_keepalive.dart`, 4 s cada, contadores do processo inteiro pelo
`time -l` — e vale pelos ciclos e instruções por pedido, que mudam pouco com a carga.

| medida | início | fim | Dart AOT 3.6.2 |
|---|---:|---:|---:|
| servidor `/json` com keep-alive, pedidos/s | 55,6 mil | 60,8 mil | 72,4 mil |
| instruções / ciclos por pedido | 271,3 mil / 78,9 mil | 244,0 mil / 74,0 mil | 212,0 mil / 69,3 mil |
| executável do servidor | 4,419 MB | 4,435 MB | 5,625 MB |
| árvores (só o núcleo, 20 rodadas), ciclos / instruções | 1,15 G / 5,52 G¹ | 1,16 G / 5,52 G | 1,03 G / 5,02 G |
| árvores, ms por rodada (com carga) | — | 13,8 | 11,6 |

¹ Da segunda continuação (`348d1e94`); o binário das árvores não foi recompilado com o
compilador do início desta rodada.

O servidor passou de 77% para 84% dos pedidos/s do Dart: −10% de instruções e −6% de
ciclos por pedido. Falta +15% de instruções e +7% de ciclos. Nada desta rodada mexeu nas
árvores (as instruções são as mesmas, 5,52 G).

**Como foi medido onde vai o tempo.** Além do `sample` (thread principal: chamadas ao
sistema 35–45%, código Dart ~30%, runtime ~15–20%), contagens exatas por pedido com pontos de
parada do `lldb` que só contam (`breakpoint set -r '^df\.dart' -G true`), com 10 e 30
pedidos numa conexão e a diferença dividida por 20: **~1.260 chamadas de funções Dart
por pedido** e **~430 chamadas ao runtime**. O mesmo método, com um script Python no
ponto de parada, registrou cada falta dos caches de RTI por sítio com a chave: ~180 por
pedido, com 5 a 8 chaves distintas no mesmo sítio (as instâncias de `Future<T>` e
`Stream<T>` de cada pedido), cada uma a ~100 instruções no runtime.

**O que entrou** (cada um validado com os testes de `dartforge-emit-native` e
`dartforge-runtime` — 1.179 e todos os do runtime passam —, `cargo clippy -D warnings`,
o corpus nativo 160/160 em `--optimize` com `DARTFORGE_CHECK_SHADOW=1`, em
`--optimize --gc-stress`, em desenvolvimento e em Wasm — os poucos `timed out (5 s)`
das rodadas com a máquina carregada passam com `--exec-limit 60`, como
`91_textos_grandes_e_interpolacao` já fazia no início —, e de 6 a 14 programas do corpus
em ARC simples e preparado, com o berçário padrão, contra `dart run`):

- `7a87ec4b`: `LoadGlobal`, `StoreGlobal` (sem raiz registrada), `EnvGet`, `CellGet` e
  a caixa de um `bool` deixam de ser pontos de coleta. Todo getter de tipo preguiçoso
  (`df.rti.*`) começa lendo a flag de pronto: inlinado, fazia de cada método genérico um
  ponto de coleta que enraizava tudo o que estava vivo (`o is T` empurrava um quadro de
  raízes por chamada). Laço polimórfico de `is T`: 174 → 158 instruções por iteração.
- `1e1bb1f3`: um memo de RTI por thread, de 1.024 entradas em conjuntos de duas vias,
  atrás dos caches de uma entrada de cada sítio (`is T`, `P<i>` e o tipo de um objeto
  novo). A falta no sítio consulta o memo em código gerado (`@df.rti_is_probe`,
  `@df.rti_eval_probe`, o caminho lento de `@df.rti_set_cached`); só a falta nas duas
  vias entra no runtime, que preenche os dois. Laço com duas instâncias alternadas: 373 →
  228 instruções por iteração; servidor −3% de instruções.
- `0f2868ab`: `String.[]` Latin-1 em linha, com as strings de um caractere numa tabela
  da thread (como o intrínseco da VM devolve o símbolo predefinido), e `s[i]` sobre
  `String` como chamada direta ao `operator []` de `_StringBase`, em vez do seletor e da
  entrada tipada. Servidor −2,7% de instruções.
- `0d4bd90e`: `==` entre duas strings da mesma classe compara as unidades sem o runtime
  (o caso de `s[i] != " "`, cujo hash ainda não foi calculado), e `StringBuffer.write`
  de uma string lê os cabeçalhos do acumulador e da string direto, sem o empréstimo do
  heap nem a validação do handle.
- `68c186a0`: as avaliações de tipo com tupla (`@df.rti_eval_site`) também consultam e
  preenchem o memo. Com o anterior: −2,6% de instruções e −1,7% de ciclos.
- `855474f2`: uma única chamada por falta em cada sítio de RTI (chave 0 quando o valor
  não tem chave) e o laço de comparação de strings fora de linha: o texto do servidor,
  que tinha crescido 39 KB, fica 5 KB acima do início.
- `326fce46`: as tabelas da thread (memo e strings de um caractere) saem do `Context`,
  cujo modelo de thread-local ia inteiro para o executável (`__thread_data` +34 KB) e era
  copiado a cada thread nova: viram um thread-local próprio, zerado (`__thread_bss`),
  com o endereço no contexto.

**Medido e não feito.**

- *Contexto da thread em registrador* (item 4 da lista). Com ~1.260 chamadas Dart por
  pedido, as cinco instruções da leitura do contexto por TLS somam ~6 mil instruções
  por pedido (2,5%). Nas árvores pesa mais: duas chamadas por nó, e é a maior parte das
  0,5 G instruções de diferença para o Dart. Exige reservar um registrador (`x28`) e
  preparar o contexto em toda entrada que o runtime chama (closures, tabelas de
  métodos, callbacks), ou passar o contexto como argumento — uma mudança de ABI que não
  cabia nesta rodada.
- *Quadro de raízes em funções folha* (item 3): depois do `7a87ec4b`, nas funções quentes
  examinadas (`_StreamController.add`, `_CopyingBytesBuilder.add`) o quadro que sobra
  guarda valores vivos através de chamadas que coletam de fato.
- *Buffers de resposta pela TLAB* (item 2): o caminho de 8 KiB custa ~1,6% da thread
  principal além do `memset`, que o Dart também paga (zera o `Uint8List` na alocação).
- *Leituras extras com `EAGAIN`* (item 6): no `sample`, os `read` fora da thread principal
  são da thread de E/S (20 de 3.061 amostras); não investigado além disso.
- *Entrada direta por aridade* (item 7): o perfil não mudou o diagnóstico; não revisitada.

**O que ainda separa do Dart, em ordem:** a cópia de `codeUnits` byte a byte em
`_CopyingBytesBuilder.add` com o teste de classe refeito a cada iteração (4,7% das
amostras em código do executável); o `memset` dos dois `Uint8List` de 8 KiB e do refill
da TLAB (~2,7% da thread principal); os
literais de lista e `List.filled` pelo runtime (~50 chamadas por pedido: `list_literal`,
`adjust_list_shape`, `set_list_type`); getters de listas tipadas chamados sem inlining
(`length`, `elementSizeInBytes`, ~66 chamadas por pedido); o contexto da thread por TLS
e a conferência de exceção pendente depois de cada chamada.

## Rodada de 2026-10-28 (madrugada): contexto da thread por argumento e cópia dos cabeçalhos

Medido no `main`, de `830e790f` (início: o compilador desse commit, recompilado de uma
cópia exportada com `git archive`) a `f0fa1965` (fim), no mesmo Mac mini M4, com a máquina
carregada por outro agente (carga de 4 a 16; as medições abaixo esperaram a carga baixar
de 4,5). Processos alternados, mediana de 5 rodadas de 4 s com 4 × 8 conexões de
`bench/comparison/load_keepalive.dart` e contadores do processo inteiro pelo `time -l`;
árvores: mediana de 7 execuções. Valem as instruções e os ciclos por pedido.

| medida | início | fim | Dart AOT 3.6.2 |
|---|---:|---:|---:|
| servidor `/json` com keep-alive, pedidos/s | 60,6 mil | 62,1 mil | 70,5 mil |
| instruções / ciclos por pedido | 244,1 mil / 74,1 mil | 235,9 mil / 72,6 mil | 212,0 mil / 71,1 mil |
| CPU do processo por pedido (user + sys), rodada com carga | 22,2 µs (12,6 + 9,6) | 21,5 µs (12,2 + 9,3) | 21,8 µs (11,2 + 10,6) |
| executável do servidor | 4,435 MB | 4,452 MB | 5,625 MB |
| árvores (só o núcleo, 20 rodadas), instruções / ciclos | 5,52 G / 1,17 G | 5,21 G / 1,12 G | 5,02 G / 1,06 G |
| árvores, ms por rodada (média das 20) | 13,9 | 13,4 | 11,6 |

O servidor passou de 86% para 88% dos pedidos/s do Dart: −3,4% de instruções e −2,0% de
ciclos por pedido; faltam +11% de instruções e +2% de ciclos. As árvores foram de +10%
para +3,7% de instruções e de +10% para +6% de ciclos; o que sobra nelas é sobretudo
memória (a coleta jovem e o `memset` da TLAB, ~23% das amostras), não instruções.

**O que entrou** (validado no estado de cada commit com os testes de
`dartforge-emit-native` — 1.182 passam — e de `dartforge-runtime` — 440 —,
`cargo clippy -D warnings`, o corpus nativo 160/160 em `--optimize` com
`DARTFORGE_CHECK_SHADOW=1`, em `--optimize --gc-stress`, em desenvolvimento e em Wasm,
com `--exec-limit 60`, e de 8 a 11 programas do corpus em ARC simples e preparado, com o
berçário padrão, contra `dart run`: 22/22 e 16/16; os testes rodaram em release, sem o
teste de debug `lean_descriptor_gives_n_zeroed_slots_and_same_area`, que já dava SIGBUS):

- `6b6b1c1f`: `dartforge_globals_area`, `dartforge_globals_area_id` e
  `dartforge_rti_eval_cache` passam a folhas na tabela de efeitos. No início desta rodada
  `DARTFORGE_CHECK_SHADOW=1` reprovava **todos** os 160 programas: o verificador do modo
  sombra, que deduz do texto do IR quem coleta, via a leitura de um tipo preguiçoso
  (`df.rti.*` → `df.rti_fill` → `df.get_area`) e a avaliação de tipo em cache como
  pontos de coleta, que o emissor já tratava como não coletores desde `7a87ec4b`; o fecho
  de `_CastIterableBase.lastWhere` do SDK de hoje mantinha um valor vivo através deles.
  Nenhuma das três aloca no heap do coletor (a área é um `Vec` nativo; a avaliação em
  cache é `dartforge_rti_eval`, já folha, mais o cache do sítio e o memo da thread).
- `52a831e2`: o contexto da thread por argumento nas chamadas diretas (item 1 da lista da
  noite). Cada função `f` do AOT de produção (memória por rastreamento, pilha sombra,
  exceções por verificação, fora de Wasm) ganha `f$cx(ptr %ctx, …)`. Se o corpo usa o
  contexto, o corpo é `f$cx` e `f` vira um trampolim que lê o TLS e o chama; senão o corpo
  continua `f` e `f$cx` é um trampolim `alwaysinline` que descarta o argumento. As
  chamadas diretas de código que já tem o contexto — no módulo, ou a uma função
  `df.dart$3a…` de outro módulo do SDK de produção — vão a `f$cx`; todo endereço tomado
  (tabelas de métodos, closures, callbacks do runtime) continua sendo `f`, então nada muda
  no runtime nem nas entradas (main, isolados, FFI, `async`, coletor). Os auxiliares
  internos que liam o contexto de novo (`df.class`, `df.subclass`, `df.get_area`, as
  sondas do memo de RTI…) ganham gêmeos `$ctx` que o recebem (`llvm/context_helpers.rs`):
  sem isso a leitura de cada auxiliar inlinado deixava de se fundir com a do prólogo e o
  servidor piorava (de 2.786 para 2.978 leituras do TLS no executável). A poda das tabelas
  reconhece os gêmeos do seletor. Árvores: 5,53 → 5,21 G instruções; servidor: −2,1 mil
  instruções por pedido (as chamadas que restam são em boa parte indiretas e ainda passam
  pelo trampolim). `DARTFORGE_CONTEXT_PARAMETER=0` desliga (o cache do SDK distingue).
- `722e654b`: `_CopyingBytesBuilder.add` de `dart:_http` copia um `String.codeUnits` pela
  própria string (item 2), num patch de `dart:_http` (`native_sdk/http/http_patch.dart`, o
  primeiro da biblioteca). O laço genérico indexava a `CodeUnits` byte a byte, retestando a
  classe da lista e relendo os campos do construtor a cada byte (~60 instruções por byte;
  agora ~25). Os patches da VM só substituem membros `external`; o abaixamento nativo agora
  deixa um membro de arquivo de patch substituir um membro com corpo. Servidor: 240,7 →
  236,3 mil instruções e 79,0 → 76,8 mil ciclos por pedido (rodada com carga).
- `f0fa1965`: a alocação de uma lista tipada (ou visão) testa em linha, no bitmap que o
  runtime publica no contexto, se a tabela de métodos da classe já está registrada, e só
  senão chama a variante `_t`. Antes toda lista tipada passava por
  `dartforge_register_table` (dois acessos a thread-locals do runtime, ~1% das amostras da
  thread principal) para uma tabela que a produção registra na partida. Servidor: −1,2 mil
  instruções por pedido.

**Contagens por pedido** (pontos de parada do `lldb` que só contam, 10 e 30 pedidos numa
conexão, diferença ÷ 20, no binário com o contexto por argumento): 942 chamadas de
funções Dart, das quais 488 diretas já pelo `$cx` e ~433 indiretas (entradas `$tc`, `$g`
e `$c` pelo despacho por seletor ou por closures: `addByte$tc` 26, os getters `length` e
`elementSizeInBytes` de listas tipadas ~66), que ainda leem o contexto no trampolim; 365
chamadas ao runtime; e ~340 chamadas a auxiliares fora de linha do próprio módulo:
`df.rti_set_cached` 123 (toda alocação de objeto genérico), `df.rti_eval_probe` 64,
`df.rti_is_probe` 58, `df.rti_eval_site` 40 (faltas do cache de uma entrada de cada sítio
que acertam no memo da thread) e `df.string_units_equal` 43.

**Medido e não feito.**

- *Registrador reservado em vez de argumento.* Reservar `x28` (`+reserve-x28` e
  `llvm.read_register`) zeraria também as leituras das chamadas indiretas, mas toda
  passagem runtime → Dart teria de salvar `x28`, pô-lo e restaurá-lo — também quando uma
  exceção desenrola por quadros Rust —, e o x86_64 não tem a opção. O argumento não muda o
  ABI, vale em todo alvo nativo, e o trampolim mantém o caminho antigo para quem toma o
  endereço; as entradas indiretas ficam para depois.
- *Coleta jovem a cada 8 MiB* (`DARTFORGE_GC_YOUNG_KB=8192`): servidor −1,3% de
  instruções e −0,4% de ciclos, árvores −2,8% de instruções e −2,5% de ciclos, mas o RSS do
  servidor vai de 18,1 para 22,4 MB (o do Dart é 18,0) e o das árvores de 18,8 para
  27,3 MB. Ficou em 4 MiB.
- *Acerto de `df.rti_set_cached` em linha*: 123 chamadas por pedido, mas o acerto já é uma
  folha sem quadro; economizaria ~5 instruções por chamada (~0,25%) com ~12 instruções a
  mais em cada um dos ~700 sítios do servidor.

**O que ainda separa do Dart, em ordem:** as ~365 chamadas ao runtime por pedido, cada uma
com o empréstimo do heap por thread-local (`StringBuffer` 58; literais de lista e
`List.filled` ~45 — `list_literal`, `adjust_list_shape`, `type_list_shape_memo`,
`set_list_type`, `list_new`; `arg_index` 16; `string_equals_object` 15; as faltas de
seletor, 8, com `class_method` a ~1% das amostras); os ~340 auxiliares fora de linha de RTI
e de strings; as ~433 chamadas indiretas que ainda pagam o TLS no trampolim; e a
conferência de exceção pendente depois de cada chamada. Nos dois processos a thread do
isolado está saturada; o Dart gasta quase a mesma CPU de processo por pedido (21,8 contra
21,5 µs), com mais tempo de sistema e menos de usuário (11,2 contra 12,2 µs).

## Rodada de 2026-10-28 (manhã): onde a vazão HTTP se perde — threads, chamadas ao sistema e alocação

Medido no `main`, de `f0fa1965` (início: o servidor e as árvores compilados com o compilador
desse commit) a `d78d5196` (fim), no mesmo Mac mini M4, com outro agente compilando (as
tabelas usam rodadas com carga entre 3,9 e 4,5). Processos alternados: mediana de 5 rodadas de
4 s com 4 × 8 conexões de `bench/comparison/load_keepalive.dart` e contadores do processo pelo
`time -l`; a CPU por thread vem do `ps -M` tirado depois da carga e antes do `/quit` (3 rodadas
alternadas, todas iguais em ±2%); árvores: mediana de 7 processos e média dos 20 tempos por
rodada que o programa imprime, em 5 execuções alternadas.

| medida | início | fim | Dart AOT 3.6.2 |
|---|---:|---:|---:|
| servidor `/json` com keep-alive, pedidos/s | 61,4 mil | 62,8 mil | 71,0 mil |
| instruções / ciclos por pedido (processo) | 236,2 mil / 73,4 mil | 231,1 mil / 72,0 mil | 212,2 mil / 70,7 mil |
| thread do isolado por pedido: sistema + usuário | 4,6 + 11,7 µs | 4,6 + 11,2 µs | 5,0 + 8,7 µs |
| thread de E/S por pedido: sistema + usuário | 3,2 + 0,6 µs | 3,2 + 0,6 µs | 3,4 + 1,5 µs |
| RSS do servidor depois da carga | 18,4 MB | 18,1 MB | 18,1 MB |
| executável do servidor | 4,452 MB | 4,468 MB | 5,625 MB |
| árvores (processo inteiro), instruções / ciclos | 5,22 G / 1,125 G | 5,01 G / 1,060 G | 5,02 G / 1,011 G |
| árvores, ms por rodada | 13,3 | 12,15 | 11,8 |
| árvores, RSS máximo | 18,9 MB | 27,5 MB | 75,1 MB |

Servidor de 86% para 88% dos pedidos/s do Dart (as medições alternadas do `ps -M`: 61,2 →
63,1 mil contra 71,6 mil); árvores de +13% para +3% do tempo por rodada do Dart, com as
instruções já abaixo das dele.

**Por que os ciclos quase empatam e os pedidos/s não.** Os ciclos por pedido do `time -l`
somam todas as threads. Separadas, a thread do isolado está saturada nos dois processos, e é
ela que fixa os pedidos/s (1 / 16,3 µs = 61 mil; 1 / 13,8 µs = 72 mil). O Dart gasta mais na
thread de E/S (4,9 contra 3,8 µs por pedido) e menos na do isolado; a soma empata. A diferença
inteira é **tempo de usuário na thread do isolado**: 11,7 µs contra 8,7 µs no início (o
sistema empata, 4,6 contra 5,0 µs).

**Chamadas ao sistema: o mesmo padrão.** Sem `dtruss` (o SIP está ligado e não há `sudo`), um
interpositor por `DYLD_INSERT_LIBRARIES` contou as entradas da libc por thread (3 s de carga):
nos dois processos a thread do isolado faz por pedido 1 `read` do soquete, 2 `ioctl(FIONREAD)`
e 2,25 `write` (2 no soquete e ~0,25 byte de interrupção no pipe do tratador de eventos), e a
thread de E/S acorda ~0,25 vez por pedido e faz 1,1 (Dart) a 1,2 (nós) `kevent`. Não há escrita
fragmentada, acordar extra nem espera no laço de eventos: as 32 conexões mantêm o isolado sempre
com trabalho. As diferenças são de detalhe e todas fora da thread do isolado: a nossa thread de
E/S drena o pipe com uma leitura a mais (que dá `EAGAIN`) e lê o relógio duas vezes por
pedido; o Dart gasta ~1,5 µs por pedido na thread do isolado em `pthread_sigmask`,
`sigaltstack` e `sigprocmask` (10% das amostras dela), que nós não temos.

**Onde vai o tempo de usuário** (`sample` das duas threads, amostras próprias): o código Dart
compilado custa quase o mesmo nos dois (nosso `df.dart*` mais os auxiliares `df.*`, ~7,0 µs por
pedido; o código AOT do Dart, que inclui os stubs de alocação e de testes de tipo, ~5,9 µs); a
diferença está no runtime: ~3,8 µs por pedido nosso (runtime + `memset`/`memmove`) contra
~1,6 µs do C++ da VM e do `malloc`. No início, a maior fatia do runtime era memória — a coleta
jovem (3,5% das amostras da thread, quase toda em `sweep_young`), o `memset` da TLAB e dos
buffers de 8 KiB (2,5%) e os caminhos lentos de alocação —, e a seguir as ~365 chamadas ao
runtime por pedido, cada uma com o empréstimo do heap por thread-local e a validação dos
handles. Nas árvores, 26% das amostras eram coleta e zeragem: `sweep_young` 8,8%, `memset`
7,5%, `mark_pending` 6,1%.

**O que entrou** (validado no estado de cada commit, ou do conjunto quando o commit seguinte só
mexe no runtime, com os testes de `dartforge-emit-native` — 1.182 passam — e de
`dartforge-runtime` — 440 —, `cargo clippy -D warnings` dos dois, o corpus nativo 160/160 em
`--optimize` com `DARTFORGE_CHECK_SHADOW=1`, em `--optimize --gc-stress`, em desenvolvimento e em
Wasm, com `--exec-limit 60`, e 11 programas do corpus em ARC simples e preparado, com o
berçário padrão, contra `dart run`: 22/22 em cada estado):

- `5f4faf0b`: a coleta menor varre cada faixa da TLAB e cada sequência de sobreviventes por
  palavra do mapa de bits: uma carga por palavra limpa e um passo por bloco marcado, sem a
  divisão por trecho de 512 bytes nem o teste bloco a bloco de antes; os mortos saem da contagem
  dos marcados. Árvores: `sweep_young` de 8,8% para 2,1% das amostras, 1,131 → 1,063 G ciclos.
- `f0478ede`: o literal de lista homogêneo de 1 a 15 elementos sai em linha no AOT de produção
  com memória por rastreamento: o armazenamento (`_List`) e o `_GrowableList` vêm das TLABs dos
  seus tamanhos, com cabeçalhos, comprimento, elementos e armazenamento escritos sem barreira
  (os dois são jovens), como a alocação em linha da VM; TLAB esgotada, literal vazio ou misto
  continuam em `dartforge_list_new`. O runtime também passa a guardar no cache do sítio o tipo de
  uma lista quando gravá-lo não mudou o armazenamento (a forma já certa), o que um sítio de
  literal repete sempre; listas continuam fora do memo da thread, que outros sítios partilham.
  Laço de três literais pequenos: 1,21 → 0,62 G ciclos.
- `50a77bfc`: a lista tipada interna de tipo de elemento constante, com a tabela da classe já
  registrada (o teste em linha de `f0fa1965`), sai da TLAB do seu tamanho quando o corpo cabe em
  16 palavras (até 112 bytes), como o `TypedDataAllocationStub` da VM: cabeçalho, comprimento e
  endereço dos bytes, que já vêm zerados. Laço de quatro listas tipadas pequenas: 0,295 → 0,221 G
  ciclos.
- `5f1bedfc`: o limite jovem cresce como o semiespaço da VM: a coleta menor (rastreamento, sem
  estresse) que alcança mais de um bloco em dez dos alocados desde a anterior dobra o limite,
  até 8 MiB; não encolhe, e `DARTFORGE_GC_YOUNG_KB` ainda o fixa. O servidor (~0,5% de
  sobrevivência) nunca cresce e fica nos 18 MB; as árvores crescem na primeira coleta que pega
  a árvore longa em construção: 12,45 → 12,15 ms por rodada, RSS 18,6 → 27,6 MB (o Dart usa
  75 MB). É a política "esperta" pedida no lugar dos 8 MiB fixos, que custavam 4 MB a mais no
  servidor.
- `d78d5196`: `_List(length)` calcula o tipo antes (só lê e interna no universo) e aloca,
  tipa e ajusta a forma num único empréstimo do heap, sem quadro de raízes; o `_GrowableList`
  novo recebe comprimento e armazenamento por escrita direta, sem o `set_field` verificado.
  Com o anterior, −2 mil instruções por pedido.

As chamadas ao runtime por pedido (pontos de parada do `lldb` que só contam, 10 e 30 pedidos,
diferença ÷ 20) caíram de ~365 para ~308: `dartforge_list_new` de 7 para 4, as listas tipadas
pelo runtime de 10 para 4; `StringBuffer` continua com 58.

**Medido e não feito.**

- *Limite jovem de 16 MiB*: nas árvores, nada além dos 8 MiB (12,0 a 12,3 ms contra 12,0) com
  39 MB de RSS. Ficou em 8 MiB.
- *Refill maior da TLAB* (`DARTFORGE_TLAB_BLOCKS=512` e `1024`): servidor +0,4% de pedidos/s,
  dentro do ruído. Ficou em 256.
- *Literal vazio em linha com armazenamento próprio*: nenhuma diferença no servidor (62,9 contra
  62,2 mil pedidos/s, ruído) nem num laço de literais vazios; descartado.
- *Zerar na varredura em vez de no refill*: os mesmos bytes, com a localidade pior (a zeragem no
  refill põe as linhas no cache logo antes do uso). Não tentado. O que tiraria o `memset` (6,6%
  das árvores, 2,6% do servidor, metade disso nos dois `Uint8List` de 8 KiB por resposta, que o
  Dart também zera) é a alocação inicializar todos os campos, como os stubs da VM, e a TLAB não
  ser zerada — mudança em todos os caminhos de alocação do emissor e do runtime, que fica para
  uma rodada com um modo de verificação que envenene as faixas entregues.
- *Acerto de `df.rti_set_cached` em linha*: continua 1,6% das amostras da thread do isolado
  (123 chamadas por pedido, ~9 ciclos cada); o gêmeo `$ctx` do auxiliar é gerado do texto dele,
  e dividi-lo em acerto em linha e caminho lento não coube nesta rodada.

**O que ainda separa do Dart, em ordem** (thread do isolado, amostras próprias no fim): as ~308
chamadas ao runtime por pedido, cada uma com o empréstimo do heap por thread-local e handles
validados (`StringBuffer` 58, `arg_index` 16, `string_equals_object` 15, `typed_memMove1` 6,
`_List(length)` 4, visões tipadas 3), ~16% das amostras; o `memset` (2,6%), boa parte dele
inerente aos buffers de 8 KiB; os auxiliares de RTI fora de linha (`rti_set_cached`,
`rti_eval_probe`, `rti_eval_site`, `rti_is_probe`: 3,9%); as ~433 chamadas indiretas que ainda
leem o contexto por TLS no trampolim; e o modelo `async` do dart2js (`_propagateToListeners`,
`_RootZone.run*`, `_BufferingStreamSubscription`). Nas árvores, a marcação menor dos
sobreviventes (7,9%) e o `memset` (6,6%); o Dart faz a coleta jovem em paralelo em threads
auxiliares, por isso empata em ciclos do processo com 2,5% a menos de tempo de parede.

## Rodada de 2026-10-28 (fim da manhã): o runtime da thread do isolado — `StringBuffer`, cópias e textos

Medido no `main`, de `4a8143dc` (início: o servidor e as árvores compilados com o compilador
desse commit) a `e042497a` (fim; o compilador recompilado de uma cópia exportada com
`git archive` mais só os patches desta rodada, porque a árvore de trabalho tinha mudanças em
andamento de outro agente), no mesmo Mac mini M4. A máquina ficou carregada quase toda a
rodada (carga de 4 a 38, outros agentes compilando); as tabelas usam só rodadas começadas com
a carga abaixo de 3,5 e descartam as que outro processo perturbou. Servidor: mediana de 5 rodadas
alternadas de 4 s com 4 × 8 conexões de `bench/comparison/load_keepalive.dart` e contadores do
processo pelo `time -l`; CPU por thread pelo `ps -M` depois da carga (2 rodadas alternadas,
iguais em ±1%); árvores: mediana de 5 processos alternados.

| medida | início | fim | Dart AOT 3.6.2 |
|---|---:|---:|---:|
| servidor `/json` com keep-alive, pedidos/s | 63,5 mil | 65,9 mil | 70,1 mil |
| instruções / ciclos por pedido (processo) | 230,4 mil / 74,8 mil | 215,5 mil / 72,0 mil | 211,1 mil / 73,4 mil |
| thread do isolado por pedido: sistema + usuário | 4,95 + 11,07 µs | 4,91 + 10,22 µs | 5,48 + 8,55 µs |
| thread de E/S por pedido: sistema + usuário | 3,44 + 0,63 µs | 3,35 + 0,61 µs | 3,64 + 1,68 µs |
| RSS máximo do servidor | 18,7 MB | 18,4 MB | 18,1 MB |
| executável do servidor | 4,468 MB | 4,501 MB | 5,625 MB |
| chamadas ao runtime por pedido (`lldb`) | ~303 | ~183 | — |
| árvores (processo inteiro), instruções / ciclos | 5,01 G / 1,034 G | 5,01 G / 1,028 G | 5,03 G / 1,028 G |
| árvores, ms por rodada / RSS máximo | 12,1 / 27,5 MB | 12,1 / 27,5 MB | 11,9 / 75,2 MB |

O servidor passou de 91% para 94% dos pedidos/s do Dart: −6,5% de instruções e −3,7% de ciclos
por pedido. Nos ciclos do processo já fica abaixo do Dart (72,0 contra 73,4 mil) e nas instruções
a 2% dele; o que sobra é o tempo de usuário da thread do isolado, que fixa os pedidos/s: 10,2
contra 8,6 µs (era 11,1). Nada desta rodada mexeu nas árvores (as instruções são as mesmas).

**O que entrou** (validado na cópia limpa com os testes de `dartforge-emit-native` — 1.043
passam — e de `dartforge-runtime`, `cargo clippy -D warnings` dos dois, o corpus nativo 160/160
em `--optimize` com `DARTFORGE_CHECK_SHADOW=1`, em `--optimize --gc-stress`, em desenvolvimento e
em Wasm, com `--exec-limit 60`, e 11 programas do corpus em ARC simples e preparado, com o
berçário padrão, contra `dart run`: 22/22, em dois estados — depois de `6a717147` e no fim):

- `c5664354`: `a == b` com receptor estático `String` chamava o nativo
  `DartForge_string_equals_object` em toda comparação, inclusive `s != null` sobre `String?`
  (as ~15 chamadas por pedido que a rodada anterior contava). A comparação com o literal `null`
  passa a ser a identidade; um receptor não nulo usa o `@df.string_equals_object` em linha; um
  anulável nulo só é igual a `null`.
- `29c17042`: `StringBuffer` e o `jsonEncode` guardavam o texto num acumulador do runtime (um
  objeto do heap com um `Vec` nativo anexado): cada `write` era uma chamada que emprestava o heap e
  ajustava a contabilidade do anexo — 58 por pedido. O texto agora são as primeiras unidades de
  uma string nunca publicada, que dobra ao crescer e só passa a dois bytes quando chega uma unidade
  acima de 0xFF (`textBufferAppend`/`textBufferToString` no overlay de `dart:_internal`); a escrita
  que cabe é uma cópia em linha (`copyStringUnits`, `@df.string_write_units`: um `memcpy` ou o laço
  que alarga), e o `toString` uma alocação em linha e uma cópia. O `writeln` e o `writeCharCode`
  não passam por `write`, como na VM. Também `_CopyingBytesBuilder._addCodeUnits` (os cabeçalhos
  de `dart:_http`) copia uma string de um byte para o `Uint8List` com um `memcpy`
  (`@df.typed_write_from_string`). O manifesto de fontes revisadas do SDK
  (`double_identity/selected_sdk_manifest.json`) recebe os hashes novos: sem isso a prova de
  identidade de `double` se desliga para todo programa.
- `d9e02969`: `_TypedListBase._memMoveN` (o `setRange` entre listas tipadas do mesmo tamanho de
  elemento, 6 por pedido) é um `memmove` em linha (`@df.typed_write_move`); o `_setRange` da VM
  já conferiu os intervalos.
- `f31f5447`: a busca do argumento nomeado no descritor (`dartforge_arg_index`, 16 por pedido)
  é o laço em linha `@df.arg_index`, fora de ARC.
- `6a717147`: o acerto de `@df.rti_set_cached` (123 por pedido, o tipo de cada objeto genérico
  novo) fica em linha no sítio, onde a classe é constante; a falta chama o `@df.rti_set_miss`
  frio e compartilhado. −0,4% de instruções e −1% de ciclos por pedido, +49 KB no executável
  com símbolos (o executável sem símbolos cresce 33 KB na rodada inteira).
- `dcdc4138`: `indexOf`, `lastIndexOf` e a comparação de trecho de `startsWith`/`endsWith`
  (~11 por pedido) leem as strings pelos cabeçalhos, sem o empréstimo do heap.
- `e042497a`: o `substring` de um byte e o `String.fromCharCodes` de uma lista do core (o parser
  HTTP monta assim o método, a URI e cada nome e valor de cabeçalho) alocam a `_OneByteString`
  em linha e a preenchem com uma folha: o `substring` com `copyStringUnits` (que ganhou o
  deslocamento da origem), a fatia de até 120 unidades com `DartForge_string_fill_latin1`, que lê
  a lista pelos cabeçalhos e devolve falso, deixando o Dart no caminho da VM, para o que não é
  Latin-1. Antes eram `OneByteString_substringUnchecked` (empréstimo do heap) e
  `DartForge_string_from_char_codes` (enraizava a lista, alocava fora da TLAB e empilhava um
  quadro: ~10 amostras por chamada).

**Como foi achado.** O `sample` da rodada anterior media o runtime pelo custo próprio de cada
função; desta vez, pelo custo inclusivo de cada entrada chamada pelo código gerado
(`h3/rtin.py`), e o `lldb` com `bt` no ponto de parada mostrou quem chamava cada uma. Foi assim
que as 15 chamadas de `string_equals_object` apareceram como `s != null`: quando o `StringBuffer`
novo passou a comparar o seu buffer com `null`, elas subiram para 61.

**Medido e não feito.**

- *Remover o acumulador do runtime.* Os cinco nativos `DartForge_sb_*` ficaram sem uso no Dart,
  mas o acumulador (`TextAccumulator`, cid 15) ainda é um nó das mensagens entre isolados
  (`ports.rs`, `GNode::Accumulator`, também usado como reserva de nó); a limpeza toca `ports.rs` e
  os testes do heap, onde outro agente trabalhava. Fica para depois.
- *Não zerar os `Uint8List` de 8 KiB.* `dartforge_typed_new` é 4% da thread do isolado, quase
  todo o `memset` dos dois buffers de cada resposta em memória fria. Dar a eles memória sem zerar
  seria observável (`chunk.buffer` alcança os bytes além da vista), e o caso geral exige o modo de
  verificação que envenena as faixas entregues (prioridade 4); não tentado.
- *Cache de duas vias nos sítios de RTI.* As sondas do memo depois de uma falta no sítio
  (`rti_eval_site`, `rti_eval_probe`, `rti_is_probe`, `rti_set_miss`) somam ~2,2% da thread;
  duas vias cortariam parte delas ao custo de dobrar as palavras de cache de cada sítio.
- *A marcação dos sobreviventes nas árvores* (prioridade 5): o código já é o marcador sem
  ramificação por tipo de coleta; o que sobra é o modelo (varredura sem cópia, numa thread)
  contra o scavenger paralelo do Dart. Não mexido.

**O que ainda separa do Dart, em ordem** (thread do isolado, 10,2 contra 8,6 µs de usuário):
o código Dart compilado (~43% das amostras da thread, plano: `_HttpParser._doParse`,
`_Future._propagateToListeners`, `_SocketStreamConsumer.write`, os `$tc` dos seletores com
argumentos em vetor, a conferência de exceção pendente e o quadro de raízes depois de cada
chamada); o `memset` dos buffers de 8 KiB e do refill da TLAB (~4%); as ~120 chamadas ao
runtime exportadas que restam por pedido — `_List(length)` de `List.filled` (~1% com o cálculo
do tipo), literais de lista, `GrowableList_setData` do `clear()` das listas do parser, `asUint8List`
(`view_new`), as faltas de seletor (`class_method`); e as sondas do memo de RTI (~2%).

## Rodada de 2026-10-28 (tarde): ARC preparado — o ARC real, com morte imediata, e o modo rápido

Mesmo Mac mini M4, carga 3 nas medidas finais (outros agentes compilando antes). "Antes"
é `c08019c1` e "agora" é `86d0dd70`, cada um compilado de uma cópia limpa (`git archive`).
Núcleos de `bench/comparison/bench.dart` isolados, 10 rodadas por processo (mediana em
ms), contadores de `time -l` do processo inteiro; servidor `bench/comparison/server.dart`
com `load_keepalive.dart` (4 × 8 conexões, 5 s), ciclos e instruções do processo servidor
por pedido e o RSS máximo dele.

O ARC preparado (`DARTFORGE_ARC_PREPARED=1`) passou a ser o **ARC real**: tokens
exatos sobre o ARC puro (toda referência contada desde a primeira gravação, também
num jovem), e o objeto morre na liberação que zera a contagem — no próprio
`dartforge_arc_release` do código, no fim das escritas de campo e de lista da ABI, no
fechamento de escopo nativo, ou na leitura de uma `WeakReference`/`Expando` —, não na
coleta seguinte. Os ciclos continuam com a coleta de ciclos da completa. O antigo
preparado (tokens exatos sobre o berçário, morte na sincronização) fica em
`DARTFORGE_ARC_PROMPT=0`; o **modo rápido sem morte imediata** é
`DARTFORGE_ARC_TOKENS=virtual`. Detalhes em docs/ARC-IMPLEMENTACAO.md §8.

| medida | tracing | ARC simples | preparado antes | modo rápido | ARC real |
|---|---:|---:|---:|---:|---:|
| árvores, ms | 12,8 | 25,3 | 168 | 26,3 | 241 |
| árvores, ciclos / instruções | 0,54 / 2,55 G | 1,02 / 5,12 G | 6,82 / 48,2 G | 1,02 / 5,12 G | 9,75 / 70,2 G |
| strings, ms | 41,5 | 54,0 | 254 | 102 | 210 |
| strings, ciclos / instruções | 1,71 / 11,6 G | 2,24 / 15,1 G | 10,3 / 76,5 G | 4,14 / 28,9 G | 8,49 / 59,2 G |
| JSON, ms | 67,0 | 166 | 577 | 252 | 393 |
| JSON, ciclos / instruções | 2,73 / 18,6 G | 6,70 / 45,6 G | 23,2 / 158 G | 10,1 / 69,3 G | 16,5 / 109 G |
| servidor `/json`, pedidos/s | 65,1 mil | 46,6 mil | 9,5 mil | 29,8 mil | 10,1 mil |
| servidor, ciclos / instruções por pedido | 69,7 / 216 mil | 90,3 / 334 mil | 393 mil / 2,22 M | 133 / 610 mil | 383 mil / 1,67 M |
| servidor, RSS depois da carga | 17,4 MB | 19,0 MB | 19,9 MB | 19,7 MB | 70,6 MB |

(O "preparado antes", o tracing e o ARC simples de `c08019c1` deram nas árvores 168 ms,
12,9 e 23,5 ms; o servidor de `c08019c1` em tracing e ARC simples deu 63,1 e 43,9 mil
pedidos/s.)

**Quando um objeto é liberado agora.** No ARC real, na liberação que zera a contagem:
as fracas leem nulo, o finalizador vai à fila (o nativo roda na hora) e os filhos perdem
a ocorrência e morrem em cascata na mesma chamada, sem coleta no meio — os testes
`heap::prompt_death_tests` e um programa com `WeakReference` sobre um local, um campo,
uma posição de lista, um valor de mapa, uma closure capturada e uma cascata mostram a
morte imediata em todos, e em nenhum no ARC simples. A memória do morto volta na
drenagem seguinte. Um zero que um quadro observacional de nativo ainda vê espera o
quadro fechar. No ARC simples e no modo rápido, como antes: o jovem na coleta menor, o
velho na sincronização depois que a contagem chega a zero.

**O que entrou** (cada passo validado com os testes de `dartforge-emit-native` — 1.044
— e de `dartforge-runtime` — 332 —, `clippy -D warnings`, o corpus nativo pelo harness
em 160/160 nos quatro modos — `--optimize` com `DARTFORGE_CHECK_SHADOW=1`,
`--gc-stress`, desenvolvimento e Wasm — e os 152 programas `.dart` do corpus comparados
com `dart run` em ARC simples, real e rápido, cada um normal, com `DARTFORGE_GC_STRESS=1`
e com `DARTFORGE_ARC_CHECK=1`: todos iguais menos o 158, que liga asserts, e o 33, que
depende de tempo, iguais ao tracing):

- `03cacd38`: a tabela de instâncias canônicas é dona, mas a inserção não contava a
  ocorrência — o berçário recontava na promoção, o ARC puro nunca (a auditoria achava
  uma lista constante com RC 1 e duas ocorrências).
- `d70dfd4b`: o ARC real e o modo rápido. Tokens sem o inventário por `HashMap` (a
  auditoria o mantém), pontos de assentamento (`Heap::arc_settle`), a pilha-sombra fora
  das raízes no ARC real (os tokens possuem o que ela guarda; uma posição velha dela
  mantinha o handle de um morto), as marcas `late` e os índices de identidade do morto
  limpos antes de o bloco voltar, a escrita que move o token para o campo, chamadas
  uniformes diretas (o recurso `noSuchMethod` copia o próprio resultado), leituras e
  escritas escalares de campo em linha. No modo rápido: nenhum retain/release, escritas
  com a divisão do berçário, alocação com dono pela TLAB, `ArcCopy`/`ArcDrop` fora dos
  pontos de coleta.
- `e5d899fc`: o retain e o release do ARC real em linha: cada página publica na cabeça o
  vetor de metadados, e o código incrementa e decrementa a contagem da entrada viva,
  indo ao runtime só no zero, no novo candidato a ciclo ou numa página não publicada.
- `86d0dd70`: a escrita que move não faz mais o par retain/release do valor.

**O que falta e por quê:**

1. **O ARC real ainda custa 2,4–9,6× o ARC simples** em ciclos (árvores 9,75 contra
   1,02 G). O tempo está na morte (descarte no runtime: consulta à tabela, percurso das
   arestas, soltura dos filhos, remoção dos metadados), no registro de cada objeto novo
   e na escrita `Ref` contada, todos por chamada. Os próximos passos: registro e escrita
   contada em linha sobre o mesmo vetor; descarte especializado por classe e
   reutilização imediata da célula morta pela alocação seguinte do mesmo tamanho
   (*reuse* do Perceus), que tiram a drenagem do caminho e baixam o RSS (hoje 70 MB no
   servidor, a memória dos mortos só volta na drenagem); elisão de pares nas leituras de
   campos `final` com receptor emprestado. A contagem dentro do objeto exigiria um
   cabeçalho maior (o de 16 bytes não tem campo livre).
2. **O modo rápido a 1,5–1,9× o ARC simples** em strings e JSON e a 64% dele no servidor:
   o resto da ABI auditada (listas, caixas, escopos nativos em cada nativo que chama
   Dart, entrada de closure pelo runtime) e as otimizações que a preparação desliga.
3. **Padrão.** Nenhum dos dois modos preparados alcança o ARC simples nos quatro núcleos:
   o padrão do ARC continua o simples.
4. O harness não tem modo ARC: a comparação dos três regimes de ARC com `dart run` é
   feita por script, programa a programa.

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
2. **Vazão HTTP ~94% do Dart** (65,9 contra 70,1 mil pedidos/s; 215,5 contra 211,1 mil
   instruções e 72,0 contra 73,4 mil ciclos por pedido, no fim da manhã de 2026-10-28). As
   chamadas ao sistema seguem o mesmo padrão do Dart; a diferença é tempo de usuário na thread do
   isolado (10,2 contra 8,6 µs por pedido): o código Dart compilado, o `memset` dos buffers de
   8 KiB, as ~120 chamadas ao runtime que restam por pedido e as sondas do memo de RTI (seção do
   fim da manhã de 2026-10-28).
3. **`async` no nível do Dart** (no fim de 2026-10-27: 254 contra 249 ms, 5,22 contra
   5,30 G ciclos, com o dobro de instruções). O que sobra é a própria máquina de estados no
   estilo do dart2js (`_wrapBody` e o `guarded` por chamada, o corpo chamado pela
   entrada uniforme). A saída é o modelo da VM.
4. **Enums e constantes como objetos da imagem.** Hoje cada `T.a` lê uma flag de
   inicialização e pode chamar o getter; com objetos estáticos a comparação vira um
   endereço constante e o tamanho cai.
5. **Contexto da thread nas chamadas indiretas.** Desde `52a831e2` as chamadas diretas
   passam o contexto por argumento (árvores a 4% do Dart em instruções); as entradas
   chamadas pelo despacho por seletor e por closures ainda o leem por TLS no trampolim.
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
