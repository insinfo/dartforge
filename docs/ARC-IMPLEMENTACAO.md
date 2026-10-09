# ARC no backend nativo — estado da implementação

Este documento registra o que está implementado da
[especificação do ARC com coleta de ciclos](ARC-CICLOS-ESPECIFICACAO.md), as
decisões desta etapa e o que ainda falta. A especificação continua sendo o
alvo; aqui fica só o que o código faz hoje.

## 1. Seleção

`--memoria tracing|arc` (ou `--memoria=`) em `aot` e `compile-native`. A
opção define `DARTFORGE_MEMORIA`, lida uma vez por `alvo::memoria_arc` e
transportada em `Module::memoria_arc` (programa e SDK). O padrão continua
`tracing`, e o IR dele não muda.

| Combinação | Resultado |
| --- | --- |
| `tracing` (padrão) | o coletor por rastreamento de sempre |
| `arc` no AOT e no JIT | contagem de referências com coleta de ciclos; a migração de instâncias da recarga conta pela foto (`Heap::trocar_campos`) |

O SDK compilado leva `memoria=arc_v1` na chave (`sdk_modulo::chave_do_sdk`).
O programa ARC chama `dartforge_memoria_arc_v1` na entrada (no
`df.preparar_isolado` de cada isolado, ou no `dartforge_entry`), e esse
símbolo versionado faz a ligação com um runtime sem a ABI falhar (§24).

## 2. Primeiro desenho: RC adiado das raízes, contado nos maduros

O runtime é um só para os dois modos. O ARC liga por isolado, em tempo de
execução. Esta seção registra o primeiro desenho, ainda selecionável com
`DARTFORGE_ARC_BERCARIO=1`. O padrão atual é o ARC puro da §6.

| Parte | Como funciona hoje | Arquivo |
| --- | --- | --- |
| Metadados | Tabela lateral por handle (`MetaArc`: geração, RC, estado, candidato, proteção, imortal). O cabeçalho e o layout não mudam (§18.1, decisão 4). | `crates/runtime/src/arc.rs` |
| Ocorrências | Contadas uma a uma, com multiplicidade (§18.1, decisão 2), entre objetos **velhos**. | `arc.rs`, `heap.rs` |
| Jovens | Continuam com a coleta menor de sempre. O jovem que sobrevive é **promovido** e ganha metadados (`rc=0`). As arestas que saem dele e as que os velhos lembrados têm para ele são contadas nesse momento. O jovem que morre na menor nunca toca a tabela. | `heap.rs` (`sincronizar_arc`), `espaco.rs` (`jovens_marcados`) |
| Gravação contada | `definir_campo`, `gravar_ref`, `gravar_refs` e os natives que gravam cru (`nativos_hash.rs`, `nativos_listas.rs`) contam a troca na hora: retain do novo, release do antigo. Só quando o dono é velho; o jovem não tem metadados. | `heap.rs` (`arc_trocou`), `nucleo.rs` (`arc_gravacao_crua`) |
| Gravação crua | `palavras_mut`, `campos_de_objeto` e `definir_flags` num velho tiram uma **foto** das ocorrências contadas. Na sincronização, as atuais são retidas e as da foto soltas. Até lá, as gravações contadas pulam o objeto fotografado. | `heap.rs` (`arc_foto`) |
| Código gerado | No ARC, `SetField`, o `dartforge_object_set` especializado, `CellSet` e os ajudantes de lista (`@df.lista_gravar_ref`, `@df.lista_gravar_dados`) não gravam em linha: chamam o runtime, que conta e aplica a barreira (§18.1, decisão 6). A alocação continua em linha (o objeto nasce jovem). | `llvm/mod.rs`, `llvm/listas_ir.rs`, `llvm/caixas_ir.rs` |
| Raízes | **Adiadas**: a pilha-sombra (ou os mapas), os quadros do runtime, os globais, os literais e as demais raízes de `Heap::raizes` não contam. Elas protegem os zeros que veem na sincronização: o zero protegido fica na fila (a tabela de contagem zero). | `heap.rs`, `arc.rs` (`drenar_zeros`) |
| Morte | Só na sincronização (fim da coleta menor), nunca dentro de uma gravação: o runtime mantém o contrato de raízes do rastreamento. O descarte rompe as arestas e solta os destinos fora do conjunto (§19.4). | `arc.rs` (`descartar`) |
| Ciclos | Na completa, o ponto fixo global (§22.3): `L` é o alcance pelas raízes reais com a regra condicional dos efêmeros, e todo registrado vivo fora de `L` morre num lote só, inclusive o de RC positivo só por ciclos. A *trial deletion* lateral sem tocar o RC real (§22.1) fica para `DARTFORGE_ARC_CICLOS=sempre`, em toda sincronização e repetida a cada volta com morte nova. Candidatos: quem desceu a um RC positivo, quem desceu a zero e voltou a subir antes da drenagem (o autociclo `x[i] = x` depois de soltar a última ocorrência de fora) e quem uma raiz vê. As raízes não geram decremento ao sumir, então o sobrevivente protegido só por elas volta a ser candidato. | `heap.rs` (`ponto_fixo_arc`), `arc.rs` (`coletar_ciclos`, `candidatar`, `drenar_zeros`) |
| Efêmeros | Entre as completas, cada entrada contada é uma ocorrência do valor atribuída à chave no grafo: segura o valor enquanto a chave vive, mesmo com o portador já inalcançável (retenção, nunca morte antes da hora). A morte da chave rompe a entrada; a do portador a solta. No ponto fixo o portador conta: a entrada de portador fora de `L` sai da contagem antes do lote (sem isso, o ciclo valor→portador ficava vivo pela chave). | `heap.rs` (`EfemerosDoArc`, `ponto_fixo_arc`) |
| Fracas e finalizadores | As tabelas laterais esquecem os mortos na sincronização (fracas zeradas, anexos e finalizáveis disparados), em laço até não haver morte nova. | `heap.rs` (`sincronizar_arc`) |
| Reclamação | Na completa: as marcas passam a ser os vivos do RC (não a marcação por rastreamento) e a varredura completa devolve os blocos dos mortos (§19.4). Até lá o morto fica com as arestas zeradas e sem metadados. | `heap.rs` (`reclamar_arc`) |
| Ativação | Uma coleta completa por rastreamento e o registro de todo vivo, com o RC das ocorrências que chegam a ele. | `heap.rs` (`ativar_arc`) |

