# Tamanho do executável e desempenho contra o Dart AOT

Plano consolidado de duas pesquisas de 2026-09-29 sobre `E:\references` (dart-sdk, julia,
swift, dartino-llvm, dart-sdk-llvm-mraleph, linzj-llvm-project, mun). Números medidos no
Windows 11 (i3-1215U, máquina ruidosa, ±15–30%); o que é estimativa está marcado.

## 1. Tamanho

### Medida (bytes)

| Programa | DartForge `aot --optimize` | Dart 3.6.2 `compile exe` | Snapshot AOT do Dart |
|---|---:|---:|---:|
| hello (`print`) | 12 211 200 | 5 797 376 | 798 816 |
| `bench/http/servidor.dart` | 13 676 544 | 6 108 160 | 1 093 464 |

O `compile exe` do Dart é o `dartaotruntime.exe` (5 015 040) mais o snapshot.

No hello, o `.text` tem 9,5 MB:

- **Código Dart: 8,2 MB (86%)**, quase todo do SDK. O programa do usuário ocupa menos de 80 KB.
- Rust: 1,17 MB. C (ring, zlib): 0,14 MB.

Por forma de entrada, o código Dart se divide em:

| Forma | MB |
|---|---:|
| corpos | 2,80 |
| `$c` | 2,02 |
| `$tearm` | 1,45 |
| `$g` | 1,04 |
| `$tc` | 0,45 |

No `.rdata`:

- **`@df.area`: 0,97 MB.** São os hashes de slot, que só servem à recarga.
- **Tabelas `df.mt.*`:** 0,47 MB.
- **`.xdata`:** 0,44 MB.

**Causa raiz:** o `/OPT:REF` não poda o SDK.

- A entrada chama os `df.registrar.<lib>` das 15 bibliotecas do SDK.
- Toda alocação referencia a `df.mt.<Classe>$d`, que lista todo membro da classe em todas as
  formas de entrada. Então uma classe instanciada mantém vivos todos os seus métodos.
- Um analisador de alcance sobre o IR confirmou isso: o modelo atual alcança 8 207 168 dos
  8 207 376 bytes. Mantendo viva só a entrada cujo seletor aparece em algum `df.seletor` vivo, o
  código Dart cai **80% no hello (1,6 MB) e 72% no servidor**. É estimativa: o executável podado
  não foi gerado.

### Ganhos rápidos (medidos por religação)

| Mudança | Onde | hello | servidor |
|---|---|---:|---:|
| Runtime com `panic=abort` + `opt-level="s"` | `emit_native/build.rs` (`precompilar_runtime`) | −648 KB | −679 KB |
| `@df.area` reduzido a `[chave, n]` fora da recarga | `llvm/mod.rs` (`emitir_descritor_da_area`), `gc_raizes.rs` | −968 KB | −968 KB |
| `/opt:safeicf` (o `==` de tear-off é preservado) | `ligador_windows.rs`; `--icf=safe` no Linux | −224 KB | −258 KB |
| Atributo `optsize` no SDK de produção | `sdk_modulo.rs` | −307 KB | −376 KB |
| **Tudo junto** | | **10,0 MB (−18%)** | **11,3 MB (−17%)** |

Cuidados:

- **`panic=abort`:** um pânico numa thread de isolado passa a abortar o processo. Conferir
  `isolados.rs`.
- **`opt-level="s"` e `optsize`:** passam pelos benchmarks antes de entrar.
- **Descartado:** o `/opt:icf` completo quebra a identidade de tear-off.

### Feito (2026-09-30)

A tabela acima é a estimativa por religação. O que entrou, medido de ponta a ponta:

- **Runtime de produção com `panic=abort`.** É uma terceira variante da `staticlib`,
  `dartforge_rtprod_*`: a feature `dll` com `panic=abort` (`ABORTAR` em `emit_native/build.rs`,
  `RuntimeCache::para_producao`). Só o executável de produção a liga. A variante `aot` (a do
  `main` C, sem o SDK da fonte) também aborta. A da DLL de desenvolvimento, que o JIT carrega,
  continua desenrolando. Quase nada muda no comportamento: um pânico no código que o Dart
  chama já abortava na fronteira `extern "C"`. O que muda está em `NATIVO.md` §1.1.
