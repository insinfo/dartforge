# `dartforge pub` e `dartforge test` como substitutos de `dart pub` e `dart test` (SDK 3.6.2)

Referência: Pub na revisão `e6704cc5219f3767ca5680ef8691c55133250664` (o `pub_rev` do
`DEPS` da tag 3.6.2 do SDK) e `pub_semver` `8cce9d00431b6653026cdfcf6cf8548078c56f02`;
package:test 1.25.7 / test_core 0.6.x. O código está em `crates/cli/src/pub_get*` e
`crates/cli/src/test_runner*`. Nada delega para o Dart ou o Pub.

## Como o `pub` funciona agora

* **Solucionador.** Porte do `VersionSolver` do Pub (PubGrub: propagação unitária,
  resolução de conflitos com aprendizado de incompatibilidades e retrocesso não
  cronológico), com as mesmas heurísticas: pacote com menos versões primeiro, versão
  travada preferida, estável mais nova primeiro (mais antiga no `downgrade`), fatos na
  ordem das dependências do pubspec e da listagem do servidor, intervalos de
  dependência agrupados como no `PackageLister` (`_dependencyBounds`, restrições de SDK
  por faixa). Por isso a solução é a mesma do Pub, não só "uma solução válida". O
  limite de 100000 decisões da busca antiga sumiu.
* **Versões e restrições.** `pub_semver` 2.1: sufixo de build entra na ordem e na
  igualdade, identificadores numéricos comparados como número, `<2.0.0` exclui as
  pré-versões de 2.0.0, `^` impresso só quando escrito assim (o `sdks:` do lockfile sai
  `>=3.5.0 <4.0.0`), forma "terse" nas mensagens do solucionador.
* **Pubspecs.** Árvore com ordem de inserção para YAML e JSON (a ordem das dependências
  muda a solução). Fontes `hosted`, `path`, `git`, `sdk` e desconhecidas; erros de
  pubspec de dependências adiados como no Pub; reinterpretação das restrições de SDK
  (`<2.0.0` padrão, `<3.0.0` → `<4.0.0` para ≥2.12, Flutter sem limite superior).
* **Saídas.** `pubspec.lock` e `.dart_tool/package_config.json` no formato exato do Pub
  (`generator: pub`, `generatorVersion`, `pubCache`), `workspace_ref.json`, remoção dos
  arquivos soltos dos membros, mensagens (`Resolving dependencies...`, lista `+ > < ~ * !`,
  `(x available)`, resumo, pacotes desatualizados e descontinuados) e explicação de
  falha do Pub (`Because ... version solving failed.`).
* **Cache.** Layout do Pub (`hosted/<servidor>/<pacote>-<versão>`, `hosted-hashes`,
  `git/cache/<nome>-<sha1>`), nome de diretório do servidor como `_urlToDirectory`.
  Extração como o `extractTarGz`: links internos mantidos (link físico vira simbólico),
  links que saem do pacote ignorados, nomes longos GNU lidos mesmo em arquivo `ustar`,
  modo 0644 mais os bits de execução.

## Matriz dos subcomandos do `pub`

