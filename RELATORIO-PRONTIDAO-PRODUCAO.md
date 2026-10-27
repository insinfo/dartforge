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
2. **Vazão HTTP ~70% do Dart** (49,7 contra 69,7 mil pedidos/s em 2026-10-27).
   Perfil achatado; os ganhos estruturais são o modelo `async`
   da VM, o contexto da thread em registrador e menos chamadas ao runtime em Rust.
3. **`async` 1,3× o Dart** (em 2026-10-27: 259 contra 253 ms, no nível do Dart em
   ciclos, com o dobro de instruções). O que sobra é a própria máquina de estados no
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
