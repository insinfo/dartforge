# O motor de geração de código (`crates/build`) — contrato

Contrato do motor que substitui o `build_runner` no ciclo do DartForge
(`docs/BUILD-RUST.md`, Fase 1). Escrito antes do código (passo B0) e
mantido junto dele: cada seção diz **o que** o motor garante e **de onde**
vem a regra no código oficial. O protocolo do executor Dart está em
`docs/BUILD-PROTOCOLO.md`.

Referência fixada: as versões que o Dart 3.6.2 resolve hoje —
`build_runner` 2.4.15, `build_runner_core` 8.0.0, `build` 2.4.2,
`build_config` 1.1.2, `graphs` 2.3.2, `glob` 2.1.3 (pub cache,
`PC = C:/Users/pmro/AppData/Local/Pub/Cache/hosted/pub.dev`). O `master` do
`dart-lang/build` (em `references/dart-build`) é referência de arquitetura,
não de contrato.

## 1. O que o motor faz, em uma frase

Lê a configuração que o `build_runner` leria, monta **o mesmo plano de
fases**, e para cada ação decide — pela impressão digital do que a ação
consultou — se executa ou reaproveita; publica as saídas numa `Geracao`
em memória (`crates/elements/src/gerado.rs`), atômica.

Duas perguntas, nada mais: **quando** (plano, agenda, impressão) e
**o quê** (executores). O motor não sabe gerar nada sozinho.

## 2. Configuração e plano (`config/`, `pacotes.rs`, `plano.rs`)

* **Grafo de pacotes** — `PackageGraph.forPath`
  (`PC/build_runner_core-8.0.0/lib/src/package_graph/package_graph.dart:60-148`):
  pacotes do `package_config.json` em ordem de nome; tipo de dependência
  do `pubspec.lock`; dependências da raiz = `dependencies` ∪
  `dev_dependencies`, dos outros só `dependencies`, cada lista ordenada.
  `$sdk` entra no fim, sem dependências.
* **Ordem dos pacotes** — SCCs de Tarjan a partir da raiz
  (`build_script_generate.dart:86-92`, `graphs/strongly_connected_components.dart`),
  reproduzidos iterativamente com a mesma pilha, para que empates saiam
  iguais.
* **`build.yaml`** — `build_config` 1.1.2: `BuilderDefinition`,
  `PostProcessBuilderDefinition`, `BuildTarget`, `TargetBuilderConfig`,
  `GlobalBuilderConfig`, `InputSet`, com as chaves permitidas de cada um
  (`*.g.dart`) — chave desconhecida é erro, como o `$checkKeys`. As chaves
  normalizam como `key_normalization.dart`. `<pacote>.build.yaml` na raiz
  substitui o do pacote (`build_config_overrides.dart`).
* **Ordem dos builders** — `findBuilderOrder`
  (`builder_ordering.dart`): Kahn com fila de prioridade pela chave,
  arestas por `required_inputs` (sufixo de uma saída declarada) e
  `runs_before` (definição e `global_options` da raiz), resultado invertido.
  Ciclo é erro. Pós-processadores depois, na ordem dos pacotes.
* **Aplicações** — a forma canônica de cada `apply(...)`/
  `applyPostProcess(...)` que o `build.dart` gerado traz: chave, fábricas
  (`import#nome`), filtro (`toNoneByDefault`, `toDependentsOf(p)`,
  `toAllPackages`, `toRoot`), `isOptional`, `hideOutput`,
  `defaultGenerateFor`, as três opções padrão e `appliesBuilders`.
  `dartforge build --plano` imprime isso; o oráculo é o
  `.dart_tool/build/entrypoint/build.dart` lido (nunca gerado) — o
  `tests/corpus.rs` e `--plano --comparar-com <build.dart>` comparam as
  duas formas canônicas.