| Subcomando | Estado | Observações |
|---|---|---|
| `get` | suportado | `--offline`, `-n/--dry-run`, `--enforce-lockfile`, `--[no-]example`, `-C`, `--packages-dir` (aviso). `--precompile` recusado. `example/` da raiz resolvido com saída resumida, como no Pub |
| `upgrade` | suportado | pacotes, `--unlock-transitive`, `--major-versions`, `--tighten`, `-n`, `--offline`, `--[no-]example`, `-C`; `--null-safety` dá o erro do Pub |
| `downgrade` | suportado | pacotes, `--tighten`, `-n`, `--offline`, `-C` |
| `add` | suportado | `dev:`/`override:`, descritores YAML (`{path: ..}`, `{git: ..}`, `{hosted: ..}`, `{sdk: ..}`), restrições, `--dev`, `--path`, `--git-url/--git-ref/--git-path`, `--hosted-url`, `--sdk`, `-n`, `--offline`, `-C`. Edição do pubspec como o `yaml_edit` (inserção ordenada só em mapa ordenado com mais de uma chave, aspas, mapa vazio, mapa em fluxo) |
| `remove` | suportado | inclusive `override:`; seção esvaziada removida |
| `deps` | suportado | `-s tree/compact/list`, `--[no-]dev`, `--json`, `--executables`, `-C` |
| `outdated` | parcial | tabela, `--json`, `--[no-]dev-dependencies`, `--[no-]dependency-overrides`, `--[no-]transitive`, `--[no-]prereleases`, `--show-all`/`--up-to-date`, `-C`. Avisos de segurança (advisories) não são buscados |
| `cache` (`add`, `clean`, `list`, `preload`, `repair`) | suportado | `add` (`--version`, `--all`, "Already cached"), `clean` (`-f` e a pergunta), `list` (JSON do servidor padrão), `preload` (hash do arquivo, servidor padrão), `repair` (hosted reinstalado do servidor, Git com `clean`/`reset`, pacotes globais reativados, código 69 nas falhas). Mesmos textos de ajuda e erros de uso. O `pub get` e o `cache add` agora também gravam o `README.md` do cache e as listagens `hosted/<servidor>/.cache/<pacote>-versions.json` com `_fetchedAt`, como o Pub |
| `global` (`activate`, `deactivate`, `list`, `run`) | suportado | `activate` hosted (restrição, `-u`), Git (`--git-ref`, `--git-path`, repositório relativo) e path; `-x`, `--no-executables`, `--overwrite`; pacote raiz `pub global activate`, `pubspec.lock` e `package_config.json` em `global_packages/<pacote>` iguais aos do Pub; *bin stubs* com o mesmo cabeçalho (`Package`, `Version`, `Executable`, `Script`), aviso de PATH, colisões, executáveis desconhecidos. `run` resolve de novo os pacotes path a cada execução e os do cache só quando falta o executável, como o Pub. Diferença do que fica no cache: onde o Pub grava o *snapshot* `bin/<script>.dart-3.6.2.snapshot` o DartForge grava o executável nativo `bin/<script>` (e `.dart_tool/pub/bin/<pacote>/<script>` para path); o *stub* executa esse binário ou `dartforge pub global run`. A ordem das linhas `Built pkg:exe.` é a de término das compilações no Pub (paralelas) e alfabética no DartForge |
| `run` | ausente | use `dartforge run` |
| `token` (`add`, `list`, `remove`) | suportado | `pub-tokens.json` no diretório de configuração do Dart (`applicationConfigHome('dart')`: `~/Library/Application Support/dart` no macOS, `$XDG_CONFIG_HOME/dart` ou `~/.config/dart` no Linux, `%APPDATA%\dart` no Windows) com a mesma ordem de chaves; token pelo stdin ou `--env-var`; avisos de nome de variável, URL normalizada (`pub.dartlang.org` → `pub.dev`, barra final), recusa de `http://` fora de localhost, `remove --all`, ajudas com a quebra de linha do package:args |
| `logout` | suportado | apaga `pub-credentials.json` ou diz que não existe |
| `uploader` | suportado | como no Pub 3.6.2: só aponta para `https://pub.dev/packages/<pacote>/admin` (código 1) |
| `login` | ausente | fluxo OAuth2 com navegador e servidor do Google; não há como testá-lo sem rede nem igualá-lo offline |
| `publish`/`lish` | ausente | ver "`pub publish`: o que falta" abaixo |
| `bump`, `unpack`, `workspace list`, `dependency_services` | ausente | |
| Sugestões após falha ("You can try one of the following suggestions") | ausente | o Pub re-resolve variações; o DartForge imprime só a explicação |
| SDK Flutter/Fuchsia | parcial | tratados como indisponíveis (o Pub sem `FLUTTER_ROOT` faz o mesmo) |
| Workspaces | suportado | como o Pub 3.6.2: membros por caminho literal (glob não existe na 3.6.2: `pkgs/*` é um diretório), `resolution: local/external` não são raiz, todas as validações do `validateWorkspace` |
| Git local com caminho relativo | suportado | URL resolvida em `file://`, escrita relativa no lockfile |

Opções próprias do DartForge em todos os subcomandos: `--sdk <lib>` (exceto no `add`,
onde `--sdk` é a fonte SDK do Pub) e `--sdk-version <versão>`.

## Como o `test` funciona agora

O `dartforge test` é o `dart test` com uma plataforma trocada. Um programa anfitrião
(`crates/cli/src/test_runner/host.dart`), compilado pelo DartForge uma vez por
configuração de pacotes e guardado em `.dart_tool/dartforge/test-host/<chave>/`, roda o
código do próprio test_core: `Configuration` (linha de comando, `dart_test.yaml`,
predefinições), um porte do `Runner` (`_loadSuites`, filtros, *sharding*, aviso de tags
desconhecidas, `NoTestsFoundException`, códigos de saída do `executable.dart`) e do
`Loader` (`loadDir`/`loadFile`, suíte pulada sem carregar, novas tentativas de carga), o
`Engine` com `stopOnFirstFailure` e **os relatores do próprio pacote**
(`allReporters[config.reporter]` e os `--file-reporter` por um `MultiplexReporter`).
Cada suíte vira um executável nativo em volta do *bootstrap* nativo do test_core
(`internalBootstrapNativeTest`, o mesmo do `dart test -c exe`) e conversa com o anfitrião
pelo protocolo de soquete do `VMPlatform` (`MultiChannel` sobre `jsonSocketStreamChannel`,
`deserializeSuite`). Por isso ids, `groupIDs`, `hidden`, `skipReason`, `testCount`,
`allSuites`, a ordem dos eventos e os textos de todos os relatores são os do package:test.

