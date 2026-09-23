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
  Quem diz as extensões de execução é o **descritor** do builder (§6);
  sem descritor, as do `build.yaml`.
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
Fora da raiz só o visível (`target_graph.dart`).

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
* **Solidez**: o executor só lê pelo contexto que registra consultas
  (`CtxGerador`/`ServicoBuildStep`). O que ele não consultou não pode
  invalidá-lo. O invariante de verificação é **incremental = do zero**.
* Só em memória (D-B1): nada de registro em disco por enquanto.

## 5. Agenda (`agenda.rs`)

Fases em série; ações da fase em paralelo com `std::thread::scope` e
índice atômico, até `min(núcleos, 8)` trabalhadores; resultados num
`BTreeMap` por `(fase, entrada)`, diagnósticos ordenados, id da geração =
blake3 da lista ordenada. **Resultado idêntico com 1, 4 e 8
trabalhadores** — verificado pelo teste de determinismo.

**Preguiça** (D-B6): saída `build_to: cache` só é calculada quando alguém a
lê — o carregador (import), o servidor (pedido HTTP), outro builder, ou
`--comparar`. Ações `isOptional` idem (é o `is_optional` do oficial).
Saídas `build_to: source` são calculadas sempre e vão ao disco só quando o
texto muda (D-B2).

## 6. Executores (`executor/`)

Por ação, na ordem, o primeiro que aceita:

1. **Nativo** (`GeradorNativo`, Rust, lê o banco semântico): há gerador,
   a versão do lock está no `imita` do descritor, e ele não recusou.
2. **Dart** (`ExecutorDart`, `docs/BUILD-PROTOCOLO.md`): hoje só existe a
   implementação `Indisponivel(motivo)`. A implementação virá do executor
   nativo auto-hospedado, compartilhado com as macros.
3. **Apoio**: o que o `build_runner` deixou no disco — saída `source` na
   árvore, saída `cache` em `.dart_tool/build/generated/<pkg>/<caminho>`.
   Se a entrada primária é mais nova que o apoio: **aviso** único no
   `dev`/`serve`, **erro** com `dartforge build --estrito` (D-B5):
   `<builder>: <saída> pode estar desatualizado — o DartForge ainda não
   executa builders Dart (BUILD-RUST.md §3, Fase 1); rode
   'dart run build_runner build'`.
4. **Nenhum**: a ação fica **pendente** com motivo; o placar conta.

Recusa sempre tem motivo, e o placar é `iguais/pendentes/diferentes` com
os motivos.

**Descritor** por builder conhecido (`nativos/*.rs`): extensões de
execução a partir das opções, versões imitadas, gerador nativo.
**Substituídos** (ficam no plano, não geram ação): `build_web_compilers:*`,
`build_modules:*`, `build_resolvers:*`, `build_test:*` (fora de
`dartforge test`). Pós-processadores só apagam arquivos de um diretório
mesclado (`build -o`): no motor são no-op declarados.

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
  <dir>] [--trabalhadores N] [--estrito]`.

## 8. Custo zero (regra governante, PLANO.md)

O motor só existe se o `package_config.json` que o carregador já lê
contém `build_runner`. Sem ele: nenhum YAML lido, nenhum grafo, nenhuma
thread; `Sessao.etapas` vazio; `Relatorio.motor == None`;
`dartforge_build::instancias() == 0`. Portões: teste estrutural
(`crates/dev/tests/custo_zero.rs`), tempo no `pesado.yml` (corpus JS com e
sem os recursos, mediana), e a edição de corpo no `new_sali/core` medida
localmente antes do merge.