* **Fases** — `createBuildPhases` (`apply_builders.dart:196-349`): SCCs
  dos alvos (na ordem de inserção de `allModules`) × aplicações × fábricas
  × alvos do SCC que passam em `_shouldApply` (saída `source` só na raiz;
  `enabled` explícito vence; `auto_apply_builders` ∧ filtro; ou
  `applies_builders` de uma âncora aplicada). Opções:
  `defaults.options` ⊕ `dev|release` ⊕ (alvo: `options` ⊕ `dev|release` ⊕
  `global_options`), e `isRoot` na raiz. Pós-processadores fundidos numa
  fase só, no fim.
* **Extensões** — as de **execução** (`build/src/generate/expected_outputs.dart`):
  sufixo, `^caminho` exato, `{{nome}}` com `(.+)` guloso e primeiro casamento.
  Quem diz as extensões de execução é o objeto `Builder`
  (`buildExtensions`): com o executor Dart disponível, o motor pergunta a
  cada fase sem gerador nativo verificado (`build.extensoes`) e refaz o
  grafo se divergirem; sem executor, valem as do **descritor** do builder
  (§6) e, sem descritor, as do `build.yaml` (que não separam as fábricas de
  um builder com várias).
* **Fontes** — por alvo: `sources.include` ou os padrões
  (`options.dart:24-51`), filtrados pela visibilidade (`target_graph.dart`:
  fora da raiz só `lib/**`, `bin/**`, `pubspec.yaml`, `CHANGELOG*`,
  `LICENSE*`, `README*` e `additional_public_assets`), menos `exclude`.
  Mais os nós sintéticos `lib/$lib$`, `test/$test$`, `web/$web$`,
  `$package$` de cada pacote (`graph.dart:574-581`).