- **`@df.area` enxuto:** `[chave, -n]` no `aot --optimize`, no programa e no SDK de produção
  (`LlvmEmitter::com_area_enxuta`). O sinal marca a forma. O leitor é
  `dartforge_area_de_globais`: dá `n` slots exatos, sem nomes e sem folga. O JIT e a DLL de
  desenvolvimento continuam com o descritor completo.
- **ICF seguro na produção:**
  - `/opt:safeicf` no `lld-link`; o desenvolvimento continua com `/opt:noicf`.
  - `--icf=safe` no `ld.lld`.
  - No `ld64.lld` não entrou: não foi verificado num Mac que um objeto sem `__llvm_addrsig`
    (os do Rust) conte como endereço tomado.
  - `corpus/nativo/80_tearoffs_de_corpo_identico.dart` confere contra a VM. Com o
    `/opt:icf` completo forçado, ele falha (`nada == nadab` dá `true`); com o seguro, passa.
- **`optsize` em toda função do SDK de produção** (`sdk_modulo::com_optsize`).
- **Rejeitado: `opt-level="s"` no runtime.** Tira mais 233 KB (hello) e 248 KB (servidor),
  mas o `json.dart` fica ~55% mais lento em duas rodadas (soma dos mínimos: 2,4 → 3,8 s e
  2,3 → 3,5 s). O runtime faz o trabalho de texto e de números do JSON.

Tamanho, `aot --optimize`, numa cópia do `HEAD` 0ef4a7ac (bytes):

| Programa | Antes | `panic=abort` + área enxuta + `safeicf` | + `optsize` no SDK | Total |
|---|---:|---:|---:|---:|
| hello | 12 349 440 | 10 706 944 | 10 253 824 | −2,00 MB (−17,0%) |
| t0 (`map`/`join`) | 12 351 488 | 10 708 992 | 10 255 360 | −17,0% |
| `bench/http/servidor.dart` | 13 827 584 | 12 124 672 | 11 592 192 | −2,13 MB (−16,2%) |
| `bench/desempenho/json.dart` | 12 429 312 | 10 773 504 | 10 319 872 | −17,0% |
| `bench/simd/bin/blend.dart` | 12 368 384 | 10 724 864 | 10 272 256 | −16,9% |

Tempo de ligação do servidor (cache de objeto quente, duas rodadas): antes 50,6 e 38,2 s;
com as três mudanças 36,9 e 36,0 s; com `optsize`, 38,0 e 36,6 s. Não piorou.

Desempenho (máquina com outros builds rodando; mínimos de rodadas alternadas):

| Medida | Antes | 3 mudanças | + `optsize` |
|---|---:|---:|---:|
| `json.dart`, soma dos mínimos (3 rodadas) | 2,14 s | 2,01 s | — |
| `json.dart` (6 rodadas) | — | 3,14 s | 3,16 s |
| `blend.dart` escalar / 3.6 / 3.14, µs (20 rodadas) | — | 76,1 / 99,2 / 30,4 | 77,0 / 97,0 / 30,4 |
| servidor, req/s em `/` e `/json`, mediana de 5 | 2 506 / 2 271 | 2 433 / 2 456 | 2 843 / 2 331 |

O servidor foi medido no Windows com um cliente Dart de 4 conexões keep-alive. O
`scripts/bench-http.py` exige `wrk` e `/proc`, então não rodou.

### Estrutural: tabelas de métodos montadas na ligação

É o equivalente, no DartForge, destas peças das referências:

- `table_selector_assigner.dart` e `precompiler.cc` (`DropFunctions`) do Dart;
- a eliminação de funções virtuais do Swift (`GenMeta.cpp`, `DeadFunctionElimination.cpp`);
- o `--trim` da Julia.

Como funciona:

- O cache de bitcode do SDK continua. Os módulos do SDK passam a só declarar as `df.mt.*$d` e os
  registros dos CIDs do runtime.
- Cada biblioteca grava um resumo com funções, referências e seletores.
- Na ligação, um ponto fixo por seletor (RTA) gera as tabelas no módulo do programa só com as
  entradas vivas.
- A LTO e o `/OPT:REF` removem o resto.

Esforço e retorno:

