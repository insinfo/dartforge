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
`DARTFORGE_GENERATOR=clang`.

### 1.3 O que a poda deixa vivo, contra o que o Dart retém

Fontes:

* **Nosso lado:** o relatório `DARTFORGE_WHY` da montagem das tabelas
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
| moldura de GC (`%gc*`, `dartforge_context`) | ~0,7 M | 4% | por função |
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
   * `@df.body` vai `noinline memory(read)`; `seletor`, `classe`, `subclasse`, `alocar`,
     `barreira`, `obter_area` e a alocação de instância (`@df.new_instance`) também vão
     `noinline`.
   * O despacho passa um descritor estático (`@df.seld.<k>`) ao `@df.selector_d`. A poda lê o
     seletor do descritor (`poda::chamada_de_seletor_compacta`).
   * `optsize` em toda função do programa grande.
   * O preço em desempenho está na §5.1. No desenvolvimento (`-O0`) do programa grande, os
     mesmos ajudantes vão fora de linha. Lá o `alwaysinline` copiava cada um em cada uso, e a
     geração pagava as cópias: os objetos do backend caem de ~95 para ~80 s.
   * No programa pequeno, os ajudantes ficam em linha também no `-O0`. Fora de linha, o
     `corpus/native` levou 116 s em vez de 53 s.

4. **HIR em paralelo.**
   * O lowering vai em pedaços de 512 funções distribuídos sob demanda entre até 4 threads,
     cada pedaço no seu módulo, juntados na ordem (`lower::baixar_funcoes`). O contexto
     (`Context`) passou a `Sync`: `OnceLock` e memórias em `RwLock`/`Arc`.
   * Os passes por função da otimização (`optimize::em_paralelo`) também vão em paralelo.
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
   * o registro das classes (`df.register.program`, 620 KB) e o `df.prepare_isolate`
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
classes alocadas iniciais. Esse RTA é o equivalente do nosso `crates/world` e
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
ligação, `tools/size/breakdown.py` e `tam/mapa_cat.py`:

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

Cada `bench/performance` foi compilado em produção duas vezes, A (tudo em linha) e B (campos e
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
`DARTFORGE_FIELDS_BY_CALL=0` e `DARTFORGE_OUTLINED_HELPERS=` (vazio). Também dá para escolher
os ajudantes um a um (`DARTFORGE_OUTLINED_HELPERS=allocate,barrier`).

Achado no caminho: com as partes do programa em ThinLTO e o SDK em LTO completa, a divisão do
módulo do SDK em partições (`/opt:lldltopartitions`) promove o `@df.area_id` interno de um
módulo do SDK com o mesmo nome do `@df.area_id` que `particao.rs` promove a externo no
programa: "duplicate symbol: df.area_id". O programa agora usa `@df.area_id.program`.

## 6. Segunda rodada: a ligação medida, o corpo, a FFI e as constantes

Tudo medido no new_sali/backend, `aot --optimize`, na mesma máquina: 4 núcleos e 8 threads,
7,7 GB de RAM, dividida com outros builds. Os tempos de relógio oscilam 10–20% com a carga, e
as rodadas com a máquina cheia estão marcadas. Os tempos de CPU vêm do perfil da ligação e
variam menos.

### 6.1 Onde o tempo da ligação vai

O perfil é o `--time-trace` do `lld` (`DARTFORGE_LTO_PROFILE=<arquivo.json>`). Com a ligação
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

O `bench/performance` (mínimo de 3 rodadas por medida) fica entre 0,94 e 1,09× com `-O1` contra
`-O2`, dentro do ruído da máquina.

Padrão: `-O1` na ligação do programa em partes (`driver::nivel_da_lto`); `-O2` no programa
pequeno. Ficam como chaves de medida: `DARTFORGE_LTO_LEVEL`, `DARTFORGE_PRELINK_O` (o nível da
pré-ligação das partes) e `DARTFORGE_LTO_MLLVM` (opções do LLVM na ligação, Windows).

### 6.3 O corpo do objeto sem dependência de memória

O `@df.body` (o endereço dos campos, por chamada no programa grande) passa de `memory(read)` a
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
`@df.fns.<k>`), que o runtime chama (`dartforge_list_from_table_g`, com a lista enraizada).