### 2.1 Diferenças em relação à especificação

A especificação (§20, §21.3) pede owners na HIR: cada `Ref` SSA é um token
contado, com `ArcCopy`/`ArcDrop` inseridos pelo CFG e verificados. Esta
etapa conta as arestas do heap e deixa as raízes **adiadas** (o RC adiado de
Deutsch e Bobrow, com a coleta de ciclos de Bacon e Rajan e o berçário
rastreado do *Ulterior Reference Counting*). Consequências:

* a morte acontece na sincronização (a cada coleta menor), não no último
  `release`;
* a pilha-sombra continua obrigatória no ARC: é ela que protege os zeros;
* não há verificador de ownership na HIR, porque não há tokens.

Os owners na HIR (§20) entram por cima disto. As arestas do heap continuam
contadas como estão; as raízes passam de adiadas a contadas, e a proteção dos
zeros pelas raízes vira só auditoria.

## 3. Diagnóstico

| Variável | Efeito |
| --- | --- |
| `DARTFORGE_ARC_CONFERIR=1` | depois de cada sincronização, nenhum objeto alcançável das raízes sem RC vivo (com o pai, a posição e os metadados no pânico) e o RC de cada vivo igual às ocorrências recontadas (§20.3, modo auditor) |
| `DARTFORGE_ARC_CICLOS=sempre` ou `nunca` | a *trial deletion* em toda sincronização (além do ponto fixo da completa), ou nem ela nem o ponto fixo |
| `DARTFORGE_ARC_TRACO=1` | uma linha por evento: sincronização (promovidos, lembrados, fotos), registro, retain, release, morte |
| `DARTFORGE_GC_RASTRO=1` | a linha `[arc]` de cada sincronização: promovidos, contados, retains, releases, mortos pelo RC e por ciclo, rodadas, examinados, candidatos |

Uma violação do contador (release com RC zero, retain de morto, trial
negativo, auditoria divergente) é pânico `bug do ARC`, nunca erro Dart
(§19.3).

## 4. Testes

* `cargo test -p dartforge-runtime --lib arc`: o núcleo com um grafo de
  mentira (cascata, multiplicidade, ciclo isolado e com referência externa,
  ciclo que segura acíclico, construção e proteção, geração obsoleta,
  auditoria, raiz observacional) e o ARC sobre o heap real (promoção e
  release, ciclo com e sem raiz, foto de escrita crua).
