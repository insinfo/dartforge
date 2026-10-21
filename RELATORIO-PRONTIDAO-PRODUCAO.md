# Prontidão do DartForge para produção (2026-10-20)

Medido no `main` em `8fe161bf`, macOS arm64 (Mac mini), Dart 3.6.2 como referência.
Os números de tamanho e de árvores que circularam antes deste relatório (explorador AOT
de 25,22 MB, árvores 1,75× mais lentas) são do commit `eb6d25ab` e já não valem.

## Resumo

| uso | situação | recomendação |
|---|---|---|
| Servidor HTTP e CLI nativos (AOT) | correto no corpus, heap estável sob carga, menos memória que o Dart, vazão 80–95% do Dart | pode ir para produção com implantação gradual e o Dart como plano B |
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

**Servidor HTTP sob carga** (`bench/comparison/server.dart`, rota `/json`, `ab -k -c 32`):

| | DartForge AOT | Dart AOT |
|---|---:|---:|
| pedidos por segundo | 26–29 mil | 27–35 mil |
| RSS no início | 15,2 MB | 23,3 MB |
| RSS depois de 1,2 milhão de pedidos | 17,6 MB | 52,7 MB |
| RSS depois de 3,2 milhões de pedidos | 19,0 MB | não medido |

O heap Dart do DartForge não vaza: depois de cada coleta completa ficam cerca de 50 KiB
ocupados e 2,3 a 3,2 MB de páginas, estáveis ao longo de 600 mil pedidos
(`DARTFORGE_GC_MEMORY=1`). O RSS ainda cresce cerca de 1 byte por pedido, só com conexões
simultâneas, fora do heap Dart. Sem concorrência, ou com uma conexão nova por pedido, o RSS
fica parado. É o próximo ponto a investigar antes de um servidor ficar semanas no ar.

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
| JS, testes do `new_sali/core` | 462–596 KB | 266–290 KB (`-O2`) | 1,6–2,1× |
| JS, mesmos testes, gzip | 144–174 KB | 84–91 KB | 1,6–2,0× |
| JS, hello | 47 KB | 34 KB (`-O2`), 5 KB (`-O4`) | 1,4× / 9× |

O DartForge compila o JS de produção cerca de duas vezes mais rápido que o dart2js.

## O que entrou nesta rodada

- `35f3270f`: `(x: 1) is Record` dava falso (programa 85).
- `1fb44b6e`: o código Dart usa no máximo 8 MiB da pilha de cada thread (programa 134 em A1).
- `706531ad`, `a0ead32d`: −644 KB no File Explorer (getters de tipo e cache de `rti_set`).
- `f6f0e0e2`, `e4e00d88`: árvores de 16,8 para 13,7 ms.
- `101af6c9`: tear-offs com corpo tipado; despacho por função de ~16 para ~10 ns.
- `8fe161bf`: cache por sítio das avaliações de tipo genéricas; `async` de 349 para 326 ms.

## Lacunas, em ordem de prioridade

1. **Tamanho do JS (~2× o dart2js).** O JS de produção segue o modelo de objetos do DDC
   (nomes privados por símbolo, `dartx`, assinaturas para chamadas dinâmicas). No hello o
   piso vem dos membros de `Object` tratados como universais no mundo fechado
   (`crates/world`, `universal_names`): `toString` puxa o RTI inteiro. Fechar a diferença
   exige gerar o modelo de objetos no estilo do dart2js, não ajustes pontuais.
2. **RSS fora do heap sob concorrência.** Cerca de 1 byte por pedido com 32 conexões
   simultâneas; medir por mais tempo e com o alocador do sistema instrumentado.
3. **`async` 1,3× o Dart.** O que sobra é a própria máquina de estados no estilo do
   dart2js (`_propagateToListeners`, closures por `await`). A saída é o modelo da VM.
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
```