O DartForge só fornece duas coisas: as anotações de biblioteca (o `parseMetadata` do
test_core usa o analisador; o anfitrião pede a leitura do DartForge com
`dartforge test --dartforge-library-metadata`, agora também com o comentário
`// @dart=`) e a compilação nativa (`compile-native`, por padrão no máximo
`cpus/4` suítes ao mesmo tempo, `DARTFORGE_TEST_COMPILE_JOBS` muda). O anfitrião é
iniciado com `argv[0]` igual ao executável do DartForge, então o "To run this test again"
do relator `compact` mostra `<dartforge> test ...` onde o `dart test` mostra `<dart> test ...`.

**Rastros de pilha.** O executável nativo ainda não tem quadros de pilha
(`StackTrace.current` é `#0 main (dart:native)`; ver
`docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md` §13.14). O package:test usa o rastro da
declaração para `line`, `column`, `url` (e `root_*`) do JSON; com um rastro vazio o
`JsonReporter` quebraria (`frames.first`). O anfitrião detecta isso na partida e, só
nesse caso, pede às suítes que não coletem rastros de declaração: o relator recebe
rastro nulo e emite `line`/`column`/`url` nulos, como o próprio package:test faz quando
não há rastro. Quando o backend ganhar quadros, a coleta volta sozinha. Os
`stackTrace` dos eventos `error` e os rastros impressos pelos relatores de texto são os
do executável nativo (`dart:native  main`).

## `pub publish`: o que falta

Não foi feito nesta rodada. O `--dry-run` do Pub 3.6.2 só sai idêntico com todas as
peças abaixo; entregar parte delas daria avisos e erros diferentes nos pacotes que
acionam as que faltam, e por isso nada foi ligado:

1. `acquireDependencies` (já existe: o `pub get`) e a lista de arquivos do
   `Package.listFiles`, com as regras de `.pubignore`/`.gitignore` do `ignore.dart` do
   Pub (por diretório, `.pubignore` vence `.gitignore`, sempre fora `.git`, `pubspec.lock`
   só na raiz etc.);
2. a árvore com tamanhos (`ascii_tree.fromFiles`, `showFileSizes`) e o arquivo
   `.tar.gz` do `createTarGz` (cabeçalhos do package:tar, ordem, modos); o tamanho
   comprimido aparece arredondado em KB, então o gzip precisa sair com o mesmo nível e
   praticamente os mesmos bytes do `GZipCodec` do Dart;
3. os 25 validadores do `Validator.runAll`, na ordem: `FileCase`, `Analyze` (o
   `dart analyze` de `lib`, `bin`, `test`... — temos o `dartforge analyze`, 99,95% no
   corpus, mas as mensagens têm de sair como as do analisador embutido no Pub),
   `Gitignore` e `GitStatus` (rodam `git`), `Pubspec`, `License`, `Name`, `PubspecField`,
   `Dependency` (consulta o servidor), `DependencyOverride`, `DeprecatedFields`,
   `Directory`, `Executable`, `CompiledDartdoc`, `Readme`, `Changelog`, `SdkConstraint`,
   `StrictDependencies` (lê os `import`/`export` de todo arquivo Dart), `FlutterConstraint`,
   `FlutterPluginFormat`, `RelativeVersionNumbering` (consulta o servidor), `PubspecTypo`,
   `LeakDetection` (671 linhas de padrões de segredos), `Size` e `DevtoolsExtension`;
4. o protocolo de envio (`GET api/packages/versions/new`, `POST` multipart para a URL
   devolvida, `GET` da `location`) com `--force`, a confirmação e os erros de servidor,
   testado só contra um servidor falso local; autenticação por `pub-tokens.json` (já
   implementado) para servidores de terceiros e OAuth2 para o pub.dev.

## Matriz das opções do `dart test`