* Grafos aleatórios com semente contra um oráculo independente (§26.2,
  `arc_grafos_aleatorios_*` em `heap.rs`): alocação, gravação contada e
  crua, raízes que entram e saem, fracas, efêmeros, menores e completas, com
  a auditoria do ARC em toda sincronização. O modelo calcula o alcance forte
  com o ponto fixo dos efêmeros sem olhar o heap; depois de cada coleta, o
  alcançável continua registrado e com os elementos do modelo; depois da
  completa, nenhum inalcançável continua vivo, e a fraca e o efêmero de alvo
  ou chave inalcançável estão zerados. Três modos: o padrão, a *trial
  deletion* em toda sincronização e o rastreamento (que valida o modelo).
  `DF_ARC_SEMENTES=N` amplia a busca; 3000 sementes por modo passaram. A
  primeira rodada achou dois furos, corrigidos: o zero que volta a subir não
  virava candidato (o autociclo vazava), e o ciclo valor→portador de um
  efêmero ficava vivo pela chave.
* O corpus nativo com `DARTFORGE_MEMORIA=arc` pelo `dartforge-diferencial`
  (2026-10-08, Windows x86-64, 238 programas):

  | Rodada | Resultado |
  | --- | --- |
  | `--nativo`, `DARTFORGE_ARC_CONFERIR=1` | 238/238 |
  | `--nativo`, `DARTFORGE_ARC_CICLOS=sempre`, `DARTFORGE_ARC_CONFERIR=1` | 238/238 |
  | `--nativo --gc-stress` | 238/238 |
  | `--jit-aot`, `DARTFORGE_ARC_CONFERIR=1` | 238/238, JIT × AOT sem divergência |
  | Depois do ponto fixo (2026-10-08): `--nativo` com auditoria, `--gc-stress`, `DARTFORGE_ARC_CICLOS=sempre` com auditoria | 238/238 nos três |

  O job `nativo-arc` do `pesado.yml` roda o corpus com a auditoria e com o
  `--gc-stress`.

## 6. ARC puro (2026-10-08): sem rastreamento do berçário

O desenho da §2 rastreava o berçário (a coleta menor achava os jovens vivos e
os promovia; as arestas do jovem entravam na promoção). Era o *Ulterior
Reference Counting*: RC nos maduros, rastreamento nos jovens. Com o ARC puro,
o padrão do modo `arc`, ninguém percorre o heap para decidir vida:

| Parte | Como funciona | Arquivo |
| --- | --- | --- |
| Gravação | Toda gravação de referência conta na hora, também num jovem (`arc_trocou`); o destino sem metadados ganha registro (RC zero) antes da primeira ocorrência. | `heap.rs` |
| Alocação em linha | O código gerado grava os campos de `AllocObject`, do contexto, da célula, do record pequeno e da closure sem passar pelo runtime: depois de cada uma, `dartforge_arc_inicial(h)` conta as referências gravadas (§21.1, item 1). | `llvm/mod.rs`, `llvm/caixas_ir.rs`, `nucleo.rs` |
| Escrita crua | A foto vale para todo objeto: a drenagem conta as ocorrências de agora e solta as da foto. A vida do fotografado segue o RC. | `heap.rs` (`arc_foto`) |
| Drenagem | No lugar da coleta menor (`drenar_arc`): as raízes diretas (pilha-sombra ou mapas, quadros do runtime, globais) dão o conjunto protegido, sem seguir arestas. O jovem registrado fica; o que só uma raiz vê ganha registro (RC zero, protegido); o resto morreu sem ninguém o ver e solta as ocorrências que contou. Os zeros morrem em cascata, menos os protegidos. | `heap.rs` |
| Ciclos | Na completa, o ponto fixo da §22.3 quando há efêmeros; senão a *trial deletion* até não haver morte nova. Candidatos: quem desceu a RC positivo, o zero que voltou a subir, o jovem que sobreviveu à drenagem e quem uma raiz vê. | `heap.rs`, `arc.rs` |
| Memória | O jovem morto sai pela varredura dos jovens (sem marca); o morto pelo RC volta à faixa livre da classe um a um (`EspacoDeObjetos::soltar_morto`, com o bit de marca apagado), sem varrer páginas. | `espaco.rs` |

`DARTFORGE_ARC_BERCARIO=1` volta ao desenho da §2 (para comparar).