- **Onde mexer:** `llvm/seletores.rs`, `lower/sdk_fonte.rs`, `df.registrar.programa` em
  `llvm/mod.rs` e os resumos em `sdk_modulo.rs`.
- **Risco:** chamadas por nome em tempo de execução (`Function.apply`, `noSuchMethod`,
  `hash_do_nome` em `seletores.rs`/`closures.rs`, mirrors). Pedem raízes conservadoras, como as
  do `crates/mundo`.
- **Esforço:** 1–2 semanas.
- **Projeção (estimativa):** hello de 3,3–3,8 MB e servidor de 4,5–5 MB, abaixo do Dart AOT. A
  religação de produção também fica bem mais rápida; hoje leva de 30 a 86 s.

Depois disso:

- **Formas de entrada sob demanda:** `$tearm` só se o getter do método for usado, e `$c`/`$tc`
  só para seletores chamados de forma dinâmica.
- **Deduplicação de stubs idênticos:** o equivalente do `DedupInstructions` do Dart.

### Feito: tabelas montadas na ligação (2026-09-30)

A especificação é `NATIVO-PODA-DE-TABELAS.md`. Resumo do que entrou:

- **SDK sem tabelas.** O SDK de produção não define mais as tabelas de métodos. Cada biblioteca
  grava um resumo (`<lib>.poda`) junto do bitcode em cache.
- **Ponto fixo na ligação.** O `poda::montar` roda um ponto fixo por seletor sobre os resumos e o
  IR do programa, e define no módulo do programa só os pares vivos.
- **Formas sob demanda.** As formas de entrada saíram de graça: cada uma é um par próprio, com o
  seu seletor.
- **Onde vale.** Só no `aot --optimize`. O JIT, a recarga e a DLL de desenvolvimento continuam
  com as tabelas inteiras.

Medidas numa cópia do `HEAD` `0ae614d8` ("antes") e na mesma cópia com a mudança ("depois"),
em bytes:

| Programa | Antes | Depois | Dart 3.6.2 `compile exe` |
|---|---:|---:|---:|
| hello | 10 711 552 | 3 181 568 (−70%) | 5 797 376 |
| t0 (`map`/`join`) | 10 713 088 | 3 183 616 (−70%) | — |
| `bench/http/servidor.dart` | 12 075 520 | 4 759 552 (−61%) | 6 108 160 |
| `bench/desempenho/json.dart` | 10 774 016 | 3 522 560 (−67%) | — |
| `bench/simd/bin/blend.dart` | 10 731 520 | 3 227 648 (−70%) | — |

- **Pares vivos no hello:** 3 111 de 50 066, em 158 de 662 tabelas. O `.text` cai de 9,5 MB
  para 2,6 MB: 1,79 MB de Dart (o `dart:core` fica com 0,64 MB), 0,68 MB de Rust e 0,14 MB de
  C.
- **Tempo da montagem:** 50–160 ms por programa.
- **Tempo de ligação (cache de objeto quente):**
  - antes, 35–66 s com a máquina livre e 78–97 s com a máquina ocupada;
  - depois, 11–22 s nas mesmas condições.

  É o LTO que encolhe.
- **Chave de comparação:** com `DARTFORGE_SEM_PODA_DE_TABELAS=1`, o `corpus/nativo/81` volta a
  10,8 MB, e todas as formas dos membros não usados reaparecem no mapa.

Ferramentas, em `tools/tamanho/`:

- `quebra.py` quebra o mapa da ligação (`DARTFORGE_MAPA_DA_LIGACAO=1`) por biblioteca.
- `por-que.py` segue o relatório `DARTFORGE_POR_QUE=<arquivo>`: a cadeia de causas de um
  símbolo ("quem puxou isto?") e os bytes por seletor que puxou.

O `alcance.py` da pesquisa virou o próprio `poda.rs`.

O que ficou para depois está em `NATIVO-PODA-DE-TABELAS.md` §6:

- a restrição pelo tipo do receptor;
- os ajudantes `_dartforge*` como raízes por biblioteca.

Os dois são o que mais pesa agora: `toString` puxa 187 KB do hello.

## 2. Desempenho

### Medida

