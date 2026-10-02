# Produção (`aot --optimize`) de um programa grande: tempo e tamanho

Especificação e medidas para o `aot --optimize` do `new_sali/backend`. A
referência é o `dart compile exe` do mesmo programa, medido na mesma máquina
(i3-1215U, 7,7 GB, Windows 11):

* **58–60 s**;
* executável de **27,3 MB** (snapshot AOT de 22,3 MB + o `dartaotruntime`).

**Metas:**

* tempo: até ~2× o do Dart (≤ ~2 min com o SDK de produção em cache);
* executável: ≤ 27 MB;
* desenvolvimento frio: até ~2×.

`EN/` é `crates/emit_native/src/`, `VM/` é `E:\references\dart-sdk\`, `TF/` é
`VM/pkg/vm/lib/transformations/type_flow/`. Escrito em 2026-10-01.

## 1. Medidas de partida

### 1.1 Tempo e tamanho

`aot --optimize` (commit 52c461d0 + C12–C20; build sem LTO do dartforge):

| fase | tempo |
|---|---:|
| front-end | 9,3 s |
| mundo | 5,5 s |
| HIR | 30,8 s |
| LLVM IR (1,31 GB de texto) | 20,8 s |
| "objeto": 77 bitcodes `lto-pre-link<O2>` | 271 s |
| ligação (LTO do `lld`) | 1 600 s |
| **total** | **1 983 s** |

* Executável: **94,0 MB**.
* Pico de memória: 4,3 GB.

### 1.2 Por que a ligação leva 1 600 s

O `dartforge` com a feature `jit` gera os objetos pelo LLVM embutido
(`EN/gerador.rs`, `Gerador::escolher`). O bitcode dele **não leva o resumo do
ThinLTO**: a API C do LLVM não escreve o índice (`Gerador::bitcode_thin`).

Por isso o `lld` faz a LTO **completa**:

* junta as 77 partes e o SDK num módulo só;
* otimiza esse módulo em uma thread;
* só depois divide a geração de código em partições.

O comentário do C9 ("ThinLTO entre as partes") só vale com
`DARTFORGE_GERADOR=clang`.

### 1.3 O que a poda deixa vivo, contra o que o Dart retém

Fontes:

* **Nosso lado:** o relatório `DARTFORGE_POR_QUE` da montagem das tabelas
  (`EN/poda.rs`, o RTA por seletor sobre o IR).
* **Dart:** `--print-instructions-sizes-to` do `gen_snapshot` (o `dart compile
  aot-snapshot` do mesmo programa).

O IR do programa tem 196 313 funções (1 255 MB de texto), das quais **64 801
vivas (798 MB)**. As outras 131 512 (457 MB, 36%) só saíam na LTO, depois de
otimizadas e geradas.

Funções retidas por pacote:

| pacote | Dart: funções | Dart: KB de código | nós: corpos vivos | nós: adaptadores vivos |
|---|---:|---:|---:|---:|
| dart_pdf | 3 168 | 1 521 | 3 562 | 1 504 |
| dargres | 329 | 1 404 | 276 | 235 |
| new_sali_backend | 2 538 | 1 156 | 2 161 | 2 402 |
| `dart:*` | 4 958 | 1 128 | 2 659 | 4 375 |
| unorm_dart | 35 | 927 | 53 | 17 |
| new_sali_core | 1 368 | 863 | 1 947 | 772 |
| pointycastle | 1 022 | 724 | 905 | 460 |
| image | 1 728 | 638 | 1 707 | 1 280 |
| pdf_plus | 1 660 | 511 | 1 918 | 1 385 |
| **total** | **23 536** | **10 711** | | |

**Conclusão: o número de funções retidas é da mesma ordem que o do Dart, e
quase sempre menor.** O TFA do Dart não retém menos funções do que a nossa
poda. A diferença de tamanho (94 MB contra 22 MB) está no **código por
função**, não no alcance.

O TFA (`TF/analysis.dart`; resumo em §4) continua valendo pelo que faz dentro
de uma função viva: desvirtualiza, troca chamadas inalcançáveis por `throw`,
tira campos só escritos. Mas ele não é a alavanca de tamanho deste programa.

### 1.4 Do que é feito o código vivo

São 19,2 milhões de linhas de IR nas funções vivas. As categorias saem do
prefixo do nome SSA que o emissor dá (`EN/llvm/mod.rs`):

| padrão | linhas | % | o que é |
|---|---:|---:|---|
| endereço dos campos (`%fx*`, `%fcb`, `%fp`) | 5,6 M | 29% | 331 651 cópias de 17 linhas (`emitir_endereco_dos_campos`): null/`Smi` → objeto de reserva, bit `FORA` → corpo realocado |
| mapa de referências na gravação de campo (`%fm*`) | 0,7 M | 4% | 178 547 gravações; o bit `is_ref` do campo no cabeçalho |
| `store` | 2,4 M | 13% | os slots da pilha de raízes (GC) e os campos |
| exceção pendente depois de chamada (`load i8 %ctx`, `zext`, `icmp`, `br`) | ~2,2 M | ~12% | ~560 mil verificações |
| moldura de GC (`%gc*`, `dartforge_contexto`) | ~0,7 M | 4% | por função |
| o resto | ~7,6 M | ~40% | a computação |

## 2. Causas

| | causa | efeito | onde |
|---|---|---|---|
| P1 | LTO completa (gerador embutido sem resumo ThinLTO) | 1 600 s de ligação, numa thread | `EN/gerador.rs`, `EN/ligador_windows.rs` |
| P2 | o IR morto só sai na LTO | 36% do IR do programa (457 MB) otimizado e gerado à toa | `EN/poda.rs`, `montar` |
| P3 | endereço dos campos em linha | 29% das linhas vivas | `EN/llvm/mod.rs`, `emitir_endereco_dos_campos` |
| P4 | verificação de exceção pendente depois de cada chamada | ~12% | o lowering das chamadas |
| P5 | pilha de raízes explícita (slots e moldura) | ~15% | `EN/llvm/gc_raizes.rs` |

## 3. Desenho e ordem

Feito (as medidas estão na §5):

1. **P2: o IR morto sai antes da geração.**
   * `poda::montar` já calculava o alcance. Com a poda, as definições do programa (funções e
     globais) que o ponto fixo não alcançou não vão ao texto montado (§3.12 de
     `NATIVO-PODA-DE-TABELAS.md`).
   * É a mesma decisão que a LTO e o `/OPT:REF` tomariam: as mesmas raízes (`main`,
     `dartforge_*`, `llvm.*`) e as mesmas arestas, com todo `@nome` citado.
   * Teste: `poda::testes::definicoes_mortas_do_programa_saem_do_ir`.
2. **P1: ThinLTO de verdade.**
   * Na produção em partes, o bitcode das partes sai do Clang (`EN/driver.rs`,
     `gerador_das_partes`), com o resumo do ThinLTO. O SDK de produção também sai do Clang, em
     ThinLTO e no mesmo cache (§6.1).
   * A ligação usa o cache do ThinLTO (`driver::cache_do_thinlto`: `/lldltocache`,
     `--thinlto-cache-dir`, `-cache_path_lto`). Numa religação, só os módulos que mudaram passam
     de novo pela otimização e pela geração de código.
   * Medido e descartado: o programa sem LTO (objeto `-O2` por parte) ganha pouco tempo (~20%)
     e perde a inlining entre programa e SDK.
3. **P3: o endereço dos campos e os ajudantes fora de linha no programa grande**
   (`lib.rs`, `FUNCOES_DO_PROGRAMA_GRANDE`; `llvm::ajudantes_fora_de_linha`).
   * `@df.corpo` vai `noinline memory(read)`; `seletor`, `classe`, `subclasse`, `alocar`,
     `barreira`, `obter_area` e a alocação de instância (`@df.nova_instancia`) também vão
     `noinline`.
   * O despacho passa um descritor estático (`@df.seld.<k>`) ao `@df.seletor_d`. A poda lê o
     seletor do descritor (`poda::chamada_de_seletor_compacta`).
   * `optsize` em toda função do programa grande.
   * O preço em desempenho está na §5.1. No desenvolvimento (`-O0`) do programa grande, os
     mesmos ajudantes vão fora de linha. Lá o `alwaysinline` copiava cada um em cada uso, e a
     geração pagava as cópias: os objetos do backend caem de ~95 para ~80 s.
   * No programa pequeno, os ajudantes ficam em linha também no `-O0`. Fora de linha, o
     `corpus/nativo` levou 116 s em vez de 53 s.

4. **HIR em paralelo.**
   * O lowering vai em pedaços de 512 funções distribuídos sob demanda entre até 4 threads,
     cada pedaço no seu módulo, juntados na ordem (`lower::baixar_funcoes`). O contexto
     (`Context`) passou a `Sync`: `OnceLock` e memórias em `RwLock`/`Arc`.
   * Os passes por função da otimização (`otimizar::em_paralelo`) também vão em paralelo.
   * O resultado é o mesmo da ordem sequencial, e os objetos em cache batem.
   * No backend (desenvolvimento): baixar 10,5–15 → 8,5–9 s; otimizar 21–25 → 12–16 s.
   * Medido e descartado: a emissão do IR em paralelo (trabalhadores com registros próprios)
     não ganhou nada, 20–35 s nas duas formas. O tempo é de alocação (`String`, `format!`), e o
     heap do Windows serializa as threads.
5. **A poda da HIR antes da otimização e da emissão** (`poda::podar_hir`, §3.13 de
   `NATIVO-PODA-DE-TABELAS.md`). O mesmo grafo da montagem da ligação, com os resumos do SDK
   (gravados também no desenvolvimento), tira da HIR o que a ligação tiraria depois. As funções
   mortas deixam de ser otimizadas, emitidas e geradas, nos dois perfis.

Falta, por ordem de ganho estimado:

6. **O piso do front-end e do IR**: guardar o HIR/IR por biblioteca de pacote entre
   compilações. O resultado da poda depende do programa todo; a chave tem de incluí-la.
7. **Forma do código** (§1.4), em especificação própria:
   * a verificação de exceção (`cmpb $0,(ctx); jne`, ~400 mil no executável) num bloco comum
     por função;
   * `this` sabido objeto, sem a seleção null/`Smi`;
   * o corpo de objeto sem indireção (`FORA` só existe para objeto que cresce: a recarga do JIT
     e `garantir_campos`), o que deixaria o campo em linha barato;
   * o registro das classes (`df.registrar.programa`, 620 KB) e o `df.preparar_isolado`
     (340 KB) como dados em vez de código;
   * as constantes (`dfc.*`, 5 MB) como objetos estáticos da imagem, como os textos já são.
8. **TFA** (§4), depois de 7: o número de funções retidas já é o do Dart.

## 4. O TFA do Dart (referência)

O TFA é uma análise de fluxo de tipos por invocação, com resumos por membro
e ponto fixo por lista de trabalho com invalidação:

* invocação: `TF/analysis.dart:78-157`;
* `_DispatchableInvocation.process`: `TF/analysis.dart:538-666`;
* lista de trabalho: `TF/analysis.dart:1913-2084`.

Os alvos de uma chamada são os das classes **alocadas** que chegam ao
receptor (`:669-746`). Antes dele, um RTA por seletor e tipo estático
(`TF/rta.dart:168-194`, chamado em `TF/transformer.dart:94-111`) dá as
classes alocadas iniciais. Esse RTA é o equivalente do nosso `crates/mundo` e
do `EN/poda.rs`.

O que o TFA acrescenta, e que vale portar por ordem de custo:

1. **Formas de entrada separadas** (getter, setter, inicializador, tear-off):
   `TF/analysis.dart:1255-1262`, e `VM/runtime/vm/compiler/aot/precompiler.cc:1728-1737`.
   Nós já temos isso por seletor (`$tear`, `$c`, `$g` são pares próprios).
2. **Chamada inalcançável vira `throw`**, e o membro usado sem corpo
   alcançável vira abstrato: `TF/transformer.dart:1432-1450` e `:2358-2405`.
3. **Campo só escrito sai**: `TF/transformer.dart:1060-1070`.
4. **Valores por campo** (união das gravações): `TF/analysis.dart:1249-1386`.
5. **Receptor por fluxo de valores** (resumos, reticulado de tipos, lista de
   trabalho com dependências): `TF/summary.dart`, `TF/summary_collector.dart`,
   `TF/types.dart`. É o caro.

A tabela da §1.3 mostra que, neste programa, o ganho de alcance do TFA é
pequeno perto da forma do código (§1.4). Por isso o TFA fica depois de P1–P5.

## 5. Resultados

`aot --optimize` do backend, uma variável por vez, na mesma máquina. Os objetos das partes vêm
frios (o IR muda a cada passo); o SDK de produção está em cache.

| passo | objetos | ligação | total | executável |
|---|---:|---:|---:|---:|
| partida (gerador embutido, LTO completa) | 271 s | 1 600 s | 1 983 s | 94,0 MB |
| Clang ThinLTO | 302 s | 455 s | 855 s | 94,6 MB |
| + IR morto fora antes da geração (P2) | 160 s | 419 s | 707 s | 94,6 MB |
| + endereço dos campos por chamada (P3) | 140 s | 378 s | 633 s | 89,1 MB |
| + ajudantes fora de linha (`seletor`, `subclasse`, `alocar`, `barreira`, `obter_area`, `classe`) | 107 s | 235 s | 463 s | 70,9 MB |
| + alocação de instância fora de linha (`nova_instancia`) e conferência da pilha (C22) | 102 s | 259 s | 511 s | 71,0 MB |
| + `optsize` nas funções do programa | 103 s | 229 s | 467 s | 64,8 MB |
| + despacho compacto (descritor `@df.seld.<k>`: 3 argumentos em vez de 5) — **o padrão** | 77 s | 225 s | **405 s** | **63,4 MB** |
| o padrão, religado sem mudança (objetos e ThinLTO em cache) | 1,7 s | 41 s | 148 s | 71,0 MB¹ |

¹ A religação quente foi medida no IR da linha de 511 s.

Por comparação, o `dart compile exe` do mesmo programa leva 58–60 s e dá 27,3 MB. O padrão
fica em 6,8× o tempo do Dart com os objetos frios, 2,5× quente, e 2,3× o tamanho. O piso de
tempo é o front-end mais o HIR e o IR (75–90 s, numa thread só). O executável do padrão passa no
e2e do backend como antes: 39 de 42 rotas iguais à VM. As 3 que diferem são o texto de pilha
dentro do JSON de erro (2) e a contagem da auditoria no banco compartilhado (1).

Nas mesmas medidas, o resto do tempo fica constante: front-end 10 s, mundo 5,5 s, HIR 35 s e
LLVM IR 22–30 s.

Fora de linha, o corpo médio de função do programa cai de 1 748 para 1 273 bytes. Do mapa da
ligação, `tools/tamanho/quebra.py` e `tam/mapa_cat.py`:

| forma | partida | com P2+P3+ajudantes |
|---|---:|---:|
| corpos (29,5 mil) | 51,6 MB | 37,8 MB |
| `$async` | 13,0 MB | 8,4 MB |
| constantes (`dfc.*`) | 8,0 MB | 5,1 MB |
| adaptadores | 6,1 MB | 5,1 MB |

### Desenvolvimento frio

`aot` do backend sem `--optimize`, com o SDK em cache e os objetos do programa frios
(`DARTFORGE_CACHE_OBJ=0`), em duas rodadas:

| fase | antes | com a HIR em paralelo e a poda da HIR |
|---|---:|---:|
| front-end + mundo | 14 s | 12–15 s |
| poda da HIR | — | 1,5–2,2 s (64 661 de 183 440 funções ficam) |
| HIR (baixar + otimizar) | 31–48 s | 19–27 s |
| LLVM IR | 18–33 s | 13–15 s |
| objetos (`-O0`) | 79–100 s | 45 s |
| **total** | **157–185 s** | **91–101 s** |

O total fica em 1,5–1,7× os 58–60 s do `dart compile exe`, dentro da meta de ~2×. Na
produção, a poda da HIR não muda os objetos nem a ligação: a montagem já tirava o mesmo IR. Ela
só poupa a otimização e a emissão.

### 5.1 O preço em desempenho

Cada `bench/desempenho` foi compilado em produção duas vezes, A (tudo em linha) e B (campos e
ajudantes por chamada), e as duas versões rodaram três vezes cada, alternadas. A máquina tem
outros builds rodando. Os números são os mínimos por medida, em µs:

| medida | A | B (tudo fora) | C (fora, menos `seletor`/`classe`) |
|---|---:|---:|---:|
| `chamadas` formas | 4 316 | 10 524 | 7 242 |
| `chamadas` closures | 29 748 | 42 146 | 33 090 |
| `chamadas` fib | 2 098 | 2 130 | 2 288 |
| `objetos_em_colecoes` ordenar | 110 125 | 118 630 | 116 585 |
| `colecoes`, `json`, `textos`, `numerico` | — | dentro do ruído | dentro do ruído |

* O despacho fora de linha custa 1,4–2,5× nas chamadas virtuais.
* O endereço de campo por chamada custa ~1,7× no código que lê muitos campos (C ainda tem os
  campos por chamada).
* O que só aloca, grava com barreira ou testa subtipo não muda.
* Nos programas pequenos o executável quase não muda (3,40 contra 3,40 MB no `chamadas`): a LTO
  já tirava o que sobra.

### 5.2 Os padrões

| perfil | campos | ajudantes fora de linha | ligação das partes |
|---|---|---|---|
| desenvolvimento (`-O0`), programa pequeno | por chamada | nenhum | objetos |
| desenvolvimento (`-O0`), programa grande | por chamada | todos (`seletor`, `classe`, `subclasse`, `alocar`, `barreira`, `obter_area`, `nova_instancia`) | objetos |
| produção, programa pequeno | em linha | nenhum | um módulo, LTO com o SDK |
| produção, programa grande (≥ 50 mil funções HIR) | por chamada | todos | Clang ThinLTO, com cache |

No programa grande o padrão é o tamanho. Uma decisão do dono, depois da medida: 33 min e 94 MB
eram inaceitáveis. O preço está na §5.1. Para voltar ao código em linha:
`DARTFORGE_CAMPOS_POR_CHAMADA=0` e `DARTFORGE_AJUDANTES_FORA=` (vazio). Também dá para escolher
os ajudantes um a um (`DARTFORGE_AJUDANTES_FORA=alocar,barreira`).

Achado no caminho: com as partes do programa em ThinLTO e o SDK em LTO completa, a divisão do
módulo do SDK em partições (`/opt:lldltopartitions`) promove o `@df.area_id` interno de um
módulo do SDK com o mesmo nome do `@df.area_id` que `particao.rs` promove a externo no
programa: "duplicate symbol: df.area_id". O programa agora usa `@df.area_id.programa`.

## 6. Segunda rodada: a ligação medida, o corpo, a FFI e as constantes

Tudo medido no new_sali/backend, `aot --optimize`, na mesma máquina: 4 núcleos e 8 threads,
7,7 GB de RAM, dividida com outros builds. Os tempos de relógio oscilam 10–20% com a carga, e
as rodadas com a máquina cheia estão marcadas. Os tempos de CPU vêm do perfil da ligação e
variam menos.

### 6.1 Onde o tempo da ligação vai

O perfil é o `--time-trace` do `lld` (`DARTFORGE_LTO_PERFIL=<arquivo.json>`). Com a ligação
fria (objetos em cache, cache do ThinLTO vazio) e 3 tarefas, o ThinLTO gasta 771 s de CPU
(266 s de relógio):

* otimização: 275 s de CPU;
* geração de código: 466 s de CPU, com a seleção de instruções (`X86 DAG->DAG`) à frente.

Os dois maiores corpos são os dois `unormdata` (o mapa literal não constante do `unorm_dart` e a
cópia dele no `dargres`): 12,8 e 10,5 s de otimização. Nenhuma outra função passa de 6 s. O
custo está espalhado pelo código, proporcional ao volume de IR.

Também medido:

* **SDK de produção em ThinLTO.** O bitcode do SDK agora sai do Clang (`sdk_modulo.rs`), e o SDK
  também entra no cache. Uma religação sem mudança cai de 148 s para 57–65 s, com a ligação em
  3,5 s.
* **Uma edição pequena** (um texto num arquivo do backend): o IR muda 7 linhas, porque os nomes
  são estáveis (hash do conteúdo). Mesmo assim a ligação leva 217 s e refaz os 43 módulos. A
  chave do cache do ThinLTO de um módulo inclui o hash de cada módulo de que ele importa, e a
  mudança se propaga.
* **6 tarefas em vez de 3:** 206 → 161 s de relógio, mas 591 → 915 s de CPU (são 4 núcleos
  físicos) e pico de 1,9 GB contra 1,25 GB. O padrão continua 3.
* **`-import-instr-limit=10`:** 550 contra 535 s de CPU, sem efeito.

### 6.2 LTO `-O1` no programa em partes

Ligação fria com os objetos em cache:

| variante | CPU do ThinLTO (otimização / código) | ligação | executável |
|---|---:|---:|---:|
| `-O2` (antes) | 771 s (275 / 466) | 266 s | 63,4 MB |
| **`-O1`** | 588 s (150 / 414) | 205 s | 63,6 MB |
| `-O2` + `-fast-isel` | 544 s (240 / 276) | 191 s | 66,7 MB |
| `-O1` + pré-ligação `-O0` | 730 s (257 / 444) | 253 s | 63,9 MB |
| `-O1` + `-fast-isel` + pré-ligação `-O0` | 571 s (250 / 293) | 202 s | 67,9 MB |

* A pré-ligação `-O0` gera o bitcode das partes sem otimizar: os objetos caem de 77 para 22 s.
  Mas o ThinLTO recebe o IR cru e gasta mais, então o saldo de CPU fica igual.
* O `-fast-isel` paga 3–4 MB de executável.

O `bench/desempenho` (mínimo de 3 rodadas por medida) fica entre 0,94 e 1,09× com `-O1` contra
`-O2`, dentro do ruído da máquina.

Padrão: `-O1` na ligação do programa em partes (`driver::nivel_da_lto`); `-O2` no programa
pequeno. Ficam como chaves de medida: `DARTFORGE_LTO_NIVEL`, `DARTFORGE_PRELINK_O` (o nível da
pré-ligação das partes) e `DARTFORGE_LTO_MLLVM` (opções do LLVM na ligação, Windows).

### 6.3 O corpo do objeto sem dependência de memória

O `@df.corpo` (o endereço dos campos, por chamada no programa grande) passa de `memory(read)` a
`memory(none)`. O otimizador junta todas as chamadas sobre o mesmo objeto, mesmo com gravações e
chamadas no meio, como no quadro do `$async` e no `this`. Antes eram 332 mil chamadas.

Isso é correto porque o corpo de um objeto não muda durante a vida dele:

* o coletor não move objetos;
* o corpo de fora (`FORA`) só nasce na migração da recarga do JIT, que nunca é produção;
* o outro caso, o erro lançado sem o campo do rastro, deixou de criar corpo de fora. O
  `dartforge_exception_throw` só grava o rastro quando o campo existe.

Resultado: executável 63,6 → 60,8 MB; CPU do ThinLTO 591 → 574 s.

### 6.4 A FFI pelas assinaturas pedidas

Ver `NATIVO-PODA-DE-TABELAS.md` §3.14. Saem 3 547 das 3 591 assinaturas: 7 094 funções HIR, das
64 661 vivas ficam 57 567. Executável 60,8 → 59,0 MB.

### 6.5 Literais constantes grandes por tabela

No contexto constante, um literal grande (8 ou mais elementos) cujos elementos são outras
constantes vai para a tabela de dados que já servia aos escalares. Cada elemento entra como
`g` + índice num vetor de endereços dos getters canônicos (`Instruction::TabelaDeFuncoes`,
`@df.fns.<k>`), que o runtime chama (`dartforge_lista_de_tabela_g`, com a lista enraizada).

Antes, os 234 getters de constante que não cabiam na tabela somavam 30 MB de IR: um `[]=` por
seletor por entrada, cada um com o seu vetor de argumentos e a sua checagem de exceção.

* As funções HIR baixadas sobem de 183 mil para 201 mil (um getter por constante aninhada
  distinta). As vivas sobem 160.
* Executável 59,0 → 57,2 MB.
* Teste: `corpus/nativo/137_constantes_grandes_por_tabela.dart` (identidade canônica,
  imutabilidade, o caso não constante), igual à VM em desenvolvimento, produção e `gc-stress`.

### 6.6 Resultado e o que resta

`aot --optimize` frio (objetos e cache do ThinLTO vazios, `DARTFORGE_CACHE_OBJ=0`), SDK em cache:

| passo | objetos | ligação | total | executável |
|---|---:|---:|---:|---:|
| padrão da §5 | 77 s | 225 s | 405 s | 63,4 MB |
| + SDK ThinLTO, LTO `-O1`, corpo `memory(none)`, FFI | 95 s | 178 s | 340 s | 59,0 MB |
| + constantes por tabela (máquina cheia: front-end 36 s contra 10 s) | 144 s | 265 s | 560 s | 57,2 MB |
| religação sem mudança | 1,6 s | 3,5 s | 57–65 s | — |

O e2e do backend continua com 39 de 42 rotas iguais à VM, com as mesmas 3 diferenças da §5.

O que falta para a meta (≤ ~2 min, ≤ 27 MB) não é bandeira do LLVM: com 3 tarefas, a ligação fria
fica perto de 550 s de CPU (≥ 180 s de relógio). O `.text` tem 55 MB em 58 mil funções. Contra
o Dart (§1.3: 10,7 MB de código para 23,5 mil funções), o volume de código por função ainda é a
diferença. No IR vivo:

| padrão | ocorrências |
|---|---:|
| volta do quadro de raízes em cada saída (`%gcvolta`, 2 linhas) | 396 mil saídas |
| gravação de raiz no quadro | 674 mil |
| checagem da exceção pendente depois de chamada | 495 mil |
| mapa de referências na gravação de campo | 238 mil |
| funções `$async` | 1 287 funções, 76,5 MB de IR |

São as causas P4 e P5 da §2: a pilha-sombra de raízes e a exceção por bandeira. Cada passo de
forma que não mexe nelas dá 3–5%.