Validação (Windows x86-64): os grafos aleatórios da §4 nos três modos (3000
sementes cada); o corpus nativo com a auditoria (238/238), com
`--gc-stress` (238/238), com `DARTFORGE_ARC_CICLOS=sempre` e a auditoria
(238/238) e no JIT × AOT (238/238 idênticos); `dart:io` 127/130 (as duas
do ambiente desta máquina, `02` e `03`, e o `91`, que estoura os 5 s com
quatro tarefas em paralelo). O `--gc-stress` achou o caminho que faltava: o
record pequeno alocado em linha não contava os campos.

**Custo atual.** O `corpus/nativo/91` (strings grandes) leva 3,2 s no ARC puro
contra 1,1 s no rastreamento: 3,1 milhões de objetos mortos pelo RC, cada um
passando pela tabela de metadados (um `HashMap` por handle) várias vezes, e
1,1 milhão de candidatos a ciclo (quase todos strings). As otimizações, em
ordem de ganho esperado:

1. os metadados num índice por página e bloco (§19.1), sem hash;
2. o filtro de tipos acíclicos (§32): string, caixa e lista de escalares nunca
   são candidatas;
3. `retain`/`release` em linha no código gerado, sobre o índice;
4. os donos na HIR (§20), que tiram contagens redundantes e a olhada nas
   raízes da drenagem.

**Otimizações feitas (2026-10-08, à noite)**, escolhidas pelo tempo por fase da
drenagem que o rastro passou a mostrar (`DARTFORGE_GC_RASTRO=1`: `fases=`
raízes/fotos/jovens das raízes/efêmeros/decisão dos jovens/soltura dos jovens
mortos/raízes candidatas/zeros e ciclos/memória, e `laco=` zeros/ciclos/ponto
fixo/tabelas, em µs; `zeros_vistos=` e `adiados=` contam a fila de zeros):

* **Os adiados uma vez por coleta.** O objeto de RC zero que uma raiz segura
  voltava à fila de zeros a cada passada, e entrava de novo a cada vez que o
  RC voltava a zero: no `json`, 557 milhões de entradas examinadas para 19,7
  milhões de mortes. Agora fica numa lista própria, uma entrada por objeto
  (`MetaArc::adiado`), que volta à fila no começo de cada coleta
  (`retomar_adiados`): 30 milhões de entradas, e as drenagens do `json` de
  17,6 s para 5,8 s.
* **Os metadados por página** (§19.1): a tabela acha a página pelo número dela
  num mapa radix e o metadado pelo índice do bloco, com a geometria que o heap
  informa no registro (`Geometria`, `registrar_vivo_em`); o `HashMap` fica só
  para os handles sem página (os testes, a imagem).
* **O descarte de um objeto só** (a cascata) sem montar conjunto nem vetor
  novos; **as tabelas laterais** perdem os mortos por um predicado (o objeto
  do espaço sem metadados morreu nesta drenagem) em vez de um conjunto com os
  milhões de mortos da rodada; **os efêmeros** não custam hash por morte quando
  não há nenhum contado; **o objeto `BRUTO`** não vira candidato a ciclo.

Medida (`bench/desempenho` em produção, preso a um núcleo P, ARC/A0): **2,81 →
2,13** de média geométrica. `json` 4,1–8,4× → 2,2–3,7×; `lista_objetos` 6,5× →
3,8×; `lista_ligada` 19,9× → 8,8×; `arvores` 17× → 15×; numérico, chamadas e
listas 0,8–1,1×. Corpus 238/238 com auditoria e `--gc-stress`, com ciclos em
toda drenagem e no JIT; os grafos aleatórios (3000 sementes × 3 modos)
seguem verdes.

O que ainda pesa, pela mesma medida (`objetos_escapam`): a decisão dos jovens
(~1 s em 377 drenagens, ~40 ns por jovem: consulta, marca, revisão e
candidatura de cada um), a cascata dos zeros (~130 ns por morte: dois
percursos do corpo, um `soltar` por aresta) e a soltura física um a um.

**Registro dos jovens pelas raízes (2026-10-09).** Na drenagem pura, o passo
que registra os jovens protegidos diretamente percorre o conjunto das raízes,
confere o estado `JOVEM` e só registra quem ainda não tem metadados. Antes
esse passo consultava os metadados de todos os jovens entregues, para depois
verificar se estavam nas raízes. A decisão de vida de todos os jovens continua
no passo seguinte; objetos velhos e estáticos não são registrados novamente.
O teste dirigido confere jovem protegido de RC zero, jovem inalcançável e
preservação da geração do velho. Os grafos aleatórios passaram com 3.000
sementes em cada um dos três modos (ARC, ciclos sempre e rastreamento).