* **Glob** — semântica do `package:glob` 2.1.3 (`src/ast.dart`,
  `src/parser.dart`): `*` = `[^/]*`, `**` = qualquer coisa, `?`, `[...]`/
  `[!...]` que nunca casam `/`, `{a,b}`, `\` escapa; casamento do caminho
  inteiro; **sem distinção de maiúsculas no Windows** (o `Glob` usa o
  `p.context` da plataforma), como o oráculo roda.

### 2.1 Perfis de versão (`perfil.rs`, `gatilhos.rs`, `linha_de_comando.rs`)

O `pubspec.lock` escolhe as regras (DF-BUILD-011): o `build_config` decide o
parser (1.1.2 recusa `triggers`; 1.2.0 aceita; 1.3.x aceita `build_to` nos
pós-processadores e prende a chave de definição ao pacote) e o `build_runner`
decide a execução (≥ 2.7.0: `run_only_if_triggered` + `triggers`; ≥ 2.14.0:
`--build-filter asset:`; ≥ 2.15.3: chave duplicada é erro). Sem `build_runner`
no lock, o `build_config` decide também os triggers.

* **Triggers** — porte de `build_triggers.dart` e `_allowedByTriggers`
  (`build/build.dart` do 2.16.1): opção `run_only_if_triggered: true` nas
  opções fundidas da fase; triggers acumulados de todos os pacotes pela chave
  escrita; `import x` = import `package:x` literal da unidade primária;
  `annotation N` = metadado de declaração de topo (inteiro ou sem o primeiro
  trecho), também nas partes legíveis pela fase. A análise é pelos tokens do
  lexer, no nível de topo; fonte que o lexer recusa conta como disparada.
  Passo não disparado: `Origem::NaoDisparada`, sem saídas, com as leituras
  (entrada e partes) como consultas.
* **CLI** — `dartforge build --define <b>=<o>=<v>` (JSON do Dart, senão
  texto; fundido por chave sobre as `global_options`, depois de
  `dev`/`release`), `--config <nome>` (`build.<nome>.yaml` substitui o
  `build.yaml` da raiz na configuração de execução — alvos, globais,
  triggers —, não no script) e `--build-filter` (relativo = `Uri.path`
  escapado; `package:` sob `lib/`; `asset:` no 2.14+). Com filtro, rodam as
  ações de fase não opcional com saída filtrada e visível, a cadeia das
  entradas geradas e, em passadas seguintes, as que um passo executado leu
  (a construção sob demanda do oficial).
* **Oráculo** — `corpus/builders/perfil_2_4` (2.4.15) e
  `corpus/builders_novos/perfil_2_16` (2.16.1, fora de `corpus/builders`
  porque exige Dart ≥ 3.11 no `pub get`), `oraculos/<configuração>/`,
  gerados por `scripts/corpus-perfis.py`; teste `tests/perfis.rs`.

## 3. Grafo de saídas (`grafo.rs`)

Para cada fase, em ordem, as entradas que casam
(`_actionMatches`, `graph.dart:374-395`: mesmo pacote, `generate_for`,
extensão com saída, e `sources` do alvo aplicado **à fonte original** da
cadeia) geram as saídas esperadas (`_addInBuildPhaseOutputs`); as saídas
viram entradas das fases seguintes. Uma saída que coincide com uma fonte
**substitui** a fonte (arquivo `source` que ficou no disco de um build
anterior). Duas fases com a mesma saída é erro (`DuplicateAssetNodeException`).

**Calcular não é executar**: o grafo responde "este `import` aponta para
uma saída de builder?" e "esta saída de `cache` é legível daqui?".

Legibilidade (`build_impl.dart:443-463`): saída de fase posterior não é
legível; da mesma fase, só a própria; de fase anterior, só se foi escrita.
Fora da raiz só o visível (`target_graph.dart`). Só os pacotes com alguma
fase que gera têm as fontes listadas; nos outros (as dependências que um
builder lê pelo resolver) uma fonte é decidida caminho a caminho pelo filtro
do pacote — `sources` dos alvos e visibilidade — mais a existência no disco
(`Grafo::externos`, `fonte_externa`).

## 4. Consultas e impressão digital (`consulta.rs`, `impressao.rs`)

```rust
pub enum Consulta {
    Arquivo(AssetId),               // bytes
    Existe(AssetId),                // canRead, também o negativo
    Glob { pacote, padrao },        // lista ordenada dos ids que casam
    ApiBiblioteca(uri),             // api (dev/hashes.rs) + anotações
    Declaracao { biblioteca, nome },// superfície: assinatura, anotações, membros públicos
    Indice { espaco, termo },       // p.ex. ("ng.seletor", "x-y")
    FonteBiblioteca(uri),           // conservador: o texto de todas as unidades
}
```

* A impressão digital de uma ação é
  `blake3(versão do motor ‖ chave e fábrica ‖ versão imitada ‖ digest das
  opções ‖ identidade da fase ‖ entrada primária ‖ Σ(consulta, digest))`.
  O digest das opções é blake3 da forma canônica (chaves ordenadas), não o
  md5 do oficial — não precisa ser igual, só estável.
* **Revalidação**: um evento (arquivo mudou; biblioteca com hash novo)
  marca `TalvezSujo` as ações do índice reverso daquela chave; o motor
  recalcula só os digests das consultas gravadas — iguais, `Atual` sem
  executar; diferente, executa. Os três estados são os de
  `asset_graph/node.dart:112-120`.
* **Corte pela saída** (`build_impl.dart:681-725`): saída com o mesmo
  digest não suja dependentes nem invalida unidades.
* **Glob depois de evento estrutural** (arquivo novo ou apagado no
  pacote): a lista de candidatos do `findAssets` é refeita no grafo novo e
  a ação só reexecuta se a resposta mudou (o `GlobAssetNode` do oficial);
  a consulta gravada passa a ter os candidatos novos.
* **Solidez**: o executor só lê pelo contexto que registra consultas
  (`CtxGerador`/`ServicoBuildStep`). O que ele não consultou não pode
  invalidá-lo. O invariante de verificação é **incremental = do zero**.
* **Visibilidade**: uma consulta de arquivo sobre uma saída que a ação não
  enxerga pela fase (de fase posterior, de outra ação da mesma fase, ou a
  própria) não entra na revalidação: a resposta que a ação recebeu não
  depende do conteúdo, e o `canRead` negativo de um `.g.dart` futuro não
  pode sujar o `json_serializable` quando o `.g.dart` aparece.

### 4.1 Estado entre processos (B04, opcional)

O equivalente do `asset_graph.json`: `.dart_tool/dartforge/build/estado/`
(`persistencia.rs`). **Só quando o usuário pede** — `dartforge build
--estado`, ou `DARTFORGE_BUILD_ESTADO=1` para `build`, `compile-js`, `dev` e
`serve` —, pela regra governante 6 do `PLANO.md` ("o disco só recebe o que o
usuário pedir"). Sem pedido o motor continua só em memória (o D-B1 vale como
padrão).

* **O que é salvo** — por ação, a chave (identidade da fase com as extensões e
  as entradas de pós-processador já conferidas, opções, `isRoot`, entrada,
  saídas previstas), a origem, as consultas com os digests e o digest de cada
  saída; o conteúdo vai para `blobs/`, endereçado pelo blake3 e conferido na
  leitura. A gravação é atômica e os blobs sem referência são apagados.
* **O que não é salvo** — ação com consulta semântica (depende do banco da
  sessão; numa passada única ele nem responde), ação de gerador por pacote (a
  revalidação é a rodada do pacote: o ngdart no estágio A), pendente e
  medida.
* **Entradas ocultas** (o que nenhuma consulta vê) — na chave global: versão
  do motor, identidade do executável (código dos geradores nativos: caminho,
  tamanho e data), plano canônico, versões do lock, `--release`. Para ação do
  executor Dart, o **código do builder**: os arquivos do depfile do bootstrap
  fora dos pacotes `hosted` (fixados pelo lock), pelo conteúdo; mudou um, as
  ações Dart salvas não valem. Sem essa lista, ação Dart não é salva.
* **Restauração** — na primeira atualização, depois de conferidas as
  extensões com o executor: volta a ação cuja chave existe e cuja origem é a
  que este motor escolheria (o mesmo nativo; Dart com o código conferido,
  mesmo sem executor nesta sessão; apoio só sem nativo e sem executor). A
  primeira verificação de uma ação restaurada recalcula **todas** as
  consultas (um `GlobAtivos` com os candidatos do grafo atual) e confere no
  disco as saídas `source`, que a CLI só regrava quando o texto muda.
* Medido (`estado_salvo_pela_vm`, `json_serializable`, motor em `debug`):
  primeiro processo 19,9 s (5 ações Dart pela VM); segundo processo 0,27 s, 0
  ações executadas, estado igual ao de um motor do zero. Testes:
  `estado_salvo_entre_processos` (entrada editada, opção global alterada,
  código do builder alterado, saída `source` apagada, estado corrompido) e
  `estado_salvo_pela_vm`.

## 5. Agenda (`agenda.rs`)

Fases em série; ações da fase em paralelo com `std::thread::scope` e
índice atômico, até `min(núcleos, 8)` trabalhadores; resultados num
`BTreeMap` por `(fase, entrada)`, diagnósticos ordenados, id da geração =
blake3 da lista ordenada. **Resultado idêntico com 1, 4 e 8
trabalhadores** — verificado pelo teste de determinismo.

**Preguiça** (D-B6): saída `build_to: cache` só é calculada quando alguém a
lê — o carregador (import), o servidor (pedido HTTP), outro builder, ou
`--comparar`. Ações `isOptional` idem (é o `is_optional` do oficial). Com o
executor Dart, as fases ocultas que ele executa entram mesmo na demanda do
carregador: um builder posterior pode lê-las (o `combining_builder` lê as
partes `.g.part` por glob) e o motor não interrompe uma ação Dart para
calcular outra. Ficam preguiçosas as opcionais e as de gerador nativo.
As ações Dart de uma fase executam em série no processo único (o executor
é um só por sessão, atrás de um `Mutex`). **Decisão (B04)**: não há
execução paralela de ações Dart. O `build_runner` as intercala num isolate só
(`Future.wait`), com um `Resolvers` compartilhado cujo `reset` entre builds
exige que nenhuma ação esteja em curso; o canal `dfexec/1` atende uma ação por
vez e o `ServicoAcao` empresta o grafo à ação corrente. Paralelizar pediria
vários processos (cada um com o seu analyzer e o seu resumo do SDK, a memória
que o `build_runner` evita) ou multiplexar o canal sem garantir isolamento
entre builders que não foram escritos para isso. Os geradores nativos e o
apoio seguem em paralelo.
Saídas `build_to: source` são calculadas sempre e vão ao disco só quando o
texto muda (D-B2).

## 6. Executores (`executor/`)

Por ação, na ordem, o primeiro que aceita:

1. **Nativo** (`GeradorNativo`, Rust, lê o banco semântico): há gerador,
   a versão do lock está no `imita` do descritor, e ele não recusou.
2. **Dart** (`ExecutorDart`, `docs/BUILD-PROTOCOLO.md`): o motor chama o
   executor e serve `BuildStep` com visibilidade, consultas e saídas
   permitidas, pelo canal `dfexec/1` compartilhado com as macros. O
   executor que existe é o **pela VM Dart** (`vm.rs` + `pacotes/build_executor`):
   executa o builder do ecossistema de verdade (o `runBuilder` do
   `package:build`, o `AnalyzerResolvers` do `build_resolvers`), ligado por
   `dartforge build --dart <exe>` ou `DARTFORGE_BUILD_DART`. O **nativo**
   (`executor_nativo.rs`, B01) é o mesmo bootstrap compilado pelo
   `compile-native` do DartForge, sem VM: é o padrão do `dartforge build`
   com a feature `nativo` (`DARTFORGE_BUILD_NATIVO=0` o desliga; nos
   compiladores só com `DARTFORGE_BUILD_NATIVO=1`). O executável fica em
   `.dart_tool/dartforge/build/executor-<chave>`, com um depfile dos fontes
   que o programa carrega (o código dos builders para o motor); o processo
   recebe `DARTFORGE_PACKAGE_CONFIG` (o `--packages` da VM) e
   `DARTFORGE_DART_SDK` (o SDK do resumo do analyzer). Os pacotes do executor
   que o projeto não resolve vêm do cache do pub
   (`dependencias_executor.rs`). Sem executor, o padrão é
   `Indisponivel(motivo)`. A sessão
   encerra o executor ao terminar ou quando ele é substituído. Cada
   mensagem do executor (VM ou nativo) tem prazo: `DARTFORGE_BUILD_PRAZO`
   segundos sem resposta (padrão 600; `0` desliga; as consultas `build.*`
   do meio da ação reiniciam a contagem) e o processo é encerrado — a ação
   em curso vira `Origem::Falha` com o motivo, e as seguintes seguem a
   política de executor indisponível (`dfexec::CanalDeProcesso::com_prazo`,
   `cliente::prazo_do_executor`). No
   `corpus/builders` pela VM: **57 iguais / 0 pendentes / 0 diferentes**
   (`crates/build/tests/executor_vm.rs`, com a saída do pós-processador do
   `cadeia_configuracao`), e incremental = do zero nas 12 edições.
3. **Apoio**: o que o `build_runner` deixou no disco — saída `source` na
   árvore, saída `cache` em `.dart_tool/build/generated/<pkg>/<caminho>`.
   Se a entrada primária é mais nova que o apoio: **aviso** único no
   `dev`/`serve`, **erro** com `dartforge build --estrito` (D-B5):
   `<builder>: <saída> pode estar desatualizado — <motivo da recusa ou falha>;
   rode 'dart run build_runner build'`.
4. **Nenhum**: a ação fica **pendente** com motivo; o placar conta.

Recusa sempre tem motivo, e o placar é `iguais/pendentes/diferentes` com
os motivos.

**Descritor** por builder conhecido (`nativos/*.rs`): extensões de
execução a partir das opções, versões imitadas, gerador nativo.
**Substituídos** (ficam no plano, não geram ação): `build_web_compilers:*`,
`build_modules:*`, `build_resolvers:*`, `build_test:*` (fora de
`dartforge test`).

**Pós-processadores** (`post_process_builders`, §6.1): executados pelo
executor Dart, como o `build_runner_core` 8.0.0 os executa.

### 6.1 Pós-processadores

* **Âncoras** — `_addPostBuildPhaseAnchors` (`graph.dart:465-482`): na fase
  de pós-processamento (a última), uma ação por entrada do pacote — fontes e
  todas as saídas das fases de build — que termina numa das
  `inputExtensions`, casa o `generate_for` e cuja fonte original está nos
  `sources` do alvo (`_actionMatches`). As `inputExtensions` são as do
  **objeto** `PostProcessBuilder` (o oficial ignora o `input_extensions` do
  `build.yaml`), perguntadas ao executor por `build.entradas_pos`; sem
  executor Dart a fase não tem âncoras e a saída fica pendente no placar.
* **Execução** — `build.posprocessar`: o `runPostProcessBuilder` do
  `package:build` com o leitor e o escritor do motor. Uma entrada gerada que
  não foi escrita não executa a âncora (`wasOutput`, origem `Omitida`). A
  âncora depende do digest da entrada primária mesmo que o pós-processador não
  a leia (`_postProcessBuildShouldRun`: o `part_cleanup` só apaga).
* **Saídas** — o grafo não as prevê: ficam no registro da âncora, ocultas,
  publicadas na geração e contadas no placar. O `addAsset` do oficial recusa
  o que o grafo já tem; aqui a escrita é recusada se o asset é fonte, saída
  prevista, saída de outra âncora ou já foi escrito pela mesma ação. Uma
  reexecução que não escreve mais uma saída a retira (`_cleanUpStaleOutputs`).
* **`deletePrimaryInput`** — o `deletedBy` do nó; só o `FinalizedReader` do
  oficial (o `serve` e o diretório mesclado `build -o`) deixa de enxergar a
  entrada, os builders continuam lendo. Numa entrada **gerada** o efeito é o
  mesmo aqui: ela sai da geração publicada (a `.g.part` que o
  `source_gen:part_cleanup` apaga). Numa **fonte** o DartForge não tem como
  escondê-la (o `serve` lê o disco e não há `build -o`): aviso, e erro com
  `--estrito`. Teste: `pos_processador_pela_vm`.

## 7. Sessão, `serve`, `compile-js`, `build`

* `EtapaDeGeracao` (trait em `crates/dev`, implementada pelo motor; o
  hospedeiro de macros a implementa depois):
  `saidas_esperadas()` (filtra o "não foi possível ler" da primeira
  passada), `observados()` (entradas não-Dart), `atualizar(ctx, mudados)`
  (nova `Geracao` + caminhos alterados).
* `Sessao::compilar` com etapa: carrega com a geração corrente, atualiza a
  etapa com o `Program` já carregado (é o `BuildStep.resolver` sem carga
  extra), e se a geração mudou invalida só as unidades geradas cujo texto
  mudou e recarrega.
* `serve` procura a geração corrente antes do disco (um `.css` pedido pelo
  navegador é demanda).
* `compile-js` usa o motor numa passada; `DARTFORGE_GERADOS` fica um ciclo
  como sinônimo (`build_runner` = só apoio; `ng` = padrão com motor).
* `dartforge build [--release] [--plano] [--comparar] [--escrever-cache
  <dir>] [--trabalhadores N] [--estrito] [--dart <exe>] [--estado]
  [--define ...] [--config <nome>] [--build-filter <glob>]` (§2.1);
  `--dart` (ou `DARTFORGE_BUILD_DART`, que vale também para `dev`, `serve` e
  `compile-js`) liga o executor de builders pela VM; `--estado` (ou
  `DARTFORGE_BUILD_ESTADO=1`, idem) liga o estado entre processos (§4.1).

## 8. Custo zero (regra governante, PLANO.md)

O motor só existe se o `package_config.json` que o carregador já lê
contém `build_runner`. Sem ele: nenhum YAML lido, nenhum grafo, nenhuma
thread; `Sessao.etapas` vazio; `Relatorio.motor == None`;
`dartforge_build::instancias() == 0`. Portões: teste estrutural
(`crates/dev/tests/custo_zero.rs`), tempo no `pesado.yml` (corpus JS com e
sem os recursos, mediana), e a edição de corpo no `new_sali/core` medida
localmente antes do merge.