Antes, os 234 getters de constante que não cabiam na tabela somavam 30 MB de IR: um `[]=` por
seletor por entrada, cada um com o seu vetor de argumentos e a sua checagem de exceção.

* As funções HIR baixadas sobem de 183 mil para 201 mil (um getter por constante aninhada
  distinta). As vivas sobem 160.
* Executável 59,0 → 57,2 MB.
* Teste: `corpus/native/137_constantes_grandes_por_tabela.dart` (identidade canônica,
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

## 7. Terceira rodada: macOS arm64 (31/10/2026)

Máquina: Apple M4 (4 núcleos de desempenho + 6 de eficiência, sem SMT), 16 GB, macOS,
LLVM 22.1.8, Dart 3.6.2. A máquina estava dividida com outros cinco agentes (carga média
30–130 em 10 núcleos), então o número principal é o **tempo de CPU** (usuário + sistema do
processo e dos filhos: Clang e `ld64.lld`); o tempo de relógio vai junto, com a carga do
início de cada medida. As medidas do DartForge e do `dart compile exe` foram alternadas. O
`--timings` passou a dar a CPU de cada fase (`dartforge_instrument::cpu_time`), e o
`DARTFORGE_LTO_PROFILE` também vale no macOS (`--time-trace` do `ld64.lld`).

Cópias de trabalho em `$SCRATCH/work` (o original fica só leitura): `new_sali/backend`
(`bin/prod.dart`) e `dart_ui/examples/file_explorer` (`bin/file_explorer.dart`), com `dart
pub get --offline`. "Objetos frios" é `DARTFORGE_CACHE_OBJ=0` (sem cache de objetos nem do
ThinLTO); "SDK em cache" é o SDK de produção já compilado pelo mesmo binário (uma
compilação de "olá" antes).

### 7.1 Onde o tempo ia (partida, `aot --optimize`, objetos frios)

| fase | CPU | observação |
|---|---:|---|
| front-end | 12,6 s | `const_eval::Engine::equal_states`: mapa constante montado por busca linear de chave |
| HIR (baixar + poda + otimizar) | 86 s | `FnBuilder::emit` procurava o bloco atual varrendo todos os blocos |
| LLVM IR | 19,9 s | 588 MB de texto |
| objetos (Clang `-O2 -flto=thin`, 32 partes) | 358 s | duas partes de 22 MB levavam minutos cada |
| ligação (ThinLTO `-O1`, 3 tarefas) | 935 s | |
| **total** | **~1 410 s** | `dart compile exe`: **41 s** de CPU |

O perfil da ligação (`--time-trace`) mostrou que **59% do tempo das tarefas do ThinLTO**
ia para três módulos:

* os dois `unormdata` (o mapa literal não constante do `unorm_dart`, e a cópia dele dentro do
  `dargres`): uma função de 470 mil linhas de IR cada, 22 MB de texto; `Machine code
  sinking` sozinho levou 690 + 628 s de tarefa (é quadrático em função enorme);
* a última parte, com o `df.register.program` (28 mil registros de classe numa linha
  reta de chamadas): gerado à parte (`clang -c -O1` do bitcode da parte), o *machine
  outliner* gastou 141,6 s de CPU contra 2,4 s sem ele, para um objeto 153 KB menor; com só o
  `df.register.program` marcado `"nooutline"`, 17,3 s e um objeto do mesmo tamanho (4 720 032
  bytes): o *outliner* procurava ali sem tirar nada.

E o executável (43,5 MB) tinha **33 cópias** do descritor da área (`@df.area`, 76 KB cada,
2,36 MB): o ajudante `df.get_area` é `private` e vai copiado para cada parte, e com ele a
constante que ele cita.

### 7.2 O que mudou

1. **Literais aninhados como dados** (`lower/literals.rs`, `fill_from_table`). Um literal
   grande (8 ou mais elementos) cujos elementos são escalares constantes ou outros literais
   sem `const` feitos só disso (`{0: {60: [null, null, {824: 8814}], …}}`) vira a tabela de
   dados que já servia aos escalares. Cada literal aninhado entra como `g` + índice de uma
   função que cria a coleção vazia do tipo estático dele (com a RTI), e uma segunda tabela
   (a forma) diz quantos dos valores seguintes vão para ela; `_dartforgeFillNested`
   (`native_sdk/collection/compact_hash.dart`) monta a árvore na ordem. É o mesmo
   resultado: coleções novas e mutáveis, com os mesmos tipos e conteúdo, na mesma ordem (os
   elementos não rodam código Dart; uma chave de mapa nunca é literal aninhado; tipo com
   parâmetro de tipo, ARC ou contexto constante ficam no caminho de antes). Os inteiros da
   tabela agora usam 1, 2 ou 4 bytes quando cabem (`b`, `h`, `w`;
   `runtime/src/natives_lists.rs`). Teste: `corpus/native/175_nested_literals_by_table.dart`
   (igual à VM em desenvolvimento, produção, `gc-stress` e wasm). O `compact_hash.dart` mudou,
   então o hash dele no manifesto revisado do SDK
   (`lower/double_identity/selected_sdk_manifest.json`) também: o manifesto prende o conteúdo
   de cada fonte do SDK, e sem ele as provas de `double`, `clamp` e multiplicação se recusam (e
   desligam essas otimizações).
2. **Constante local grande definida uma vez** (`partition.rs`, `SHARED_CONSTANT_BYTES` =
   4 KB): em vez de copiada para cada parte, vai para a parte 0 como `@<nome>.program`
   `hidden` e as outras a declaram. O ThinLTO só a importa como `available_externally` (o
   endereço escapa para o runtime), então fica uma cópia no executável. Teste:
   `partition::tests::large_local_constant_is_defined_once`.
3. **`"nooutline"` no `df.register.program`** (`llvm/selectors.rs`).
4. **ICF `safe_thunks`** na ligação de produção do programa em partes no macOS
   (`linker_macos.rs`, `icf_mode`), com `-mllvm -addrsig` no LTO: o código de máquina idêntico
   é dobrado, e uma função cujo endereço é tomado (um *tear-off*) vira um *thunk* em vez de
   dividir o endereço. O `corpus/native` inteiro passou com ele ligado em todo programa
   (`corpus/native/80_tearoffs_de_corpo_identico` igual à VM), mas no programa pequeno o ganho
   é 0,5% (16 KB de 3,3 MB) e o executável já é menor que o do Dart: fica só no programa em
   partes, como o *outliner*. `DARTFORGE_ICF` escolhe (`none`, `safe`, `safe_thunks`, `all`).
   **Revertido na §8:** no programa em partes ele junta *tear-offs* (os objetos do ThinLTO saem
   sem `__llvm_addrsig`); hoje fica desligado por padrão.
5. **Buscas lineares que eram quadráticas**:
   * mapa e conjunto constantes (avaliação e verificação de chave/elemento repetido,
     `const_eval/evaluation.rs` e `verifier.rs`): índice por valor primitivo (`KeyIndex`);
   * o bloco atual do `FnBuilder` (`lower/fn_builder.rs`): o id é o índice.
6. **Tarefas de geração** (`driver::default_generation_tasks`): uma por núcleo físico
   (metade das *threads* com SMT), uma por GiB de memória além de 4 GiB de reserva, até 8; sem
   o tamanho da memória, 3 como antes. Nesta máquina: 3 → 8. A CPU total não muda; o relógio
   da ligação e das partes cai com a máquina livre.

### 7.3 Resultado

`new_sali/backend`, `aot --optimize`, SDK em cache, objetos frios (CPU = usuário + sistema;
entre parênteses, a carga média da máquina no início):

| | front-end | HIR | LLVM IR | objetos | ligação | **CPU total** | relógio | executável |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| partida | 12,6 s | 86 s | 19,9 s | 358 s | 935 s | **~1 410 s** | 2 620 s (92) | 43,52 MB |
| itens 1 (sem ints curtos), 2, 5, 6 | 4,8 s | 48,5 s | 13,5 s | 99 s | 506 s | **682 s** | 557 s (31) | 38,93 MB |
| + ICF `safe_thunks` (religação das mesmas entradas) | | | | | 471 s | | | 37,92 MB |
| todos os itens, manifesto do SDK ainda com o hash antigo | 5,0 s | 67,5 s¹ | 17,8 s | 99,8 s | 364 s | **566 s** | 551 s (56) | 36,90 MB |
| **final** (manifesto revisto: as provas de `double`/`clamp`/multiplicação voltam) | 4,8 s | 80,3 s¹ | 17,8 s | 100,9 s | 361 s | **575 s** | 589 s (73) | **36,98 MB** |
| `dart compile exe` (mediana de 7) | | | | | | **41,3 s** | 78 s | 26,46 MB |

¹ A HIR mediu 48,5 s de CPU na segunda linha com quase o mesmo código; a diferença é a máquina
(carga 56–157 durante as últimas medidas, contra 31: com a máquina cheia as *threads* vão para os
núcleos de eficiência).

* CPU: **2,45× menos** (1 410 → 575 s); ainda **13,9×** o `dart compile exe`.
* Relógio: 2 620 → 589 s com carga parecida; ainda ~7,5× o do Dart (78 s).
* Executável: **43,52 → 36,98 MB (−15,0%)**; ainda **1,40×** o do Dart.
* Religação sem mudança (objetos e ThinLTO em cache): 91 s de CPU (233 s de relógio, carga 129).
* O módulo mais lento do ThinLTO caiu de 905 s (um `unormdata`) para 91 s de tarefa.

Do mapa da ligação (`DARTFORGE_LINK_MAP=1`):

| seção | partida | final |
|---|---:|---:|
| `__TEXT,__text` | 34,42 MB | 29,86 MB |
| `__TEXT,__const` | 4,69 MB | 2,68 MB |
| `__DATA_CONST,__const` | 1,75 MB | 1,75 MB |
| `__DATA_CONST,__dfimg` (textos estáticos) | 1,89 MB | 1,89 MB |

No `__text` final: corpos 14,7 MB, `$async` 4,4 MB, adaptadores `$c` 2,8 MB, runtime e SDK
nativo 2,6 MB, funções extraídas pelo *outliner* 1,4 MB (eram 2,5), constantes `dfc.*` 1,2 MB.

`dart_ui/examples/file_explorer` (38 mil funções HIR depois da poda: abaixo do limiar de
programa grande, então campos e ajudantes em linha e sem `optsize`):

| | CPU | relógio | executável |
|---|---:|---:|---:|
| partida | 334 s | 531 s (60) | 20,69 MB |
| final | 303 s | 311 s (76) | 19,88 MB |
| final como programa grande (`DARTFORGE_LARGE_PROGRAM_FUNCTIONS=0`, medida) | 214 s | 242 s (62) | 15,66 MB |
| `dart compile exe` | 16,7 s | 46 s | 12,26 MB |

No File Explorer o ganho de tamanho é o do ICF (−3,9%); os literais aninhados e as cópias da
área pesavam pouco ali. Tratado como programa grande (campos e ajudantes por chamada, `optsize`),
ele cairia mais 21% (15,66 MB) e 29% de CPU, com o preço de desempenho da §5.1 nas chamadas
virtuais; o limiar (50 mil funções HIR) continua o mesmo até o dono decidir.

Desenvolvimento (`aot` sem `--optimize`, SDK em cache, objetos frios) do backend:

| | front-end | HIR | LLVM IR | objetos (`-O0`) | **CPU total** | relógio (carga) |
|---|---:|---:|---:|---:|---:|---:|
| partida | 12,0 s | 74,4 s | 17,8 s | 127,7 s | **235 s** | 344 s (40) |
| final | 4,8 s | 54,5 s | 13,5 s | 101,2 s | **177 s** | 303 s (67) |

### 7.4 O que resta

O tempo ainda é do LLVM: 462 dos 575 s de CPU são o Clang das partes e o ThinLTO, para 533 MB
de IR. No ThinLTO, a seleção de instruções e a alocação de registradores são metade do tempo das
funções; o resto se espalha pelo código, proporcional ao volume de IR. As alavancas que faltam
são do mesmo tamanho para tempo e para executável:

1. **Forma do código** (§1.4 e §6.6): a moldura da pilha-sombra, a checagem da exceção pendente
   depois de cada chamada, os adaptadores `$c`/`$g`/`$tc` (4 MB) e o `$async` (4,4 MB).
2. **A otimização dobrada**: a pré-ligação `-O2` das partes e o ThinLTO `-O1` otimizam as mesmas
   funções duas vezes (medido no Windows, §6.2: sem ganho trocar só um dos níveis).
3. **Análises de proveniência repetidas** na HIR (`receiver_provenance::analyze` roda sobre o
   módulo inteiro na poda, na desvirtualização física e na emissão; ~12% da CPU da HIR).
4. **`df.prepare_isolate` e os outros inicializadores longos** podem ter o mesmo problema do
   *outliner* que o `df.register.program` tinha; não medido.

Achado no caminho: a inferência do front-end dá `Map<int, List<Object?>>` a um literal de mapa
aninhado num mapa cujo outro valor é `{}`, e a VM dá `Map<dynamic, dynamic>` (o LUB do contexto
desce para os elementos). É anterior a esta rodada e não depende do caminho por tabela; o
`corpus/native/175` declara o tipo para não depender disso.

## 8. Quarta rodada: a otimização dobrada, o teste `is` fora de linha e o ICF revertido (01/11/2026)

Mesma máquina e mesmo método da §7 (CPU primeiro; carga 37–70 nestas medidas). Base: o `main` de
01/11 com a §7 integrada. Todas as linhas abaixo são do mesmo binário, mudando só as chaves.

### 8.1 O que foi medido

Dos 575 s de CPU da §7, 462 eram LLVM. No perfil da ligação (`--time-trace`), a geração de
código era 68% do tempo das tarefas do ThinLTO (seleção de instruções 183 s de tarefa, alocação de
registradores 90 s) e a otimização 27%; a pré-ligação `-O2` das partes, 101 s de CPU no Clang,
otimizava as mesmas funções que o ThinLTO `-O1` otimiza de novo.

| pré-ligação das partes | objetos | ligação | CPU total | executável |
|---|---:|---:|---:|---:|
| `-O2` (antes) | 100,9 s | 361 s | 575 s | 36,98 MB |
| `-O1` | 67,8 s | 355 s | 571 s | 36,95 MB |
| `-O0` | 23,9 s | 368 s | 474 s | 37,09 MB |

(as três com o binário da §7, objetos frios; a HIR varia com a carga)

O ThinLTO quase não muda com o nível da pré-ligação. `-O0` poupa mais, mas o executável cresce
0,1 MB; `-O1` poupa 33 s e não muda o tamanho.

### 8.2 O que mudou

1. **Pré-ligação `-O1` nas partes** (`generator::parts_prelink`; `DARTFORGE_PRELINK_O`
   escolhe, `0` fica como chave de medida). O programa pequeno (um módulo, LTO com o SDK) segue
   com `-O2`.
2. **O teste `is` com cache do local de uso fora de linha no programa grande**
   (`@df.rti_test`, `llvm/mod.rs`; `rti_test` em `OUTLINED_HELPERS_LARGE_PROGRAM`): ~40
   instruções de IR em cada um dos ~16 mil locais (4% das linhas do IR) viram uma chamada para um
   ajudante `noinline` que é folha do coletor, como o `@df.rti_is_probe` que ele chama. O
   programa pequeno continua com o teste em linha. `DARTFORGE_OUTLINED_HELPERS` escolhe.
3. **`"nooutline"` nos inicializadores longos de topo** do programa grande (membro de biblioteca
   sem parâmetros, com 2 000 instruções ou mais): as tabelas de código do `saslprep`, montadas
   por centenas de elementos `for`, custavam ~17 s de CPU ao *outliner* para 4 KB.
4. **ICF desligado** (`DARTFORGE_ICF` liga para medir). Passando o `corpus/native` pelo
   pipeline do programa grande (`DARTFORGE_PART_KB=64`, que divide até um programa pequeno em
   partes de 64 KB, e `DARTFORGE_LARGE_PROGRAM_FUNCTIONS=0`), o
   `80_tearoffs_de_corpo_identico` diverge da VM com o `--icf=safe_thunks` da §7: dois
   *tear-offs* de corpo igual com o mesmo endereço (`conjunto: 9` em vez de 10). Os objetos que o
   ThinLTO do `ld64.lld` 22.1.8 gera para as partes não têm `__llvm_addrsig` (conferido com
   `otool` nos objetos retidos por `-object_path_lto`): o `-mllvm -addrsig` não chega às tarefas
   do ThinLTO, e o `safe` vira `all`. Sem ICF o `80` passa. Custa o 1,0 MB que a §7 tinha
   ganhado.

### 8.3 Resultado

`new_sali/backend`, `aot --optimize`, SDK em cache, objetos frios, mesmo binário:

| | objetos | ligação | **CPU total** | relógio (carga) | IR | executável |
|---|---:|---:|---:|---:|---:|---:|
| itens 1 e 2 desligados (`-O2`, `is` em linha), ICF ligado | 92,9 s | 324 s | 495 s | 261 s (71) | 534 MB | 37,26 MB |
| itens 1 e 2 ligados, ICF ligado | 65,7 s | 316 s | 457 s | 238 s (51) | 512 MB | 36,45 MB |
| **padrão novo** (itens 1–4) | 65,2 s | 327 s | **469 s** | 260 s (59) | 512 MB | **37,42 MB** |
| `dart compile exe` (mediana destas medidas) | | | **38,8 s** | 78 s | | 26,46 MB |

* Os itens 1 e 2: −38 s de CPU e −0,81 MB; tirar o ICF devolve +0,97 MB.
* Contra a §7 (575 s, 36,98 MB com o ICF que estava errado): **−106 s de CPU (−18%)**, +0,44 MB.
  Parte da queda de CPU vem do `main` de 01/11 (as linhas medidas no mesmo binário isolam os itens
  desta rodada).
* Contra o `dart compile exe`: **12,1× a CPU** (era 13,9×), relógio ~3,3× com a máquina menos
  carregada, executável **1,41×**.

### 8.4 Desempenho

Nenhum dos itens muda o código do servidor HTTP nem dos programas de `bench/performance`: são
programas pequenos (um módulo), e os itens 1–3 só valem no programa em partes ou no programa
grande. Como substituto, os programas de `bench/performance` foram compilados pelo pipeline das
partes (`DARTFORGE_PART_KB=64`) com a pré-ligação `-O2` contra `-O0`: os executáveis saíram com o
mesmo tamanho em cada programa (bytes diferentes; o ThinLTO refaz a otimização), e a medida alternada
oscilou de 0,84× a 1,87× entre núcleos com código praticamente igual — com a máquina carregada
o A/B não diz nada. Por isso a pré-ligação `-O0` ficou só como chave e o padrão é `-O1`, que
também não mudou o tamanho do backend.

### 8.5 Validação

* `corpus/native` pelo pipeline do programa grande (partes de 64 KB, campos e ajudantes por
  chamada, `optsize`, *outliner*, `@df.rti_test`): 164/164 em `--optimize` e em
  `--optimize --gc-stress` sem ICF (com o ICF da §7, 163/164: o `80`).
* `corpus/native` no padrão: 164/164 em desenvolvimento, `--optimize` (com
  `DARTFORGE_CHECK_SHADOW=1`), `--optimize --gc-stress` e `--wasm` (`--exec-limit 60`).
* `cargo test --release -p dartforge-emit-native --features embedded-llvm --
  --include-ignored`: 1083 de 1084 na biblioteca, 29 de integração e 128 *doctests*. A falha,
  `allocation_inventory::imports::tests::actual_selected_sdk_inventory_loads_without_authorizing_opaque_startup`,
  é o inventário do SDK certificando também o registro de `dart:async`; nenhum item desta rodada
  muda o IR do SDK (todos valem só para o programa), e o teste passava antes do `main` de 01/11
  (que trouxe `951ffb98` e `7f35a02f`, que mexem no SDK e no inventário). Clippy `-D warnings`
  limpo no `dartforge-emit-native`.

### 8.6 O que resta

* **A geração de código** é agora a maior parte: ~245 s de CPU no ThinLTO para 512 MB de IR.
  Só cai com menos código: a moldura da pilha-sombra (as gravações de raiz, 8% das instruções do
  IR), a checagem da exceção depois de cada chamada, o mapa de referências na gravação de campo
  (`%fm*`, 8%), os adaptadores `$c`/`$g`/`$tc` e o `$async`. São escolhas do modelo de execução
  (raízes na pilha-sombra, exceção por bandeira), não do *driver*.
* **ICF de verdade** precisa da tabela de endereços significativos nos objetos do ThinLTO:
  ou um `ld64.lld` que a escreva nas tarefas do ThinLTO, ou marcar as funções cujo endereço é
  comparado e dobrar o resto no próprio IR (o `MergeFunctions` já faz parte disso, com *thunks*).
* **`receiver_provenance::analyze`** roda sobre o módulo inteiro três vezes (poda, desvirtualização
  física e emissão), ~12% da CPU da HIR (~6 s); o módulo muda entre as três, e reaproveitar o
  resultado exige provar que a mudança não o afeta. Não mexi.