Medida dirigida (`objetos_escapam`, produção ARC, cinco execuções alternadas
antes/depois, afinidade `0x4`; mediana das cinco rodadas após o aquecimento em
cada execução, depois mediana das execuções):

| núcleo | antes (ms) | depois (ms) | depois/antes |
| --- | ---: | ---: | ---: |
| `arvores` | 888,131 | 865,112 | 0,974 |
| `lista_ligada` | 179,882 | 172,090 | 0,957 |

Os resultados foram idênticos em todas as execuções (`3156655` e
`499999500000`). Os executáveis têm SHA-256 começando em `2c9c3458e97d`
(antes) e `219f7ac7a2e9` (depois). Uma execução adicional com rastro em cada
versão, fora da medição de vazão, teve as mesmas 382 drenagens e 24.928.308
jovens: a fase de jovens das raízes (inclui montar o vetor dos jovens) caiu
de 535.255 para 207.153 µs acumulados. A suíte completa do runtime passou
108 testes, com três microbenchmarks ignorados. Esta medida dirigida não
atualiza a média geométrica do benchmark inteiro nem prova o corpus nativo.

**Índice de metadados sem divisão (2026-10-09).** Cada página prepara o
inverso da parte ímpar do tamanho do bloco, módulo 2⁶⁴, e o número de bits
da potência de dois retirada. A consulta multiplica o deslocamento pelo
inverso e roda esses bits para a direita: só um início exato de bloco
produz índice menor que `blocos`. A extensão da geometria precisa caber em
u64, para que um índice aceito não represente um produto que transbordou.
Na reutilização de página para outra classe, os fatores são recalculados.

Verificação: todos os bytes de uma página em cada uma das 88 classes de
tamanho contra um oráculo pela divisão original, tamanhos grandes até o
limite de u64 e troca de geometria da página. A suíte do runtime passou
112 testes (inclui o exemplo de API), com três microbenchmarks ignorados;
os grafos aleatórios passaram 3.000 sementes em cada um dos três modos.
Medição isolada do módulo real (`bench/arc/metadados.rs`, `rustc -O`, oito
páginas de 1.024 blocos, dez milhões de pares retain/release e consultas por
rodada, sem alocar nem coletar, RC permanece um): cinco execuções alternadas
no núcleo P, mediana das rodadas após a primeira e depois das execuções.
115,678 → 100,900 ms; repetição independente: 115,591 → 100,553 ms (−13%).
Houve rodadas com forte variação de tempo em ambas as versões; todos
os dados entraram nas medianas. Isso mede o acesso aos metadados, não o ARC
inteiro.

Na medida dirigida de produção de `objetos_escapam` (mesmo protocolo, sem
rastro): árvores 698,022 → 683,622 ms; lista ligada 141,285 → 139,065 ms.
O ganho de cerca de 2% é pequeno diante da dispersão; não prova ganho na
média completa. Resultados iguais em todas as execuções; SHA-256 dos
executáveis começando em `0b8b24aa282f` e `45f607ffcd23`. A execução com
rastro manteve 382 drenagens e 24.928.308 jovens em ambas as versões.

### 6.1 Comparação completa auditável (2026-10-09)

O benchmark inteiro, depois das mudanças de raízes e índice, teve ARC/A0
**2,107**, com árvores **14,65×** e lista ligada **7,63×**. São 32 núcleos,
sete execuções alternadas no núcleo P, produção/LTO, Dart 3.6.2 AOT como
referência. A1/A0 0,983, B0/A0 0,959, B1/A0 0,950 e A0/Dart 1,260.
Todos os resultados iguais em todas as 378 execuções, sem processo falho
nem núcleo ausente. [Protocolo, dados brutos e hashes dos executáveis](../bench/resultados/2026-10-09-modos-windows/README.md).

A primeira rodada, com os mesmos executáveis mas sem registro das amostras
brutas, deu ARC/A0 2,120. Ambas ficam perto dos 2,13 anteriores: o ganho
isolado de 13% na consulta de metadados não prova ganho relevante no
benchmark inteiro. O objetivo de aproximar o ARC de A0 continua pendente.

### 6.2 Faixas vizinhas na lista livre (2026-10-09)

`EspacoDeObjetos::soltar_faixa` junta intervalos contíguos na ponta da lista
livre, inclusive quando a faixa nova liga duas já devolvidas. A classe e a
página precisam ser iguais; não percorre os demais livres. O reabastecimento
da TLAB pode então entregar vários blocos de um trecho, em vez de um bloco
por passagem pelo runtime. O caminho de quarentena continua devolvendo
apenas os blocos que saíram do anel.