| Carga | DartForge | Dart AOT |
|---|---:|---:|
| Servidor HTTP, CPU/req, 1 conexão | 180 µs | 102 µs |
| JSON (`bench/desempenho/json.dart`) | 1,3–7× o Dart AOT | 1× |

No servidor, cerca de 50 µs por requisição são E/S que os dois pagam. Detalhes nas seções
§9.10 e §10 de `NATIVO-PLANO.md`.

Perfil do JSON (tempo *self*):

| Categoria | % |
|---|---:|
| código Dart | 20 |
| consulta ao slot | 14 |
| coleta (21 inclusivo) | 14 |
| classe do valor (`value_class`, `is_subclass`) | 11,5 |
| `malloc` | 11 |
| alocação no heap | 8 |
| listas do runtime | 6 |
| RTI | 4,6 |

### Feito: propostas 1 e 2 (2026-09-30)

Detalhes, perfis e o que não foi verificado estão em `NATIVO-PLANO.md` §11. As medidas são
do AOT com `--optimize`, com execuções alternadas e a mediana da razão pareada.

| Medida | Antes | Depois | Razão pareada antes/depois | Dart AOT |
|---|---:|---:|---:|---:|
| JSON, tempo total (mín., 8 rep.) | 17,1 s | 13,9 s | 1,19 [0,98–1,39] | 3,5 s |
| JSON `decode_medio` (mín.) | 331 ms | 227 ms | 1,42 | 44 ms |
| JSON `encode_pequeno` (mín.) | 180 ms | 99 ms | 1,82 | 32 ms |
| HTTP, CPU/req, 1 conexão (mediana, 4 rep.) | 259 µs | 303 µs | 1,00 [0,74–1,10] | 157 µs |

- **Perfil do JSON (self):** a classe do valor cai de 12,6% para 0,7%, e a coleta de 10,0% para
  4,9%.
- **O "depois" inclui o que outras frentes mudaram na mesma árvore no intervalo.** A parte em
  linha sozinha, medida no mesmo binário com `DARTFORGE_SEM_CLASSE_EM_LINHA=1`, dá uma razão de
  1,07 no JSON total.
- **HTTP:** só 4 repetições válidas, com builds concorrentes. Não há ganho mensurável acima do
  ruído, o que concorda com o −2–3% estimado.
- **`CONTAGEM_JOVEM` não foi revista:** a medida foi interrompida.

### Feito: espaço unificado, fases B–D (integração, 2026-09-30)

Especificação em `NATIVO-ESPACO-UNIFICADO.md`; a tabela completa está em
`NATIVO-PLANO.md` §12.2. As medidas são do AOT com `--optimize`, com 5 rodadas
alternadas do `main` antes (`6f6d14bf`), do de depois e do Dart AOT 3.6.2, no
mínimo em ms. Saídas iguais às da VM.

| Medida | Antes | Depois | Dart AOT |
|---|---:|---:|---:|
| JSON `decode_medio` | 116,3 | 89,2 | 27,9 |
| JSON `decode_grande` | 297,3 | 172,0 | 77,8 |
| JSON `encode_medio` | 96,7 | 49,8 | 30,2 |
| JSON `utf8_bytes` | 207,4 | 101,2 | 31,0 |
| `textos/construir` / `hashes` | 108,5 / 45,6 | 70,0 / 33,9 | 49,1 / 27,7 |
| `int.toString` × 1 milhão | 88,8 | 37,4 | 11,5 |
| `Map<int,int>` (500 mil) | 87,5 | 113,3 | 41,7 |
| HTTP, CPU/req, 1 conexão (µs, mediana de 3) | 150,7 | 139,1 | 75,3 |
| HTTP, RSS em repouso (MB) | 16,2 | 19,3 | 17,0 |
| `hello` / servidor (bytes) | 10 786 816 / 12 213 760 | 10 711 552 / 12 075 520 | 5 797 376 / 6 108 160 |

- **Ganhos:** JSON de 1,3× a 2× mais rápido; o `encode` chega a 1,5–1,9× do Dart AOT. Textos
  ficam 1,2–1,4× o Dart AOT, e `int.toString` 2,4× mais rápido que antes.
