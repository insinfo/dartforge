# `dfexec/1` — protocolo entre o motor e o executor Dart

Contrato do canal entre o motor de build (`crates/build`, hospedeiro) e o
**executor** que roda código Dart em tempo de compilação: builders do
ecossistema (serviço `build.*`) e, depois, macros (serviço `macro.*`).
Um protocolo e um executor para os dois (regra governante, PLANO.md,
item 2).

Os dois lados existem e rodam builders reais:

* **hospedeiro** — `ServicoAcao` (`crates/build/src/executor.rs`) é o
  `ServicoBuildStep` que limita leituras ao grafo, registra cada uma como
  consulta e só deixa escrever saídas da ação; o cliente `build.*`
  (`crates/build/src/cliente.rs`) fala pelo canal de `crates/dfexec`,
  compartilhado com as macros;
* **executor** — `packages/build_executor` (`dartforge_build_executor`):
  instancia as fábricas do bootstrap e executa `Builder.build` pelo
  `runBuilder` do próprio `package:build`, com `AssetReader`/`AssetWriter`
  que perguntam tudo ao hospedeiro; o `BuildStep.resolver` é o
  `AnalyzerResolvers` do `build_resolvers` (o mesmo do `build_runner`), que
  lê as bibliotecas pelo mesmo `BuildStep` — então também as leituras do
  analyzer viram consultas da ação;
* **processo** — `crates/build/src/vm.rs` (`ExecutorVm`) roda o executor na
  **VM Dart oficial**, provisoriamente: é opcional e explícito
  (`dartforge build --dart <exe>`, ou `DARTFORGE_BUILD_DART` no
  `dev`/`serve`/`compile-js`). Sem ele o executor é `Indisponivel(motivo)` e
  o motor usa o apoio do `build_runner`. O processo definitivo virá do
  backend nativo auto-hospedado (sem a VM oficial no produto) e fala este
  mesmo protocolo; nada no motor muda quando ele chegar.

### O processo pela VM

O hospedeiro grava em `.dart_tool/dartforge/build/` do projeto (mesma
profundidade do `.dart_tool/build/entrypoint/` do oficial, para que os
imports relativos do plano — `../../../tool/builders.dart` — valham sem
reescrita):

* `dartforge_build_executor/lib/**`: as fontes do executor, embutidas no
  binário do DartForge;
* `package_config.json`: o do projeto, com URIs absolutas, mais o pacote do
  executor. O projeto usa `build_runner`, então `build`, `build_resolvers`,
  `package_config` e `logging` já estão lá, nas versões do lock — o executor
  não traz dependência própria;
* `bootstrap.dart`: os imports das fábricas como o `build.dart` do oficial os
  escreve e o mapa chave → fábrica, das aplicações que podem virar ação
  (builders e pós-processadores; sem os substituídos);
* `bootstrap-<chave>.dill` e `.d`: o kernel de `dart compile kernel
  --depfile`, reaproveitado entre sessões enquanto nenhum arquivo do depfile
  for mais novo que ele. A chave é blake3 da chave do plano (plano, versões
  do lock, versão do motor), das fontes do executor, do bootstrap, do
  `package_config.json` gerado e do caminho do `dart`.

O processo roda com o diretório do pacote raiz como diretório corrente (o
`build_resolvers` guarda o resumo do SDK em `.dart_tool/build_resolvers`,
como no oficial) e `--packages` do arquivo gerado.

## 1. Transporte e enquadramento

* Um processo executor por sessão, iniciado na **primeira** ação que
  precisar dele e reaproveitado até o fim da sessão (quente).
* Canal: stdin/stdout do processo filho (sem socket: nada de porta, nada
  de firewall). stderr é log livre, só para diagnóstico humano.
* Quadro: comprimento `u32` big-endian seguido de UTF-8 JSON — o mesmo
  enquadramento do `message_grouper.dart` do protótipo oficial de macros,
  sem o transporte TCP dele (`docs/MACROS-ARQUITETURA.md`).