Testes dirigidos de ordem crescente/decrescente/ponte, preservação de bloco
vivo e quarentena; suíte completa do runtime: 115 testes passaram, três
microbenchmarks ignorados. Grafos aleatórios: 3.000 sementes por modo.
Em duas medidas dirigidas independentes, ARC caiu 4–5% em árvores e
7,5–8,8% em lista ligada; A0 variou cerca de ±3% e −2,3% a 0% nos mesmos
núcleos. [Amostras, hashes e protocolo](../bench/resultados/2026-10-09-arc-faixas/README.md).
São dois núcleos do benchmark. Na rodada completa posterior, ARC/A0 foi
**2,108**, contra 2,107 da anterior: não há ganho demonstrado na média.
A1/A0 0,991, B0/A0 0,969, B1/A0 0,952, A0/Dart 1,270; todas as 378
execuções válidas e iguais. [Dados da rodada completa](../bench/resultados/2026-10-09-modos-faixas-windows/README.md).
A validação do corpus nativo desta mudança continua pendente no CI.

### 6.3 Quadros proprietários explícitos (2026-10-09)

`Heap::push_frame_proprietario` abre slots contados: `set_root` retém a
cópia antes de soltar a referência anterior; `root` conta cada ocorrência
adicionada e `pop_frame` solta todas. A ativação do ARC e a promoção dos
jovens reconstroem essas ocorrências com multiplicidade. A auditoria recebe
os slots proprietários; o percurso de alcance visita ambos os tipos.

Os quadros existentes continuam observacionais, inclusive os usados pelo
código gerado. A distinção é explícita por quadro, como exige §21.3; ainda
falta migrar os owners temporários de Rust, fornecer movimento de resultados
owned e integrar a HIR. Isso não conclui o contrato de ownership.

Três testes dirigidos cobrem cópias, substituição, mesma referência, Smi,
ativação tardia, promoção, raízes observacionais e ciclos com arestas fortes,
nos modos puro e berçário. Suíte release do runtime: 105 aprovados, zero
falhas, três microbenchmarks ignorados. Exemplo público aprovado em doctest.
O CI nativo `37908897460` antecede esta mudança; não a valida.

`Heap::mover_raiz` transfere uma ocorrência entre slots proprietários e
zera a origem, sem `retain`; o conteúdo substituído é soltado. Quando ambos
os slots continham o mesmo handle, uma das duas ocorrências é consumida.
Mover o slot para ele próprio é identidade. A validação das categorias e
dos índices precede qualquer alteração. A operação permite transferir um
resultado do quadro chamado ao chamador antes de fechar o primeiro; ainda
não adota um token owned externo aos slots nem altera a ABI do código gerado.
Teste de transferência, alias e substituição aprovado em debug/release;
rejeição de destino observacional aprovada em debug, preservando o owner.
Suíte debug completa: 107 aprovados, três microbenchmarks ignorados; exemplo
de movimento aprovado em doctest.

O auxiliar de construção de células, contextos, closures e records em
`caixas.rs` agora guarda cópias proprietárias dos argumentos. O quadro
temporário de `asFunction` em `ffi.rs` também conta o ambiente até a closure
guardar a aresta forte. Esses caminhos não executam Dart entre abrir e fechar
o quadro. Teste dirigido com coleta forçada verifica a passagem da proteção
para a aresta da closure, a contagem e a liberação nos modos puro e berçário.
Suítes completas debug/release: 108 aprovados, zero falhas, três microbenchmarks
ignorados. Os demais quadros Rust ainda exigem migração e revisão das saídas.

`lista_com_raizes` também usa slots proprietários. Seus chamadores publicam
os resultados sem safepoint entre o fechamento do quadro e a gravação da
aresta; o owner temporário não substitui essa publicação. Teste dirigido
força crescimento de lista compacta, descompactação de `i64::MAX` e caixa
double com coleta antes de cada alocação, nos modos puro e berçário. Confere
valores, ausência de quadros residuais, RC das caixas e morte da cadeia ao
soltar o owner externo. Suítes debug/release: 109 aprovados, três microbenchmarks
ignorados. A rodada remota `37915929494` antecede esta migração das listas.

