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
| `cache` (`add`, `clean`, `list`, `preload`, `repair`) | ausente | |
| `global` (`activate`, `deactivate`, `list`, `run`) | ausente | |
| `run` | ausente | use `dartforge run` |
| `publish`/`lish`, `uploader`, `token`, `login`, `logout` | ausente | |
| `bump`, `unpack`, `workspace list`, `dependency_services` | ausente | |
| Sugestões após falha ("You can try one of the following suggestions") | ausente | o Pub re-resolve variações; o DartForge imprime só a explicação |
| SDK Flutter/Fuchsia | parcial | tratados como indisponíveis (o Pub sem `FLUTTER_ROOT` faz o mesmo) |
| Workspaces | suportado | como o Pub 3.6.2: membros por caminho literal (glob não existe na 3.6.2: `pkgs/*` é um diretório), `resolution: local/external` não são raiz, todas as validações do `validateWorkspace` |
| Git local com caminho relativo | suportado | URL resolvida em `file://`, escrita relativa no lockfile |

Opções próprias do DartForge em todos os subcomandos: `--sdk <lib>` (exceto no `add`,
onde `--sdk` é a fonte SDK do Pub) e `--sdk-version <versão>`.

## Matriz das opções do `dart test`

| Opção | Estado | Observações |
|---|---|---|
| `--platform vm` (`-p`) | suportado | é a única plataforma; reportada com o compilador `exe` |
| `-p chrome/firefox/node/...` | ausente | recusado explicitamente |
| `-r expanded` | suportado | formato do `ExpandedReporter` (linhas `MM:SS +n ~n -n: ...`, `loading`, `[E]`, `Skip:`, resumo, aviso de `--chain-stack-traces`, cores pela configuração). Diferem o tempo decorrido e os quadros dos rastros de pilha nativos; em execução paralela os eventos de cada suíte saem juntos |
| `-r json` | parcial | eventos próprios (`testStart`, `testDone`, `error`, `print`, `done`) por suíte, não o protocolo JSON do package:test |
| `-r compact/github/failures-only/silent` | ausente | recusados |
| `--file-reporter` | ausente | recusado |
| `-n/--name`, `-N/--plain-name`, `path?name=` | suportado | |
| `-t/--tags`, `-x/--exclude-tags`, tags do `dart_test.yaml` | suportado | |
| `-j/--concurrency`, `--total-shards`, `--shard-index` | suportado | |
| `--timeout`, `--ignore-timeouts`, `--run-skipped`, `--no-retry`, `-P/--preset` | suportado | |
| `dart_test.yaml` e anotações (`@TestOn`, `@Timeout`, `@Skip`, `@OnPlatform`, `@Tags`, `@Retry`) | suportado | via `Configuration`/`Loader` do próprio test_core |
| seleção por linha/coluna (`path:linha:coluna`) | ausente | recusado |
| `--coverage`, `--pause-after-load`, `--debug`, `--fail-fast` | ausente | recusados |
| watch | ausente | recusado |

Evidências: `bench/results/2026-10-31-pub-test/README.md`.