- **Pioras:** os mapas de chave `int`. O `int.hashCode` passou a ser o da VM, que o programa 95 fixa
  (`7.hashCode == 81207`), e a chave sequencial deixou de cair em posição vizinha do `_index`.
  O HTTP melhora 8% em CPU/req; a memória em repouso sobe 3 MB (89 classes de tamanho).
- **O que a integração achou medindo:** 65% do runtime ia na conferência de handles no mapa de
  páginas (`bloco_vivo`). Ela passou a valer só no `--gc-stress`, na verificação e nos testes
  (item 63 de §4.10 da especificação).
- **Próximos alvos:**
  - o caminho de `_Map`/`_Set` com chave `int`: sonda mais barata e menos acessos ao `_index`/`_data`
    por operação;
  - o `decode` do JSON, que ainda está a 2,2–3,2× do Dart AOT;
  - a RSS das classes médias pouco usadas.
- **Feito depois (2026-09-30, `NATIVO-PLANO.md` §13):**
  - O `Map<int,int>` passou de 3,4× para 1,2× do Dart AOT (0,97× numa sessão quieta).
  - O `decode_medio` foi de 3,7× para 1,3×, e o `utf8_bytes` de 3,9× para 2,4×.
  - O que mudou: a sonda sem o `Heap`, a reinserção em lote, o listener do JSON com uma pilha
    única e a memória rápida do `is`.

### Causa raiz

Os objetos das classes do programa já seguem o desenho da VM do Dart:

- cabeçalho com o `cid`;
- alocação em linha por *bump pointer* (`assembler_x64.cc` `TryAllocateObject`);
- barreira de escrita em linha;
- raízes por vivacidade, no modelo da Julia.

Mas `String`, `List`, `Map`, `Set`, closures, ambientes, células, caixas de `double`/`int` e
listas tipadas moram em `Heap::slots: Vec<Option<Value>>`, com o conteúdo no `malloc`. Cada
acesso faz TLS, `RefCell`, índice e `match`. Cada objeto novo é um `malloc`, cada morto é um
`drop` com `free`. A coleta menor custa pelo que foi alocado, não pelo que sobreviveu. Juntos,
esses custos somam cerca de 55–60% do JSON e 17–20% da CPU do servidor.

### Propostas, por ordem

**Rápidas** (dias a 2 semanas cada, independentes; ganhos estimados):

1. **Classe por slot num vetor denso.** Os bits de fixa/imutável ficam no próprio vetor, e somem
   os `HashSet` `fixas`/`imutaveis`. O `df.classe` lê a classe em linha, e o `is C` sem
   argumentos de tipo vira consulta a uma tabela de bits `@df.sub.<C>[cid]`.
   - Referência: `LoadClassId` em `assembler_x64.cc`, faixas de cid em `class_finalizer.cc` e os
     type testing stubs.
   - Ganho: JSON −8–11%, HTTP −2–3%.
2. **Coleta menor sem recalcular `estimated_bytes` nem filtrar as tabelas laterais a cada
   coleta,** e revisão de `CONTAGEM_JOVEM`. Ganho: JSON −5–10%.
3. **RTI com cache de 2 vias,** caminho rápido em linha para `is`/`as` genérico (comparar o
   metadado do cabeçalho) e `rti_definir` com id pré-internado. Ganho: −2–4%.
4. **Contexto do isolado sem chamada.**
   - No AOT, uma `thread_local(initialexec)` do módulo.
   - Depois, `%ctx` como parâmetro oculto, como o `THR` da VM e o `llvm-ptls.cpp` da Julia.
   - Ganho: −1–2%.
5. **TBAA e laços de listas sem guardas,** que habilitam a vetorização do código escalar. Em
   andamento (frente de vetorização). A referência é o `LoadOptimizer` em
   `redundancy_elimination.cc`.

**Médias:**

6. **Tabela de despacho global** `[cid + deslocamento do seletor]` no módulo do programa. Casa com
   a proposta estrutural de tamanho e depende da proposta 1.
   - Referência: `dispatch_table_generator.cc` e `DispatchTableCallInstr` do Dart.
7. **Análise de escape na HIR** para caixas, closures, ambientes e listas pequenas que não
   escapam.
   - Referências: `llvm-alloc-opt.cpp` da Julia e `DeadObjectElimination.cpp` do Swift.