* Toda mensagem é um objeto com `"t"` (tipo) e, quando é pedido ou
  resposta, `"id"` (inteiro crescente de quem pede). Os dois lados pedem:
  o hospedeiro pede execução, o executor pede leituras durante ela.

## 2. Handshake

```json
→ {"t":"hello","protocol":"dfexec/1","services":["build"],"engine":"<versão>"}
← {"t":"hello","protocol":"dfexec/1","services":["build"],"executor":"<versão>","abi":"<hash>"}
```

Versão diferente ou serviço ausente: o hospedeiro encerra o processo e o
executor fica `Indisponivel(motivo)` para a sessão.

## 3. Serviço `build.*`

### Hospedeiro → executor

* `build.load {id, script}` — `script` é o equivalente ao
  `.dart_tool/build/entrypoint/build.dart` do oficial: imports das
  fábricas e mapa chave → fábricas. O script é compilado **uma vez** e
  guardado em cache por uma chave versionada (hash do plano, versões do
  lock, versão do DartForge). No processo pela VM a compilação acontece
  antes de o processo nascer (o bootstrap é o próprio script); o
  `build.load` confere que cada fábrica pedida está no bootstrap. O
  kernel é invalidado pelo depfile — as fontes de um builder local
  (`tool/builders.dart`) ou de um pacote por caminho entram nele. Resposta
  `build.loaded {id}` ou `error {id, message}`.
* `build.extensions {id, key, factory, options, isRoot}` — as extensões de
  execução do objeto `Builder` que a fábrica devolve com essas opções
  (`buildExtensions`, que o `expected_outputs.dart` usa). Resposta
  `build.extensions {id, extensions: [[input, [outputs...]], ...]}`, na ordem
  do mapa (o primeiro casamento vale), ou `error {id, message}`. O motor
  pergunta uma vez por plano, para as fases sem gerador nativo verificado, e
  refaz o grafo se alguma diverge do descritor ou do `build.yaml` (as
  fábricas de um builder com várias, extensões que dependem das opções).
* `build.round` (notificação, sem `id` nem resposta) — começo de uma
  atualização do motor: o executor espera a ação em curso, chama
  `Resolvers.reset()` e renova o `ResourceManager`, como o `build_runner`
  faz entre dois builds (`build_impl.dart:94-96`).
* `build.run {id, phase, key, factory, options, isRoot, input,
  allowed_outputs}` — `input` e `allowed_outputs` são AssetIds
  (`"pacote|caminho"`); `options` é o `BuilderOptions.config` já com a
  precedência aplicada. O executor instancia o builder uma vez por
  (chave, fábrica, opções, `isRoot`) e o reusa, como o oficial faz por fase.
* Resposta `build.result {id, outputs: [{asset, bytes_base64}],
  logs: [{level, message}], failed: bool}`. O executor pela VM escreve
  pelo `build.write` durante a ação e devolve `outputs` vazia; `logs` são
  os registros do `log` do builder (`fine`/`info`/`warning`/`severe`, com o
  `print` do builder como aviso, como o `scopeLogAsync` faz) e `failed` é
  verdadeiro quando houve registro severo ou exceção. Uma ação que falha não
  publica saída: o motor cai no apoio com o erro como motivo.
* `build.post_inputs {id, key, factory, options, isRoot}` — as
  `inputExtensions` do `PostProcessBuilder` que a fábrica devolve. Resposta
  `build.post_inputs {id, inputs: [...]}` ou `error`. O motor pergunta uma
  vez por plano e monta as âncoras com elas (`docs/BUILD-MOTOR.md` §6.1).
