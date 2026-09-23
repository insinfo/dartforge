# `dfexec/1` — protocolo entre o motor e o executor Dart

Contrato do canal entre o motor de build (`crates/build`, hospedeiro) e o
**executor** que roda código Dart em tempo de compilação: builders do
ecossistema (serviço `build.*`) e, depois, macros (serviço `macro.*`).
Um protocolo e um executor para os dois (regra governante, PLANO.md,
item 2). Hoje **não há implementação**: o motor tem só a trait
`ExecutorDart` e a implementação `Indisponivel`. O executor virá do backend
nativo auto-hospedado (sem Node e sem a VM oficial no produto).

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
→ {"t":"ola","protocolo":"dfexec/1","servicos":["build"],"motor":"<versão>"}
← {"t":"ola","protocolo":"dfexec/1","servicos":["build"],"executor":"<versão>","abi":"<hash>"}
```

Versão diferente ou serviço ausente: o hospedeiro encerra o processo e o
executor fica `Indisponivel(motivo)` para a sessão.

## 3. Serviço `build.*`

### Hospedeiro → executor

* `build.carregar {id, script}` — `script` é o equivalente ao
  `.dart_tool/build/entrypoint/build.dart` do oficial: imports das
  fábricas e mapa chave → fábricas. O executor compila **uma vez** e guarda
  em cache por `blake3(fontes + versões do lock + versão do DartForge +
  ABI)`; nunca recompila por ciclo. Resposta `build.carregado {id}` ou
  `erro {id, mensagem}`.
* `build.executar {id, fase, chave, fabrica, opcoes, entrada,
  saidas_permitidas}` — `entrada` e `saidas_permitidas` são AssetIds
  (`"pacote|caminho"`); `opcoes` é o `BuilderOptions.config` já com a
  precedência aplicada, mais `isRoot`.
* Resposta `build.resultado {id, saidas: [{asset, bytes_base64}],
  logs: [{nivel, mensagem}], falhou: bool}`.

### Executor → hospedeiro (durante um `build.executar`)

Cada método do `BuildStep` vira um pedido; o hospedeiro registra **cada
um** como `Consulta` da ação (é assim que o motor sabe o que a ação leu):

| pedido | `BuildStep` | resposta |
|---|---|---|
| `ler {asset}` | `readAsBytes`/`readAsString` | `{bytes_base64}` ou erro `AssetNotFound` |
| `existe {asset}` | `canRead` | `{sim: bool}` |
| `glob {padrao}` | `findAssets` | `{assets: [...]}` ordenados |
| `digest {asset}` | `digest` | `{hex}` |
| `escrever {asset, bytes_base64}` | `writeAsBytes` | ok, ou erro `SaidaNaoPermitida` |
| `log {nivel, mensagem}` | `log.*` | — |
| `resolver.* {...}` | `BuildStep.resolver` | Fase 3 (BUILD-RUST.md): servido pelo banco semântico; enquanto não existe, o hospedeiro responde `indisponivel` e o analyzer roda dentro do executor |

Legibilidade das respostas segue o `build_impl.dart:443-463` (§3 de
`docs/BUILD-MOTOR.md`).

## 4. Encerramento

`{"t":"fim"}` do hospedeiro; o executor responde `{"t":"fim"}` e sai.
Processo que morre no meio de uma ação: a ação falha com o stderr no
diagnóstico, e o próximo pedido inicia um executor novo.

## 5. Serviço `macro.*`

Reservado. O hospedeiro de macros usa o mesmo enquadramento, handshake e
executor, com as fases de macro no lugar do `build.executar`.