Visões tipadas e `dart_lista_fixa` (resultados de I/O) também mantêm cópias
proprietárias durante a alocação. O quadro só fecha depois de gravar as arestas
da visão ou da lista, de modo que os argumentos continuem contados até a
publicação. Testes com coleta forçada cobrem visão de visão (a base interna
sobrevive à visão intermediária) e duas ocorrências do mesmo texto num
resultado de I/O, incluindo liberação ao sair o último owner. Suítes debug/release:
111 aprovados, três microbenchmarks ignorados. A rodada remota atual também
antecede estas migrações.

A montagem `dartforge_record_new` e a concatenação `List.+` também usam
quadros proprietários para argumentos e caixas temporárias. Os resultados
são devolvidos sem coleta depois de soltar o quadro. Testes sobre as funções
da ABI, com coleta forçada, cobrem aliases repetidos num record e concatenação
de formas int/double, valores encaixotados e liberação das entradas e da
cadeia de saída. Suítes debug/release: 113 aprovados, três microbenchmarks ignorados.
Os três testes ABI de owners (I/O, record e concatenação) também passaram
com `DARTFORGE_ARC_CONFERIR=1`, auditando RC nas coletas.
O corpus remoto atual ainda não contém estas migrações.

O quadro dos nós em `materializar` (mensagens entre isolados) agora conta
owners durante a alocação, ligação de arestas e reconstrução de índices.
O retorno não introduz safepoint depois de fechar o quadro. Teste dirigido
monta dois nós cíclicos com duas referências ao mesmo filho, coleta durante
a construção e verifica preservação e morte do ciclo depois da última raiz.
Passou com auditoria RC no modo puro; os quatro testes ABI de owners também
passaram com `DARTFORGE_ARC_CONFERIR=1` e `DARTFORGE_ARC_BERCARIO=1`.
Isso cobre os nós temporários; os owners da fila de mensagens e sua
transferência ao despacho continuam pendentes (§21.3/§23.2).
Suíte release completa: 114 aprovados, três microbenchmarks ignorados.

Globais e slots de exceção/rastro agora são owners persistentes. Os setters
retêm antes de soltar; `mover_raiz_global` transfere a ocorrência, soltando
o destino anterior (inclusive alias), e mover para o mesmo endereço não
altera RC. A ativação, promoção e auditoria incluem esses slots com
multiplicidade. Testes cobrem aliases, troca por outro objeto/Smi, remoção
repetida, movimento e ativação com slots abertos no tracing. Suítes
debug/release: 116 aprovados, três microbenchmarks ignorados. Literais,
tabelas canônicas, eventos e filas ainda exigem seus contratos de owners.
O código gerado ainda não fornece a ABI owned/borrowed completa.
Doctests do runtime: oito aprovados, incluindo os quatro métodos persistentes.

As tabelas de literais, tear-offs de topo e singletons enum contam um owner
por entrada. Repetir o registro do mesmo handle não retém novamente; trocar
uma entrada solta o owner anterior. A ativação, promoção e auditoria incluem
essas entradas com multiplicidade. A construção do enum também usa quadro
proprietário para o nome temporário. Teste dirigido cobre registros repetidos,
promoção, substituição e morte do tear-off que perdeu sua entrada, nos modos
puro e berçário. `permanentes` é índice de identidade, não raiz adicional:
o percurso de raízes e a purga desse índice permitem reclamar a entrada
substituída quando perde seus owners. Release: 117 aprovados, três
microbenchmarks ignorados. Doctests: 11 aprovados.
Filas, eventos e demais tabelas persistentes continuam pendentes.

Eventos únicos transferem agora o owner global da fila para um slot ativo
(`mover_global_para_slot`) antes de chamar Dart, sem `retain`. O slot é
soltado depois que a porta retorna, inclusive com exceção pendente. Timers
periódicos mantêm seu owner persistente e fazem uma cópia ativa para a chamada;
cancelar o timer dentro do callback não invalida o borrow ativo. Dois testes
com auditoria RC forçam coleta dentro do callback, incluindo cancelamento
reentrante, e verificam a liberação depois do retorno. A ABI owned/borrowed
do callback gerado e os owners das mensagens ainda estão pendentes.
Testes dirigidos aprovados nos modos puro e berçário com auditoria;
suíte release: 119 aprovados, três microbenchmarks ignorados. Doctest da
transferência global → slot aprovado.

Um terceiro teste do callback simula retorno com exceção pendente: coleta
durante a chamada, verifica que a pendência continua viva após soltar o slot
ativo e confirma morte depois de `dartforge_exception_clear`. Aprovado em
debug/release e nos modos puro/berçário com auditoria. Suíte debug completa:
120 aprovados, três microbenchmarks ignorados. O teste não executa um
desenrolamento LLVM por tabelas; essa integração continua pendente.