* `build.post_process {id, phase, key, factory, options, isRoot, input,
  allowed_outputs: []}` — uma âncora: o `runPostProcessBuilder` do
  `package:build` sobre `input`. As escritas vão pelo `build.write`; o
  hospedeiro recusa (`OutputNotAllowed`) o asset que o grafo já tem, como o
  `addAsset` do oficial, e o executor recusa o mesmo asset duas vezes. A
  resposta é o `build.result` com `deleted: [asset]`, as entradas
  marcadas por `deletePrimaryInput`.

### Executor → hospedeiro (durante um `build.run`)

Cada método do `BuildStep` vira um pedido; o hospedeiro registra **cada
um** como `Consulta` da ação (é assim que o motor sabe o que a ação leu):
`findAssets` limita o glob ao pacote da entrada e registra também os
candidatos gerados ainda ausentes; uma nova saída em memória invalida a ação.
`canRead` negativo registra o caminho válido mesmo antes de ele entrar no
grafo, para que a criação posterior do arquivo invalide a ação. Fontes de
pacotes que o grafo não listou (dependências sem fase que gere nelas, como o
`json_annotation`) são legíveis caminho a caminho pelo filtro do pacote
(`sources` dos alvos e visibilidade fora da raiz): é assim que o resolver lê
as bibliotecas importadas, e cada leitura vira consulta. O executor pede cada
asset uma vez por ação; o `digest` é o `md5` do `AssetReader` padrão do
`package:build`, calculado sobre os bytes lidos pelo `read`.

| pedido | `BuildStep` | resposta |
|---|---|---|
| `read {asset}` | `readAsBytes`/`readAsString` | `{bytes_base64}` ou erro `AssetNotFound` |
| `exists {asset}` | `canRead` | `{exists: bool}` |
| `glob {pattern}` | `findAssets` | `{assets: [...]}` ordenados |
| `digest {asset}` | `digest` | `{hex}` |
| `write {asset, bytes_base64}` | `writeAsBytes` | ok, ou erro `OutputNotAllowed` |
| `log {level, message}` | `log.*` | `{}` (o executor pela VM manda os logs no `build.result`) |
| `resolver.* {...}` | `BuildStep.resolver` | Fase 3 (BUILD-RUST.md): servido pelo banco semântico; enquanto não existe, o hospedeiro responde `unavailable` e o analyzer roda dentro do executor |

No canal, os nomes da primeira coluna usam o prefixo `build.` (por exemplo,
`build.read` e `build.response`), e cada pedido recebe o mesmo `id` na resposta.
Um pedido com `AssetId` ou bytes inválidos recebe `build.response {id, error}`;
o canal permanece aberto, para que o builder possa tratar o erro e continuar a
ação. Mensagens sem `id` ou respostas fora de ordem continuam sendo falhas do
protocolo.

Legibilidade das respostas segue o `build_impl.dart:443-463` (§3 de
`docs/BUILD-MOTOR.md`).

## 4. Encerramento

`{"t":"end"}` do hospedeiro; o executor responde `{"t":"end"}` e sai.
Processo que morre no meio de uma ação: a ação falha (cai no apoio com o
motivo) e o `ExecutorVm` fica indisponível pelo resto da sessão. stderr do
processo é herdado (os avisos de depreciação do dart-sass, por exemplo,
aparecem no terminal). Reiniciar o processo no próximo pedido e anexar o
stderr ao diagnóstico da ação ainda não foram feitos.

## 5. Serviço `macro.*`

Definido em [`MACROS-PROTOCOLO.md`](MACROS-PROTOCOLO.md) §5 e implementado
dos dois lados (`crates/macros_host/src/{protocol,executor}.rs` e
`packages/macros/lib/src/executor/service.dart`): o mesmo enquadramento e o
mesmo handshake daqui, com `"services": ["macro"]` e as mensagens
`macro.instantiate`/`macro.execute`/`macro.query`/`macro.response` no
lugar do `build.*`. Um executor que ofereça os dois serviços atende ao motor
de build e às macros no mesmo processo.