**Estrutural** (semanas por fase): valores do runtime no espaço de objetos, como os
`UntaggedOneByteString`, `UntaggedArray` e `UntaggedGrowableObjectArray` de
`dart-sdk/runtime/vm/raw_object.h`.

- **Fase B, strings:** `_OneByteString`/`_TwoByteString` com `length`/`hash` no corpo e unidades
  em linha. Os `int.toString` e `_JsonStringParser` da VM passam a servir como estão.
- **Fase C:** caixas de `double`/`Mint`, células, ambientes e closures, pela alocação em linha que
  já existe.
- **Fase D:** listas expansíveis com vetor de base no espaço e elementos de 8 B, mais mapas e
  conjuntos.
- **Ganho estimado:** JSON 2–3× mais rápido e HTTP −15–20% de CPU/req. A coleta que custa pelo
  alocado se resolve junto; a coleta continua sem mover objetos, como na Julia.

**Estado (2026-09-30).** As fases B, C e D viraram uma especificação só, de uma vez e sem os slots
convivendo com o espaço: `docs/NATIVO-ESPACO-UNIFICADO.md`. Ela troca o desenho incremental acima
por cids fixos para as classes do runtime (1–65), três formatos de corpo que o coletor percorre sem
olhar a classe (`INSTANCIA`, `BRUTO`, `REFS`), classes de tamanho médias e regiões grandes, cartões
nas listas grandes, literais de string estáticos no AOT e a cópia entre isolados por bloco; e tira
o caminho sem SDK da fonte (`sdk_por_nome.rs`, `DARTFORGE_SDK_DA_FONTE`). A implementação foi
dividida em pacotes paralelos (P0–P5, §4 da especificação) e é validada na integração (§5); os
números medidos entram em `NATIVO-PLANO.md` §12. O marcador de ABI de §4 abaixo entrou como a
conferência da tabela de cids (`dartforge_registrar_cids`).

### O que não fazer, com evidência

- **Trocar a pilha-sombra por *statepoints*:**
  - o fork do linzj gastou cerca de 32 dos 122 commits corrigindo *stack maps*;
  - o mraleph registra que o *statepoint* derrama tudo no chamador;
  - o Dartino registra perda de otimização com os `gc.relocate`;
  - com um coletor que não move, o ganho seria só as gravações de slot.
- **LTO do runtime Rust com o programa:** medida em `SIMD-NATIVO.md` §7.2, 41 → 38 ms por
  +1,5 min de build. Os *helpers* quentes continuam como IR `alwaysinline` (`df.*`).
- **O modelo de alocação do Swift:** ARC com `malloc` não é modelo para um runtime com coletor.

## 3. Ordem proposta

1. **Ganhos rápidos de tamanho:** feito em 2026-09-30 (§1, «Feito»). Entraram `panic=abort`,
   o `@df.area` enxuto, o `safeicf` e o `optsize`; o `opt-level="s"` foi rejeitado.
2. **Propostas 1 a 4 de desempenho,** baratas e independentes.
3. **Tabelas de métodos na ligação** (tamanho): feito em 2026-09-30 (§1, «Feito: tabelas montadas
   na ligação»). A tabela de despacho global (proposta 6) continua aberta.
4. **Fases B → C → D** dos valores no espaço de objetos: especificadas juntas em
   `NATIVO-ESPACO-UNIFICADO.md` (2026-09-30), implementadas em paralelo por pacote e medidas na
   integração (`NATIVO-PLANO.md` §12).

As ferramentas de medida reusáveis estão em `tools/tamanho/`: `quebra.py` e `por-que.py`. O
`alcance.py` da pesquisa virou `crates/emit_native/src/poda.rs`.

## 4. O que aproveitar do scriptc (Vercel Labs)

Conferido em `E:\references\scriptc` (revisão `0d9d946`), que compila TypeScript para C/LLVM com
runtime nativo próprio em C, contagem de referências e ilha QuickJS opcional (`--dynamic`). O
que já está neste plano não se repete aqui. O que ele acrescenta:

1. **Teste de ausência de código** (`tests/harness/deadstrip.test.ts`). Confere que corpo, wrapper
   assíncrono, instância genérica e entrada de vtable de algo inalcançável **não aparecem** no
   artefato, e que os mesmos construtos, quando alcançados, dão os mesmos diagnósticos.
   - Adaptar para o DartForge junto com as tabelas montadas na ligação. O caso é uma classe
     **instanciada** com métodos nunca chamados: o executável não pode conter nem o corpo nem os
     `$c`/`$tc`/`$g`/`$tearm` deles (conferido pelo mapa do ligador ou por `llvm-nm`), e o programa
     roda igual à VM.
   - Sem esse caso, o teste só provaria a eliminação de classes inteiras, que já existe.
   - **Feito (2026-09-30):** `corpus/nativo/81_poda_de_tabelas.dart` e o teste
     `sdk_modulo::testes::poda_tira_membros_nao_usados`, que confere pelo mapa da ligação.
2. **Marcador de ABI na ligação** (`packages/compiler/src/backend/runtime-abi.ts`). O objeto do
   programa deixa `scr_runtime_abi_v4` indefinido, e só o runtime compatível o define: a mistura
   falha na ligação, não em tempo de execução.
   - Aqui os hashes do cache já protegem o caminho normal. O marcador (ex.:
     `dartforge_runtime_abi_v<N>`) cobre ligações manuais e distribuições misturadas. Vale
     sobretudo **antes** das fases B/C/D, que mudam layouts.
   - Custo: horas.
3. **Plano do runtime por capacidades** (`packages/runtime-pack-common/runtime-pack-matrix.mjs`:
   unidades opcionais com predicados, e zlib, mbedTLS e QuickJS só quando a capacidade é
   efetiva).
   - Aqui o runtime é uma staticlib única por fragmentos (`crates/runtime/build.rs`), e o
     `/OPT:REF`/`--gc-sections` já descarta o código Rust não referenciado.
   - A pesquisa de tamanho mostrou que rustls, ring e zlib entram no hello só porque o código de
     `dart:io` do SDK está vivo pelas tabelas de métodos. Eles saem sozinhos com as tabelas
     montadas na ligação.
   - Então a seleção explícita por capacidade tem **prioridade baixa** como ganho de tamanho. Vale
     como relatório verificável: quais capacidades o executável liga e por quê.
4. **Relatório de "por que isto ficou no executável".** O `crates/mundo` já registra a causa de
   alcance (`Causa`: raiz, função, variável, classe). Estender isso ao SDK nativo e às tabelas
   responde a perguntas como "quem puxou este método?" e "por que TLS entrou no hello?".
   - Primeiro passo: trazer para `tools/` o `alcance.py` e o `quebra.py` da pesquisa.
   - **Feito para as tabelas (2026-09-30):** `DARTFORGE_POR_QUE=<arquivo>` grava a causa de cada
     símbolo vivo, e `tools/tamanho/por-que.py` segue a cadeia (`NATIVO-PODA-DE-TABELAS.md`
     §3.10). O runtime Rust fica de fora do relatório; o que ele mostra é qual função Dart cita
     o native.
5. **Inventário de compatibilidade ligado a evidência.** O scriptc separa suporte comprovado,
   parcial, recusa explícita e não revisado, e não trata "não está no registro" como prova de
   ausência. Aplicável por eixo (AOT, JIT, JS dev, JS produção, plataforma) sobre o diferencial
   que já existe.

**O que não copiar:**

- **A ilha QuickJS** (o equivalente seria embutir a VM).
- **Contagem de referências com coleta de ciclos no lugar do GC:** o perfil aponta representação e
  indireções, não o modelo de coleta.
- **A restrição de métodos genéricos fora da vtable:** o scriptc só especializa chamadas
  resolvidas estaticamente e recusa as demais, o que fere a compatibilidade com o Dart.
  Especialização aqui só como caminho adicional, com a entrada genérica sempre presente e um
  orçamento de versões.
- **Os arrays do runtime** (`packages/runtime/src/scr_array.c`, slots de 8 B com a espécie dos
  elementos no contêiner): a ideia de representação confirma a fase D, mas o código atende
  semântica de JS (buracos, propriedades) e usa `realloc`.

**Comparação de tamanho.** O hello de ~320 KB divulgado pelo scriptc é macOS, ligado à
libSystem, com runtime de escopo menor. Não é comparável aos números do Windows deste
documento.