A regressão de `ValG::Mesmo` foi reproduzida: uma string enviada para uma
porta do mesmo isolado morria ao sair o owner do emissor, ainda na fila.
Mensagens normais agora retêm um token por ocorrência compartilhada, numa
tabela própria do heap destinatário. Alcance, ativação, promoção e auditoria
incluem essa tabela. O despacho solta os tokens depois de publicar o valor
materializado; porta fechada, despachante ausente e encerramento das portas
soltam as ocorrências descartadas. IDs crescem sem reutilização ou wrap.
Mensagens copiadas entre isolados não carregam esses tokens; handles `Mesmo`
precisam pertencer ao isolado atual e ao destinatário.

Testes sobre o envio real cobrem sobrevivência da string na fila, descarte e
dois aliases passados ao despacho com coleta dentro do callback. Aprovados
com auditoria nos modos puro e berçário. As mensagens de controle, o término
concorrente e a ABI completa de callbacks continuam exigindo validação própria.
Suíte release: 122 aprovados, três microbenchmarks ignorados.

O fechamento de todas as portas do isolado foi testado com duas mensagens
pendentes compartilhando uma string: a fila é drenada e ambas as ocorrências
são soltadas. Aprovado com auditoria nos modos puro e berçário. O contador
de raízes percorridas pelo tracing inclui a tabela de owners de mensagens;
teste de duas ocorrências, uma e nenhuma confere os números e a liberação.
Isso não cobre uma postagem concorrente à terminação do isolado.
Suíte release atual: 124 aprovados, três microbenchmarks ignorados.

O encerramento marca `Filas::encerrada` sob o mutex usado para publicar e
drena mensagens normais e de controle. Um remetente que obteve o `Arc` da
fila antes de sua retirada do registro não pode publicar depois dessa marca.
Na publicação normal, a admissão precede a leitura dos handles e a criação
dos tokens, sob o mesmo mutex. A retenção não coleta nem chama Dart. Teste com
barreira entre duas threads força publicação por clone antigo depois do
fechamento; aprovado em debug. Suíte release: 125 aprovados, três
microbenchmarks ignorados. A validação remota desta mudança ainda está pendente.

Uma fila já encerrada descarta o grafo sem consultar handles compartilhados,
que podem já ter morrido após o fechamento. Teste com handle comprovadamente
coletado aprovado em debug; a rejeição não tenta registrar ou reter esse handle.
Suíte release: 126 aprovados, três microbenchmarks ignorados. Os seis testes
de materialização passaram também com auditoria ARC e berçário habilitados.

O tratamento de controle usa um quadro proprietário para a mensagem
materializada até o retorno do handler. Os caminhos nativos de envio OOB
copiam com `compartilhar=false`: o grafo portátil não depende de handles
`Mesmo` enquanto aguarda. Teste posta uma string na fila real de controle,
coleta o original, materializa e coleta durante o tratamento, depois confere
a liberação ao retornar. Aprovado com auditoria nos modos puro e berçário.
Isso não valida ainda todos os comandos OOB nem o desenrolamento LLVM.
Suíte release: 127 testes unitários aprovados, três microbenchmarks ignorados;
13 testes de integração e 12 doctests aprovados.

## 5. Pendências

* Owners na HIR, inserção e verificador (§20), com as saídas excepcionais
  (§20.4).
* `ownership.tsv` por extern (§21.2).
* A proteção condicional entre as rodadas (§22.2) e o índice `chave →
  entradas` do ponto fixo (§22.3, a otimização). O ponto fixo da completa
  está feito; entre as completas o efêmero é uma ocorrência atribuída à
  chave, que retém (nunca mata antes da hora).
* Política de agendamento da rodada de ciclos por orçamento (§22.5); hoje
  ela roda na completa.
* Reduzir o custo da reclamação por bloco (`EspacoDeObjetos::soltar_morto`,
  já usada pela drenagem pura, sem varrer páginas): a junção dos vizinhos
  na ponta da lista está feita (§6.2), sem ganho demonstrado na média
  completa; avaliar os intervalos contíguos que não chegam à ponta (§19.4).
* A recarga com código antigo retido e o descritor por versão de layout
  (§23.4), isolados com mensagens contadas (§23.2) e FFI (§23.3).
* Comparação ARC × rastreamento × VM e decisão (§13, P6).