| Opção | Estado | Observações |
|---|---|---|
| `--platform vm` (`-p`) | suportado | é a única plataforma; reportada com o compilador `exe` |
| `-p chrome/firefox/node/...`, `-c` que não seja `exe` | ausente | recusado explicitamente |
| `-r expanded` | suportado | o `ExpandedReporter` do pacote; diferem o tempo e os quadros de pilha |
| `-r json` | suportado | o `JsonReporter` do pacote: `start`, `suite`, `group`, `testStart`, `print`, `error`, `testDone`, `allSuites`, `done` com os mesmos ids e campos. `line`/`column`/`url` saem nulos (ver acima); `debug` só existe com `--debug`, que é recusado |
| `-r compact` | suportado | o `CompactReporter`; difere o tempo (e quantas vezes a linha `loading` é redesenhada enquanto a suíte compila) e o executável do "To run this test again" |
| `-r github`, `-r failures-only`, `-r silent` | suportado | os relatores do pacote; diferem tempos e quadros de pilha |
| `--file-reporter <relator>:<arquivo>` | suportado | qualquer relator, como no `Runner` |
| `--fail-fast` | suportado | `Engine(stopOnFirstFailure:)` |
| `-n/--name`, `-N/--plain-name`, `path?name=` | suportado | |
| `-t/--tags`, `-x/--exclude-tags`, tags do `dart_test.yaml` | suportado | |
| `-j/--concurrency`, `--total-shards`, `--shard-index` | suportado | a concorrência de execução é a do `Engine`; a compilação nativa tem limite próprio |
| `--timeout`, `--ignore-timeouts`, `--run-skipped`, `--no-retry`, `-P/--preset`, `--test-randomize-ordering-seed` | suportado | |
| `dart_test.yaml` e anotações (`@TestOn`, `@Timeout`, `@Skip`, `@OnPlatform`, `@Tags`, `@Retry`) | suportado | |
| `--help`, `--version` | suportado | o uso do package:test; o DartForge acrescenta as opções dele depois |
| seleção por linha/coluna (`path:linha:coluna`) | ausente | recusado: depende dos quadros de pilha da declaração |
| `--coverage` | ausente | recusado; ver a análise abaixo |
| `--pause-after-load`, `--debug` | ausente | recusados (não há serviço de VM) |
| watch | ausente | não é opção do package:test 1.25 (erro de uso do próprio `Configuration`) |

Opções próprias do DartForge: `--sdk <lib>`, `--packages <config>`,
`--process-timeout <s>` (mata o executável de uma suíte depois de N segundos; sem limite
por padrão, como o `dart test`) e `--keep-artifacts`.

## `--coverage`: por que não há implementação

Desenho avaliado antes de escrever código (02/11). O `dart test --coverage <dir>` grava,
por suíte, `<dir>/<caminho>.vm.json` com o que o `package:coverage` coleta pelo serviço da
VM (`collect(..., isolateIds)`, `SourceReport` com `Coverage` e `forceCompile`):

```
{"type":"CodeCoverage","coverage":[{"source":"package:tst/tst.dart",
  "script":{"type":"@Script","fixedId":true,"id":"libraries/1/scripts/package%3Atst%2Ftst.dart",
            "uri":"package:tst/tst.dart","_kind":"library"},
  "hits":[1,1,2,1,5,1,3,0,8,0]}, ...]}
```

Medido no SDK 3.6.2 (`pt/cov` da sessão): o arquivo de uma suíte de 2 testes traz **223
scripts** — todas as bibliotecas carregadas no isolado, `package:test_api`,
`package:matcher`, `package:stack_trace`, o próprio *bootstrap* —, cada um com as linhas
"cobríveis" e uma contagem. As contagens não são execuções: `add` foi chamado 4 vezes e
as linhas dele valem 1; uma linha de `add_test.dart` vale 4. Cada linha conta quantos
intervalos do relatório (funções e *closures* compiladas) a tocaram.

Para reproduzir esse arquivo seria preciso, para todo o programa e não só para o código
do usuário:

1. o conjunto de linhas cobríveis da VM, que vem das posições de *token* que o compilador
   não otimizado da VM grava nos descritores de PC de cada função (chamadas, retornos,
   checagens), inclusive das funções nunca chamadas (`forceCompile`);
2. os intervalos (`ranges`) em que o `SourceReport` corta cada script, que definem a
   contagem de cada linha;
3. a lista de bibliotecas que a VM carrega no isolado de teste, com os ids
   `libraries/1/scripts/...`.

Nenhum dos três existe fora da VM: não são propriedade do código-fonte, e sim do
`kernel_to_il`/`flow_graph_builder` da VM e do carregamento do isolado. O backend nativo do
DartForge tem outra redução (outro conjunto de pontos de chamada e de funções) e não tem
serviço de VM. Instrumentar o nosso AOT ou JIT daria um mapa de linhas plausível que o
`format_coverage --lcov` leria, mas com linhas e contagens diferentes do `dart test` em
praticamente todo arquivo — um subconjunto disfarçado, que o pedido proíbe. Fidelidade
completa exigiria portar a atribuição de posições do compilador da VM inteira; fica
registrado como impossível nesta arquitetura, e `--coverage` continua recusado com
mensagem explícita.

Evidências: `bench/results/2026-10-31-pub-test/README.md` e `bench/results/2026-11-02-pub-test-2/README.md`.
