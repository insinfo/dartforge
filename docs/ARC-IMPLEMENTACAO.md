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

## 2. Desenho desta etapa: RC adiado das raízes, contado no heap

O runtime é um só para os dois modos. O ARC liga por isolado, em tempo de
execução.

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
* Reclamação por bloco, sem varrer as páginas (§19.4, caminho otimizado).
* A recarga com código antigo retido e o descritor por versão de layout
  (§23.4), isolados com mensagens contadas (§23.2) e FFI (§23.3).
* Comparação ARC × rastreamento × VM e decisão (§13, P6).
