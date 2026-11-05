# Servidor LSP do DartForge (`crates/lsp`, `editors/vscode`)

Servidor da linguagem Dart em Rust, por stdio, com sincronização incremental
e colunas UTF-16 corretas. Expõe o front-end novo (parser completo de Dart
3.6, que aceita 100% do `new_sali`) ao editor, e existe para provar que não
repete o defeito medido no PLANO.md: o LSP do Dart chega a ~6 GB no projeto
de referência e a memória só sobe a cada edição até ser preciso matar o
processo.

## Desenho

```text
                    stdin (bytes)
                       │
                 ┌─────▼──────┐
                 │  leitora   │  thread 1: `ler_mensagem` quadro a quadro
                 └─────┬──────┘  (Content-Length + JSON), enfileira Values
                       │ mpsc
              ┌────────▼────────┐
              │    Servidor     │  thread principal: dono de tudo,
              │  ┌───────────┐  │  passado por `&mut` ao despacho
              │  │ fila      │  │  VecDeque de mensagens em ordem
              │  │ docs      │  │  DocumentStore: 1 entrada por documento
              │  │ cancel.   │  │  ids na fila (teto 4096, limpa ao responder)
              │  │analisador │  │  trait Analisador (parser + verificadores locais)
              │  └───────────┘  │
              └──┬─────────▲────┘
                 │ pedido  │ resultado (uri, versão, diagnósticos)
              ┌──▼─────────┴────┐
              │     tipado      │  thread 3: análise do `dartforge analyze`
              └────────┬────────┘  por pacote (ver "Diagnósticos tipados")
                       │
                       │ respostas e publishDiagnostics,
                       │ uma mensagem inteira por vez sob Mutex
                 ┌─────▼──────┐
                 │   stdout   │  SÓ protocolo. Log vai para stderr.
                 └────────────┘
```

Cópia do desenho do rust-analyzer, não do código (`references/rust-analyzer`):
documentos em memória versionados (`mem_docs.rs` → `DocumentStore`),
operações com cancelamento (`op_queue.rs` → fila + `$/cancelRequest`),
conversão ampla de posições (`lsp/utils.rs` → `utf16.rs`), cliente fino
(`editors/code` → `editors/vscode`).

Cada `bombear` faz, nesta ordem: (1) extrai e aplica **todos** os
`$/cancelRequest` da fila, estejam antes ou depois da requisição alvo —
sem isso o cancelamento seria condição de corrida entre a leitora e o
despacho; (2) processa as notificações líderes (`didOpen`/`didChange`/`didClose`,
cada mudança publica diagnósticos); (3) executa **uma** requisição, ou
responde `RequestCancelled` (-32800) se ela foi cancelada. Requisição em
execução checa cancelamento em pontos seguros (o gancho de teste
`dartforge/dormir` dorme em fatias de 10 ms; a análise real checará entre
documentos, nunca no meio de uma frase do parser).

Por documento, o único estado retido é texto + tabela de linhas + versão; a
chegada da versão N substitui a N−1 no lugar e `didClose` remove. Nenhum
cache sem teto, nenhum histórico de versões, nenhum log em memória; o
conjunto de cancelamentos só guarda ids de requisições ainda na fila e é
limpo ao responder. Nada de `Rc<RefCell<…>>` no estado global.
O analisador esquece a versão de linguagem associada à URI em `didClose` e
recarrega o `package_config.json` na próxima abertura; reabrir um documento
já aberto também invalida esse estado.
O caminho local da URI é decodificado como URL de arquivo, inclusive nomes
Unicode percent-encodados, antes de descobrir o pacote e sua versão.
Uma mudança em `package_config.json`, `pubspec.yaml`, `pubspec.lock`,
`analysis_options.yaml` ou `.packages` anunciada por
`workspace/didChangeWatchedFiles` (o `pub get`) descarta também a versão de
linguagem dos documentos que continuam abertos e os programas retidos, e os
diagnósticos sintáticos deles são publicados de novo.

O despacho roda numa thread com pilha de 1 GiB (espaço de endereço
reservado, não memória usada): o parser, a análise e o formatador recursam
na árvore, e uma cadeia gerada de 30 000 termos `+` estourava a pilha de
8 MiB da thread principal no binário release (5 000 no debug). Estouro de
pilha aborta o processo, e nenhum `catch_unwind` o contém; pânicos comuns
continuam contidos por requisição e por notificação.

A formatação (`formatting`, `rangeFormatting`, `onTypeFormatting`) roda numa
thread própria, também com pilha de 1 GiB (`formatting::start`). O despacho
espera até 200 ms; se ela não terminou, a requisição é respondida depois
(quando a thread acorda o laço principal), e as mensagens seguintes continuam
sendo despachadas. Um `$/cancelRequest` dessa requisição responde
`RequestCancelled` na hora (o resultado da thread é descartado), e um
documento alterado enquanto ela formatava recebe `ContentModified`
(-32801). Sem o gancho de despertar (`Server::on_diagnostics`, o caso dos
testes que usam o `Server` direto) a formatação continua síncrona. Casos
patológicos continuam caros como no `dart format` (uma lista de 5 000
argumentos `a + b` leva ~2,4 s nos dois), mas não travam mais o servidor.
Cadeias binárias longas ficaram lineares (27/10/2026, release, M4): 80 000
termos `1 + 1 + …` levavam 13 s porque cada edição convertia o deslocamento
em posição varrendo a linha (uma só, antes de formatar) desde o início;
agora as edições são convertidas por um cursor sequencial
(`utf16::PositionCursor`), e o conjunto leva ~0,15 s. No divisor de linhas,
`1 * 2 + 1 * 2 + …` copiava o conjunto de regras inteiro a cada tentativa de
ligar uma regra, mesmo as que falhavam (80 000 termos: 1,5 s); com ligação no
lugar e desfazer (`RuleValues::undo`), 0,16 s. Saída idêntica: oráculo do
formatador 45 407/45 407 arquivos e as cadeias geradas (até 10 000 termos,
acima disso o `dart format` estoura a pilha) iguais ao `dart format`.
Testes: `tests/formatting.rs`.

`$/setTrace` (e o `trace` do `initialize`) guarda o nível; acima de `off`,
cada requisição atendida sai também como `$/logTrace` com o tempo gasto
(em `verbose`, o id e o tamanho dos parâmetros). Notificações `$/…`
desconhecidas são ignoradas em silêncio, como a especificação permite (só
com `DARTFORGE_LSP_DEBUG`/`RUST_LOG=debug` aparecem no `stderr`).

## Descoberta do SDK

O servidor procura o SDK do Dart uma vez, na partida
(`dartforge_elements::sdk_discovery::search`, a mesma busca do compilador e do
`dartforge analyze` sem `--sdk-path`). Vale o primeiro candidato com
`libraries.json` e `core/core.dart` (aceita a raiz do SDK ou o `lib/`) que
também carregue (`sdk_discovery::search_with`): um candidato que existe mas
falha ao carregar — `libraries.json` corrompido, `dart:core` ilegível — é
recusado, aparece no log com o motivo (`could not be loaded: …`) e a busca
segue para o próximo. A versão não muda a ordem: uma escolha explícita
(`DARTFORGE_SDK_LIB`, `DART_SDK`…) vale mesmo que exista um SDK 3.6.x mais
abaixo na lista; só gera o aviso descrito adiante.

1. `initializationOptions.sdkPath` do cliente (troca o SDK antes de qualquer
   análise);
2. `DARTFORGE_SDK_LIB` — é por ela que a extensão do VS Code repassa a
   configuração `dartforge.sdkPath` (`editors/vscode/src/server.ts`);
3. o SDK ao lado do executável: o da distribuição do `dartforge pack`
   (`<raiz>/lib/dart-sdk`, com o marcador `dartforge-distribution.json`), ou
   um `dart-sdk/` junto do binário;
4. `DART_SDK`;
5. `FLUTTER_ROOT` (`<flutter>/bin/cache/dart-sdk`);
6. o `dart` do `PATH`, seguindo links simbólicos (Homebrew) e o script do
   Flutter (`<flutter>/bin/dart` → `bin/cache/dart-sdk`);
7. o `flutter` do `PATH`, do mesmo modo;
8. locais de instalação conhecidos: Homebrew (`/opt/homebrew/opt/dart@3.6`,
   `…/dart/libexec`, também `/usr/local` e Linuxbrew), `/usr/lib/dart`
   (pacote Debian), `/opt/dart-sdk`, o snap do Flutter, `~/dart-sdk`,
   `~/flutter`, `~/development/flutter`, `~/fvm/default`; no Windows,
   `C:\tools\dart-sdk` (Chocolatey), `%ProgramFiles%\Dart\dart-sdk`,
   `C:\src\flutter`. O VS Code aberto pelo Dock ou por um lançador gráfico
   muitas vezes não herda o `PATH` do shell; é para esse caso que eles
   existem.

Depois do `initialized`, um `window/logMessage` diz qual SDK foi escolhido,
por qual fonte e com qual versão (arquivo `version`), e lista os candidatos
recusados. Um SDK que não seja 3.6.x é usado, com um aviso no log: os
recursos de linguagem não dependem dele (a versão corrente da ferramenta é
fixa, e as bibliotecas `dart:*` ficam no piso 3.6, `docs/VERSOES-LINGUAGEM.md`),
mas as bibliotecas de outra versão podem dar respostas diferentes das do
servidor do Dart 3.6.2 (hover, completar e navegação funcionaram com o SDK
3.13.5). Sem SDK nenhum, além do log, um único `window/showMessage` de aviso
explica como configurar (`dartforge.sdkPath`, `DARTFORGE_SDK_LIB`, `sdkPath`):
só diagnósticos de sintaxe, formatação e estrutura respondem.

## Sincronização incremental e UTF-16

Capacidade anunciada: `textDocumentSync: 2` (incremental),
`positionEncoding: "utf-16"`. Cada documento guarda `Vec<usize>` de inícios
de linha, recalculada só do ponto editado em diante (`recalc_a_partir`).
Coluna do protocolo (unidades UTF-16) vira offset de bytes na entrada e o
span de bytes do diagnóstico vira posição na saída; quebras `\n`, `\r\n` e
`\r`; saturação sem `panic` fora do documento; coluna no meio de um par
substituto trunca para a borda.

Sem debounce: medido, não chutado. A medição abaixo dá ~0,35 ms por
diagnose (transporte + análise + publicação, média em 2.516 mudanças sobre
o `new_sali`); um debounce de 50 ms só adicionaria latência sem economizar
trabalho relevante. A análise tipada (seção seguinte) também não usa
debounce: roda noutra thread, coalesce as edições que chegam enquanto está
ocupada e para no próximo ponto de cancelamento quando o texto muda.

## Diagnósticos tipados (fluxo contínuo)

Cada `didOpen`/`didChange` produz **duas** publicações para o documento:

1. **Imediata**, no próprio `bombear`: o parser na versão de linguagem do
   arquivo mais os verificadores de `crates/analysis` que olham só o arquivo
   (duplicatas, locais não usados, `external`, aridade de operador…), já
   pela regra de publicação e pelos `// ignore:`. Custa milissegundos.
2. **Tipada**, em segundo plano (`crates/lsp/src/tipado.rs`): a mesma
   análise do `dartforge analyze` — `dartforge_parity::analise::Motor`
   (carga, outline, inferência de `crates/types`, todos os verificadores de
   `crates/analysis`, imports não usados, diretivas sem alvo) seguida de
   `dartforge_parity::publicaveis` (sintaxe sempre; semântica só com
   código em `crates/analysis/verified.txt`; `analysis_options.yaml` e
   `// ignore:`). Nenhuma regra é reescrita no LSP: a publicação tipada é a
   sintaxe do parser mais a semântica publicada do motor. Conferido com o
   exemplo de medição abaixo: nos arquivos medidos, a lista publicada é
   idêntica (código, linha, coluna) à do `Motor` + `diagnosticos_json`,
   que é o caminho do `dartforge analyze`.

Regras do fluxo:

* **Um programa por pacote.** O pedido marca o pacote do documento (a raiz
  com `pubspec.yaml`, como no `analyze`). A análise inclui **todos** os
  documentos abertos daquele pacote (e a biblioteca dona de cada parte
  aberta) num único `Motor::analisar_com`, com o texto aberto valendo mais
  que o disco — inclusive para arquivos só importados. Por isso editar um
  arquivo importado (aberto, sem salvar) republica os diagnósticos de quem o
  importa, direta ou transitivamente, se estiver aberto. Fechar um documento
  reanalisa os outros abertos do pacote com o texto do disco.
* **Versão.** Cada resultado leva a versão do texto analisado. O servidor só
  publica se ela ainda é a vigente; senão descarta
  (`Servidor::tipados_descartados`). Os resultados são drenados depois das
  notificações do mesmo `bombear`, então uma mudança já recebida descarta o
  resultado que ela tornou velho antes de ele chegar ao editor. Documento
  fechado também descarta.
* **Edições rápidas.** Marcar um pacote já marcado não enfileira outra
  análise: uma rajada vira uma análise da última versão. A análise em curso
  consulta, entre as fases do motor (depois da carga, antes dos tipos e
  antes de cada biblioteca na inferência de corpos), se o pacote foi marcado
  de novo; se foi, para e recomeça com o texto novo.
* **Memória.** O trabalhador retém o `Motor` (layout do SDK e nomes das
  bibliotecas) e o texto vigente de cada documento aberto (compartilhado com
  as tarefas por `Arc<str>`, sem cópia; sai no `didClose`). Programa, árvores e
  tabela de tipos vivem uma análise — e, no binário, **num processo filho**
  (`dartforge-lsp --typed-analysis`, ver "Memória com a pasta do workspace
  aberta"): o alocador do processo longo guardaria as páginas da análise e o
  servidor ficaria no pico do maior pacote. `DARTFORGE_LSP_TYPED_IN_PROCESS=1`
  volta à análise na thread (o padrão de testes e de quem embute o
  `Server`). O motor usa o cache do SDK em disco (`SdkCache`, o mesmo do
  `analyze`, em `DARTFORGE_CACHE_DIR` ou `target/dartforge`).
* **Sem SDK** (`AnalisadorSemantico::novo(None)`, ou SDK não descoberto) ou
  com `DARTFORGE_LSP_TYPED=0`, só a publicação imediata existe. Pânico na
  análise tipada é contido: fica a imediata.
* O laço do binário acorda pelo gancho `Servidor::ao_ter_diagnosticos`;
  testes e medições usam `Servidor::aguardar_diagnosticos(limite)`.
* **Incremental.** Desde 2026-10-31 nem toda edição reanalisa o pacote, e as
  análises pequenas rodam num processo residente que guarda as árvores entre
  elas: ver "Edição incremental" logo abaixo.

### Edição incremental (2026-10-31)

Antes, cada tecla num pacote de uma pasta do workspace reanalisava **todos**
os arquivos do pacote num processo filho novo: no `new_sali/frontend` (326
arquivos do pacote, 3.316 bibliotecas no programa) eram ~6 s por edição,
quase tudo nos verificadores de tipos dos 326 arquivos (2,8 s) e na carga
(1,2 s: ler, lexar e analisar sintaticamente as ~30 MiB de fonte das
dependências; medido com `sample` no modo na thread). Agora são duas
camadas, em `crates/lsp/src/typed.rs`:

* **Plano por assinatura de API** (`plan`, `Memo`). O trabalhador guarda,
  por pacote e por arquivo analisado, os diagnósticos publicados, a
  assinatura de API do texto analisado
  (`dartforge_elements::incremental::api_signature`: tokens fora dos
  corpos, as diretivas e a sequência de corpos, a mesma classe **Body** da
  §16.6 da especificação) e os alvos das diretivas. A cada análise:
  - um arquivo mudado com a **mesma** assinatura reanalisa só a sua
    biblioteca (o arquivo, as partes e o dono, pelos `part`/`part of` nos
    dois sentidos — `unused_element` de um privado depende do corpo de
    outra unidade da biblioteca);
  - um arquivo mudado com assinatura **diferente** (ou desconhecido)
    reanalisa quem depende dele pelas diretivas, transitivamente;
  - os demais arquivos a publicar (o mesmo conjunto de antes: abertos e,
    no workspace, os afetados pelas diretivas) saem com o que o memo tem —
    os diagnósticos de um arquivo dependem só do texto da sua biblioteca e
    das declarações que ele vê. Um arquivo fora do memo é analisado; o
    memo cai inteiro quando a configuração muda (`stale`), quando uma
    análise falha e quando o pacote não tem mais nada aberto nem está no
    workspace. Só diagnósticos e alvos de diretivas ficam, nunca programa.
* **Processo de análise residente** (`ResidentChild`,
  `dartforge-lsp --typed-analysis --resident`). As análises pequenas (até
  1.024 arquivos desde 2026-11-01, eram 32; fora a primeira do pacote) vão
  para um filho que vive entre as análises e guarda as unidades analisadas (fonte e árvore) de todo o
  programa, SDK incluído (`dartforge_parity::analysis::Resident`, sobre o
  `AnalyzedUnitCache` do `dartforge dev`); a carga seguinte só reanalisa o
  que mudou: textos abertos que mudaram ou deixaram de estar abertos,
  arquivos com data/tamanho diferentes no disco, os arquivos do lote (cujos
  diagnósticos de sintaxe vêm da análise) e a entrada sintética. Tipos,
  escopos e diagnósticos são refeitos em toda análise, salvo quando só
  corpos do mesmo lote mudaram (desde 2026-11-01 o programa e o *outline*
  ficam; ver "Segunda rodada" abaixo). Protocolo: uma
  mensagem JSON por linha (`Analyze {id, job}`, `Cancel {id}`), uma
  resposta por análise. Cancelar é cooperativo (o filho para no próximo
  ponto de cancelamento do motor); se não parar em 500 ms, o processo é
  encerrado e o próximo começa frio. Limites: `DARTFORGE_LSP_RESIDENT_MIB`
  (padrão 64 MiB de fonte guardada; acima disso o filho esvazia o que
  guarda depois da análise; `0` desliga e volta ao filho por análise) e
  `DARTFORGE_LSP_RESIDENT_IDLE_SECS` (padrão 300 s sem análise: o filho
  termina e devolve a memória). Lotes grandes (a primeira análise, uma
  mudança de API que muitos arquivos veem) seguem num filho próprio, para
  que o pico deles não fique com o residente.

Testes: `tests/typed_incremental.rs` aplica uma sequência de edições (corpo,
API usada por outros arquivos e por um *barrel* com `package:`, corpo de
parte e de dono, erro de tipo, privado novo, fechamento) e, a cada passo,
compara tudo o que o editor mostra com um servidor novo que analisa do zero
os mesmos textos — na thread e com o processo residente — e confere que uma
edição de corpo analisa só a biblioteca (1 ou 2 arquivos).
`Server::typed_analyzed_files` conta os arquivos dados às análises.

Medição: `cargo run --release -p dartforge-lsp --example editing_latency --
--server target/release/dartforge-lsp --root <pacote> --file <arquivo>
--api <arquivo>` dirige o binário real pelo protocolo (abre a pasta,
digita um comando caractere a caractere esperando cada versão, rajada sem
esperar, hover e completar, edição de API e de corpo em outro arquivo) e
mede o *footprint* do servidor e dos filhos. macOS arm64, release, SDK
3.6.2, mesma sessão para os dois binários (`d0cf42b5` e `acba3c82`),
carga da máquina ~8–15; `new_sali/frontend` (326 arquivos do pacote,
3.316 bibliotecas, nenhum diagnóstico publicado) e `limitless_ui` (356
arquivos, 62 publicados):

| | frontend antes | frontend depois | limitless_ui antes | limitless_ui depois |
|---|---:|---:|---:|---:|
| `didChange` → tipados, mediana (35 teclas) | 2.811 ms | 383 ms | 2.455 ms | 175 ms |
| idem, p90 | 4.406 ms | 493 ms | 3.125 ms | 222 ms |
| rajada de 15 teclas / 60 ms → assentado | 2.635 ms | 440 ms | 2.209 ms | 202 ms |
| edição de API em arquivo usado por outros | 3.809 ms | 2.183 ms | 2.187 ms | 887 ms |
| edição de corpo nesse arquivo | 3.642 ms | 474 ms | 2.066 ms | 79 ms |
| primeira análise do workspace | 3.091 ms | 3.169 ms | 4.600 ms | 4.208 ms |
| hover / completar depois das edições (mediana) | 897 / 932 ms | 915 / 903 ms | 8 / 35 ms | 7 / 34 ms |
| servidor: pico / repouso | 478 / 210 MiB | 477 / 234 MiB | 93 / 93 MiB | 103 / 35 MiB |
| filho de análise: pico | 571 MiB | 751 MiB (residente; 720 em repouso) | 284 MiB | 333 MiB (residente; 327 em repouso) |

Numa rodada anterior com a máquina sob carga ~40–70, o antes era 6,0 s por
tecla no frontend (plano por API sozinho: 0,99 s; com o residente: 0,28 s).
O residente fica em ~353 MiB **vivos** entre as análises no frontend,
estável ao longo das edições (`DARTFORGE_LSP_MEMORY=1`); o *footprint* de
~720 MiB inclui as páginas que o alocador guarda das tabelas temporárias de
cada análise. É o custo da latência: `DARTFORGE_LSP_RESIDENT_MIB=0` volta ao
filho por análise (pico de ~570 MiB só durante cada análise), e o residente
termina sozinho depois de 5 min sem análise (o Dart 3.6.2 ficou em
737–1.402 MiB no `new_sali`, seção "Memória"). A edição de API no frontend
ainda custa ~2,2 s porque o arquivo é exportado pelo *barrel* que 203
arquivos importam: o lote passa de 32 e roda num filho próprio, frio.
Hover e completar continuam recarregando o programa da biblioteca dentro do
servidor a cada consulta no frontend (~0,9 s, pico de ~480 MiB no próprio
servidor) porque o programa passa do orçamento da sessão semântica
(`DARTFORGE_LSP_SESSION_MIB`, 8 MiB de fonte): é o que falta.

### DartForge × servidor oficial nos mesmos projetos (2026-10-31)

**Aviso sobre a máquina.** Medido no macOS arm64 (10 núcleos) com seis agentes compilando
em paralelo: média de carga entre 40 e 190 durante as sessões (valor de `uptime` no início
e no fim de cada uma no log de cada sessão; a faixa vai no título de cada tabela). Nessas
condições o tempo de parede mede também a espera por um núcleo, e a espera pesa mais para
quem faz mais trabalho por pedido. Por isso cada linha traz o **tempo de CPU** ao lado: é a
régua mais estável aqui. As razões de parede abaixo de ~3× ou acima de ~0,3× **não são
conclusivas** com essa carga; as diferenças de 10× ou mais e as de CPU são.

Driver: `crates/lsp/examples/editing_latency.rs`, que agora dirige os dois pelo protocolo
(`--server <dartforge-lsp>` ou `--dart <sdk>/bin/dart`, que roda
`dart language-server --protocol=lsp`). Nenhum dos dois recebe `workDoneProgress` do
cliente, então os dois relatam o estado por `$/analyzerStatus`, que é o que "assentado"
quer dizer. A sessão, igual para os dois:

1. `initialize` com o pacote como pasta do workspace e `didOpen` do arquivo: tempo até a
   primeira análise do workspace assentar (o primeiro `isAnalyzing: false` depois de um
   `true` sem outro `true` por 10 s: o Dart relata um `true` curto para o arquivo aberto,
   passa segundos montando os contextos sem relatar nada e só então analisa tudo);
2. um comando digitado caractere a caractere num corpo de método (12 teclas), esperando cada
   versão assentar; uma rajada de 40 teclas sem esperar;
3. edições cujo diagnóstico tem de sair, 3 vezes cada: erro de tipo num corpo
   (`int.parse(0);`), privado novo não usado no fim do mesmo arquivo (`unused_element`) e
   função pública com erro num arquivo que o *barrel* do pacote exporta: tempo até a
   publicação do arquivo trazer o diagnóstico na linha nova, e até assentar (dependentes
   reanalisados);
4. em 5 identificadores do arquivo, `hover`, `completion` (no meio do nome), `definition`,
   `references`, `prepareRename` e `codeAction`: uma vez logo depois de uma edição de corpo
   assentada e duas vezes em cada posição sem edição no meio;
5. todas as edições desfeitas, a última publicação de cada arquivo guardada (para comparar
   os diagnósticos) e a memória de todos os processos (servidor e descendentes:
   *physical footprint* e RSS, amostrados a cada 25 ms).

CPU (`proc_pid_rusage`): numa edição, do envio até assentar, a do servidor, a dos filhos
que terminaram e a dos descendentes vivos (o processo residente); num pedido, a do processo
do servidor (onde os dois respondem). `crates/lsp/oracle/editing_compare.py` roda as sessões
revezando os servidores (a ordem gira a cada rodada; 3 rodadas por projeto) e imprime as
medianas (latência: mediana de todas as amostras das rodadas; partida e memória: mediana das
rodadas) e a concordância dos diagnósticos finais por (arquivo, linha, código).

Projetos (cópias de trabalho, `package_config.json` das originais): `new_sali/frontend`
(Angular, 284 arquivos em `lib/`, 2.835 bibliotecas no programa de um arquivo, 23 MiB de
fonte), `new_sali/backend` (367 arquivos, 306 com diagnósticos) e `limitless_ui` (190 em
`lib/`). Arquivos: `processo_filtros_component.dart`, `usuario_controller.dart` e
`datatable_component.dart` (edições de corpo e privadas); `documento_service.dart`,
`backend_utils.dart` e `alert_component.dart` (exportados pelo *barrel*).

"Antes" é o `22955178` (3 rodadas revezadas com o Dart, sem a coluna de CPU); "depois" é
o binário deste commit (3 rodadas revezadas com o Dart, com CPU). A coluna do Dart é a da
rodada de "depois".

#### frontend (3+3 sessões antes, 3+3 depois; carga 47–100)

| ms (mediana) | DartForge antes | DartForge depois | Dart 3.6.2 | depois/Dart | CPU depois | CPU Dart |
|---|---:|---:|---:|---:|---:|---:|
| partida até a 1ª análise assentar | 14.408 | 10.771 | 8.804 | 1,22× | 3.702 | 4.839 |
| tecla num corpo → assentado | 1.832 | 1.294 | 82 | 16× | 403 | 48 |
| edição de corpo → diagnóstico publicado | 1.495 | 1.449 | 66 | 22× | 417 | 38 |
| edição de API privada → diagnóstico publicado | 2.859 | 1.091 | 56 | 19× | — | — |
| idem → assentado (dependentes) | 2.859 | 3.307 | 137 | 24× | 1.036 | 56 |
| edição de API exportada (*barrel*) → diagnóstico publicado | 10.313 | 983 | 104 | 9,50× | — | — |
| idem → assentado (dependentes) | 10.313 | 9.807 | 302 | 32× | 3.302 | 114 |
| hover, 1º depois de uma edição | 3.498 | 4,9 | 7,1 | 0,69× | 2,6 | 7,4 |
| hover, sem edição | 3.605 | 2,3 | 1,5 | 1,57× | 2,3 | 2,2 |
| completion, 1º depois de uma edição | 6.892 | 2.113 | 224 | 9,42× | 1.016 | 69 |
| completion, sem edição | 5.357 | 277 | 9,2 | 30× | 83 | 5,0 |
| definition, sem edição | 3.366 | 3,2 | 1,4 | 2,17× | 2,9 | 1,4 |
| references, 1º depois de uma edição | 62.527 | 51.136 | 6.452 | 7,93× | 16.392 | 2.773 |
| references, sem edição | 20.961 | 7.226 | 235 | 31× | 2.283 | 107 |
| prepareRename, sem edição | 5.347 | 2,0 | 0,9 | 2,16× | 2,0 | 0,9 |
| codeAction, sem edição | 3.537 | 19 | 2,0 | 9,22× | 16 | 1,7 |
| memória, todos os processos: pico (MiB, *footprint*) | 1.969 | 1.981 | 888 | 2,23× | | |
| memória: média da sessão (MiB) | 1.045 | 1.267 | 628 | 2,02× | | |
| memória: em repouso no fim (MiB) | 654 | 1.522 | 864 | 1,76× | | |
| memória: pico de RSS (MiB) | 1.199 | 1.112 | 715 | 1,56× | | |

#### backend (3+3 sessões antes, 3+3 depois; carga 39–193)

| ms (mediana) | DartForge antes | DartForge depois | Dart 3.6.2 | depois/Dart | CPU depois | CPU Dart |
|---|---:|---:|---:|---:|---:|---:|
| partida até a 1ª análise assentar | 8.468 | 6.577 | 3.551 | 1,85× | 2.270 | 3.353 |
| tecla num corpo → assentado | 372 | 148 | 46 | 3,25× | 112 | 44 |
| edição de corpo → diagnóstico publicado | 339 | 200 | 18 | 11× | 114 | 22 |
| edição de API privada → diagnóstico publicado | 5.798 | 190 | 24 | 7,97× | — | — |
| idem → assentado (dependentes) | 5.798 | 4.245 | 362 | 12× | 1.621 | 156 |
| edição de API exportada (*barrel*) → diagnóstico publicado | 6.605 | 80 | 63 | 1,27× | — | — |
| idem → assentado (dependentes) | 6.606 | 5.022 | 491 | 10× | 1.588 | 178 |
| hover, 1º depois de uma edição | 68 | 7,8 | 7,2 | 1,08× | 5,0 | 3,0 |
| hover, sem edição | 51 | 2,3 | 0,6 | 3,92× | 2,3 | 0,7 |
| completion, 1º depois de uma edição | 23 | 14 | 20 | 0,69× | 8,6 | 7,1 |
| completion, sem edição | 82 | 13 | 8,9 | 1,49× | 12 | 11 |
| definition, sem edição | 54 | 2,9 | 0,9 | 3,41× | 2,8 | 0,9 |
| references, 1º depois de uma edição | 18 | 4,9 | 1,9 | 2,58× | 4,8 | 2,0 |
| references, sem edição | 82 | 4,1 | 1,3 | 3,15× | 4,0 | 1,3 |
| prepareRename, sem edição | 48 | 2,9 | 0,5 | 5,80× | 2,9 | 0,6 |
| codeAction, sem edição | 76 | 8,9 | 1,3 | 6,85× | 8,7 | 1,3 |
| memória, todos os processos: pico (MiB, *footprint*) | 331 | 308 | 359 | 0,86× | | |
| memória: média da sessão (MiB) | 214 | 183 | 315 | 0,58× | | |
| memória: em repouso no fim (MiB) | 224 | 215 | 321 | 0,67× | | |
| memória: pico de RSS (MiB) | 306 | 274 | 407 | 0,67× | | |

#### limitless_ui (3+3 sessões antes, 3+3 depois; carga 39–196)

| ms (mediana) | DartForge antes | DartForge depois | Dart 3.6.2 | depois/Dart | CPU depois | CPU Dart |
|---|---:|---:|---:|---:|---:|---:|
| partida até a 1ª análise assentar | 18.461 | 16.040 | 6.849 | 2,34× | 4.522 | 4.514 |
| tecla num corpo → assentado | 752 | 531 | 232 | 2,29× | 191 | 149 |
| edição de corpo → diagnóstico publicado | 669 | 538 | 188 | 2,87× | 192 | 102 |
| edição de API privada → diagnóstico publicado | 3.176 | 393 | 168 | 2,33× | — | — |
| idem → assentado (dependentes) | 3.176 | 3.065 | 266 | 12× | 1.084 | 138 |
| edição de API exportada (*barrel*) → diagnóstico publicado | 2.803 | 221 | 184 | 1,20× | — | — |
| idem → assentado (dependentes) | 2.803 | 2.772 | 278 | 9,98× | 995 | 149 |
| hover, 1º depois de uma edição | 12 | 1,0 | 6,4 | 0,16× | 0,9 | 3,3 |
| hover, sem edição | 11 | 0,7 | 1,3 | 0,54× | 0,6 | 1,2 |
| completion, 1º depois de uma edição | 45 | 38 | 20 | 1,93× | 35 | 15 |
| completion, sem edição | 312 | 292 | 18 | 16× | 80 | 18 |
| definition, sem edição | 14 | 4,2 | 1,6 | 2,71× | 4,1 | 1,5 |
| references, 1º depois de uma edição | 5.410 | 6,6 | 9,0 | 0,73× | 2,6 | 8,2 |
| references, sem edição | 4.131 | 4,7 | 36 | 0,13× | 4,2 | 23 |
| prepareRename, sem edição | 2.871 | 5,1 | 0,9 | 5,67× | 5,0 | 1,0 |
| codeAction, sem edição | 192 | 70 | 4,5 | 16× | 29 | 4,5 |
| memória, todos os processos: pico (MiB, *footprint*) | 1.322 | 1.129 | 762 | 1,48× | | |
| memória: média da sessão (MiB) | 748 | 480 | 495 | 0,97× | | |
| memória: em repouso no fim (MiB) | 1.039 | 1.058 | 762 | 1,39× | | |
| memória: pico de RSS (MiB) | 714 | 752 | 711 | 1,06× | | |

**Onde ganhamos ou empatamos:** `hover` (frontend 2,3 × 1,5 ms, CPU 2,3 × 2,2; `limitless_ui`
0,7 × 1,3 ms; era 3,6 s e 11 ms), `references` no `limitless_ui` (4,1 s → 4,7 ms; Dart 36 ms),
o diagnóstico do próprio arquivo numa edição de API exportada no backend e no `limitless_ui`
(80 × 63 ms; 221 × 184 ms) e a memória do backend (0,6–0,9× o Dart). **Na mesma ordem, ainda
atrás:** `definition`, `prepareRename` e `references` no backend ficaram em 2–5 ms (eram
0,05–5,3 s), contra 0,5–1,6 ms do Dart.


**Concordância dos diagnósticos no fim da sessão** (por arquivo, linha e código):
`new_sali/frontend` 0 × 0 (nenhum dos dois publica nada); `new_sali/backend` **99,6%** dos
6.704 do Dart também saem no DartForge (6.680 comuns; 48 só nossos: `undefined_class` 22,
`non_type_as_type_argument` 11, `type_test_with_undefined_name` 5…; 24 só do Dart:
`undefined_identifier` 20, `non_abstract_class_inherits_abstract_member` 3);
`limitless_ui` 1 × 24: os nossos estão em `benchmarks/` e `example/`, pastas com
`pubspec.yaml` próprio que o Dart trata como contextos à parte (`uri_does_not_exist`,
`undefined_identifier`), e o único do Dart é um `uri_does_not_exist` nosso noutra linha.
Iguais antes e depois deste commit.

**O que mudou neste commit** (cada item com o efeito medido acima):

* **Bibliotecas de fora do projeto sem corpos nas consultas.** A carga de uma biblioteca
  (`hover`, `definition`, `completion`, `prepareRename`, ações, *signature help*…) lê o SDK e
  os pacotes com `parse_lexed_outline` (`crates/frontend`): o corpo em bloco de cada
  declaração vira um bloco vazio sobre as mesmas chaves (o leitor "dieta" do analisador para
  bibliotecas só ligadas). Ficam assinaturas, listas de inicializadores, inicializadores de
  variáveis e campos (com os literais de função deles) e os corpos `=>`.
  `dartforge_elements::load::load_lenient_outline` decide por unidade. No
  `new_sali/frontend` a biblioteca tem 2.835 unidades e 23 MiB de fonte; a análise sintática
  era 55% da carga.
* **Orçamento da sessão semântica 8 → 32 MiB de fonte**: com os corpos de fora, o programa
  de uma biblioteca do frontend cabe e fica retido; a edição de corpo troca a unidade no
  lugar (o caminho incremental que já existia), em vez de recarregar ~1–4 s a cada consulta.
* **Chave da sessão mais barata**: sem o SDK e o *pub cache* (nada os reescreve no lugar; o
  servidor do Dart também não os vigia) e com a data de modificação de cada pasta do projeto
  em vez de percorrer a árvore para listar os `.dart`. A consulta reaproveitada custava
  9–14 ms de CPU só nessa conferência; agora 0,6–3 ms no total. Abrir, fechar ou mudar no
  disco um arquivo que a carga da biblioteca não leu também não a derruba mais.
* **`prepareRename`, e a identificação do nome de `references` e `implementation`, pela
  carga da biblioteca** (antes: o projeto inteiro, recarregado a cada pedido quando passava
  do orçamento: 2,1–5,6 s no frontend, 2,6–2,9 s no `limitless_ui`). As buscas locais
  (variável local, parâmetro de tipo, prefixo) ficam na biblioteca.
* **Busca de referências estreita quando a carga larga não cabe** no orçamento dela (48 MiB;
  `wide_source_estimate`): entram só os arquivos do projeto e dos pacotes cujo texto tem o
  nome, os demais vêm sem corpos e só as bibliotecas do projeto com o nome são inferidas. Um
  arquivo sem o nome não o referencia nem declara (`computeReferencedNames` só conta
  identificadores), então o resultado é o da carga larga. A carga de uma busca não fica
  retida. `implementation` (transitiva) mantém a carga larga.
* **Diagnósticos em duas fases**: quando o lote de uma edição tem dependentes (mudança de
  API, privada ou exportada), a biblioteca do arquivo editado é analisada e publicada primeiro
  (os "arquivos prioritários" do Dart), e os dependentes depois.
* **O processo residente reaproveita as unidades do SDK** que ele mesmo guardou, em vez de
  decodificá-las do cache do SDK a cada análise (elas já eram guardadas e nunca servidas).

**Onde ainda perdemos, e por quê:**

* **Diagnóstico por tecla: 2–16× o Dart em parede, 1,3–8× em CPU** (frontend 403 × 48 ms
  de CPU por tecla). O processo residente
  guarda as árvores, mas refaz a cada análise o que é do programa inteiro: carga (montar o
  programa com as unidades guardadas), diretivas e *outline* (escopos das 3.316 bibliotecas)
  e as duas passadas de inicializadores — no frontend, com `DARTFORGE_ENGINE_TIMES=1`,
  ~0,25–0,55 s, 0,4–0,8 s e 0,4–0,8 s por análise, contra ~0,01 s dos corpos do lote. O Dart
  reaproveita os resumos ligados das bibliotecas que não mudaram e só ressolve a biblioteca
  editada. Falta o mesmo no motor: guardar *outline*, tipos e inicializadores entre análises
  quando nenhuma assinatura de API mudou (a sessão semântica já faz isso para as consultas,
  `Query::swap_unit`).
* **Mudança de API até assentar: 10–32× em parede, 7–29× em CPU.** Os dependentes de um arquivo exportado pelo
  *barrel* (mais de 32) vão para um filho frio, que lê e analisa tudo de novo; com as duas
  fases o arquivo editado sai antes, mas o total subiu um pouco (uma análise a mais).
* **`references` no frontend: 8–31×** (16 s de CPU na primeira busca depois de uma edição,
  contra 2,8 s do Dart). O Dart responde de um índice construído na análise;
  nós carregamos e inferimos, por busca, todos os arquivos que citam o nome — para nomes
  comuns quase tudo (no frontend a carga larga inteira passa de 48 MiB e não fica retida).
  No `limitless_ui` (carga larga retida) passamos a ganhar. Falta um índice de nomes
  referenciados por arquivo, mantido pelo processo residente.
* **`completion` no frontend e no `limitless_ui`: 9–30×.** Fora de um corpo, ~1 s de CPU. O caminho incremental só vale
  com o cursor dentro de um corpo; numa assinatura ou num membro de classe a consulta
  recarrega a biblioteca.
* **Partida: 1,2–2,3× em parede com CPU igual ou menor.** A primeira análise do DartForge
  gasta menos CPU que a do Dart (frontend 3,7 × 4,8 s; backend 2,3 × 3,4 s; `limitless_ui`
  4,5 × 4,5 s), mas leva mais
  tempo de parede; a diferença não foi investigada (o filho da primeira análise é de uma
  thread só e, com a máquina cheia, espera mais por núcleo).
* **Memória no frontend e no `limitless_ui`: pico 1,5–2,2× o Dart, e o repouso do frontend
  piorou (654 → 1.522 MiB; Dart 864).** O pico é o das cargas de `references` dentro do
  servidor (no frontend, uma busca de nome comum passa de 1 GiB de *footprint*, e o alocador
  do sistema não devolve as páginas, ver "Memória"); somam-se a biblioteca retida pela sessão
  e, no `limitless_ui`, a carga larga retida (1,39× em repouso). No backend, 0,6–0,9×. O
  próximo passo é a busca larga num processo filho, como a análise tipada.

#### Segunda rodada (2026-11-01): o residente reaproveita o programa

Mesmo driver (agora com a CPU de cada pedido incluindo a dos filhos que o servidor espera:
a busca de referências no filho entra na conta), mesmos projetos e arquivos, 3 rodadas
revezadas com o Dart. "31/10" é o binário de `289f373c` (as tabelas acima); "agora" é o
deste commit. Carga da máquina na faixa do título de cada tabela.

O que mudou:

* **O processo residente guarda o programa entre análises** (`World`,
  `crates/parity/src/analysis.rs`). Quando a análise seguinte é do mesmo lote e só corpos
  dos arquivos do lote mudaram (mesma forma e assinatura de API; nada mais em memória nem no
  disco), as unidades editadas trocam de texto no lugar
  (`dartforge_elements::incremental::swap_unit`, o mesmo das consultas) e a análise segue do
  *outline* em diante: carga, escopos, *outline*, limites simples e as duas passadas de
  inicializadores não são refeitos. Os diagnósticos desses passos, que dependem só de
  assinaturas e inicializadores, só mudam de posição: são deslocados (a edição é um trecho
  contínuo; um diagnóstico dentro dele volta ao caminho de sempre), e os de sintaxe da
  unidade vêm da nova leitura. A inferência dos corpos do lote, os verificadores e os
  diagnósticos são refeitos. Uma troca recusada, outro lote, um arquivo mudado no disco ou
  outro texto em memória voltam à carga completa. No frontend, com
  `DARTFORGE_ENGINE_TIMES=1`: carga 0,25–0,55 s → 0,01–0,03 s, *outline* 0,4–0,8 s →
  0,004–0,03 s, inicializadores 0,4–0,8 s → 0.
* **Buscas de referências que não cabem no orçamento da carga larga num processo filho**
  (`dartforge-lsp --reference-search`): a carga vive e morre com o filho e o servidor não
  fica no pico dela.
* **`completion` fora de um corpo** responde com a análise retida do texto real (ali só o
  escopo importa), em vez de recarregar a biblioteca com a sentinela.
* **Lotes grandes no residente**: `DARTFORGE_LSP_RESIDENT_BATCH` (padrão 1.024, era 32).
  Medido no frontend (exportação do *barrel*, ~200 dependentes): os dependentes assentam em
  4,9 s em vez de 9,3 s e o diagnóstico do próprio arquivo em 0,49 s em vez de 1,0 s, com a
  mesma CPU (3,1 s); o pico do residente quase não muda (446 → 465 MiB) e o pico total cai
  (791 → 472 MiB), porque não há um segundo processo frio que lê e analisa tudo de novo.

Testes novos: `resident_reuses_the_program_when_only_bodies_change` (`crates/parity`:
corpo, erro de sintaxe, local não usado, declaração nova, arquivo de fora mudado no disco;
cada passo comparado com uma análise do zero, inclusive o erro de um inicializador depois dos
corpos editados) e `reference_search_in_a_child_process_finds_the_same`
(`tests/editor_session.rs`: a busca no filho, forçada com `DARTFORGE_LSP_WIDE_MIB=0`, acha o
mesmo que a do servidor). O oráculo de paridade nos quatro projetos de sempre (cópias de
`args-2.7.0`, `path-1.9.1`, `string_scanner-1.4.1` e um `app` com erros comuns) tem a tabela
de recursos idêntica antes (`22955178`), em `289f373c` e agora: `references`,
`implementation` e `definition` 100%, *completion* top-5 100% (top-1 99%).

##### frontend (3+3 sessões em 31/10, 3+3 agora; carga 32–91)

| ms (mediana) | DartForge 31/10 | DartForge agora | Dart 3.6.2 | agora/Dart | CPU agora | CPU Dart |
|---|---:|---:|---:|---:|---:|---:|
| partida até a 1ª análise assentar | 10.771 | 10.236 | 5.419 | 1,89× | 3.308 | 3.991 |
| tecla num corpo → assentado | 1.294 | 102 | 58 | 1,77× | 75 | 50 |
| edição de corpo → diagnóstico publicado | 1.449 | 124 | 29 | 4,32× | 81 | 37 |
| edição de API privada → diagnóstico publicado | 1.091 | 986 | 28 | 35× | — | — |
| idem → assentado (dependentes) | 3.307 | 2.629 | 78 | 34× | 956 | 55 |
| edição de API exportada (*barrel*) → diagnóstico publicado | 983 | 692 | 59 | 12× | — | — |
| idem → assentado (dependentes) | 9.807 | 5.947 | 246 | 24× | 3.115 | 108 |
| hover, 1º depois de uma edição | 4,9 | 2,6 | 11 | 0,24× | 2,6 | 7,8 |
| hover, sem edição | 2,3 | 2,4 | 1,4 | 1,71× | 2,3 | 2,3 |
| completion, 1º depois de uma edição | 2.113 | 1.073 | 193 | 5,57× | 1.030 | 55 |
| completion, sem edição | 277 | 163 | 4,7 | 35× | 84 | 4,8 |
| definition, sem edição | 3,2 | 3,3 | 1,0 | 3,30× | 3,3 | 1,0 |
| references, 1º depois de uma edição | 51.136 | 35.652 | 7.861 | 4,54× | 15.472 | 2.475 |
| references, sem edição | 7.226 | 7.255 | 173 | 42× | 2.266 | 107 |
| prepareRename, sem edição | 2,0 | 3,4 | 0,8 | 4,53× | 3,3 | 0,8 |
| codeAction, sem edição | 19 | 20 | 1,8 | 12× | 16 | 1,6 |
| memória, todos os processos: pico (MiB, *footprint*) | 1.981 | 2.096 | 939 | 2,23× | | |
| memória: média da sessão (MiB) | 1.267 | 1.231 | 635 | 1,94× | | |
| memória: em repouso no fim (MiB) | 1.522 | 1.170 | 863 | 1,36× | | |
| memória: pico de RSS (MiB) | 1.112 | 1.323 | 862 | 1,54× | | |

##### backend (3+3 sessões em 31/10, 3+3 agora; carga 39–83)

| ms (mediana) | DartForge 31/10 | DartForge agora | Dart 3.6.2 | agora/Dart | CPU agora | CPU Dart |
|---|---:|---:|---:|---:|---:|---:|
| partida até a 1ª análise assentar | 6.577 | 7.360 | 3.626 | 2,03× | 2.314 | 3.327 |
| tecla num corpo → assentado | 148 | 62 | 38 | 1,61× | 61 | 42 |
| edição de corpo → diagnóstico publicado | 200 | 52 | 17 | 3,05× | 57 | 21 |
| edição de API privada → diagnóstico publicado | 190 | 191 | 22 | 8,52× | — | — |
| idem → assentado (dependentes) | 4.245 | 5.300 | 362 | 15× | 1.643 | 161 |
| edição de API exportada (*barrel*) → diagnóstico publicado | 80 | 106 | 64 | 1,65× | — | — |
| idem → assentado (dependentes) | 5.022 | 5.504 | 382 | 14× | 1.650 | 187 |
| hover, 1º depois de uma edição | 7,8 | 5,1 | 5,9 | 0,86× | 5,1 | 1,6 |
| hover, sem edição | 2,3 | 2,4 | 0,6 | 4,00× | 2,2 | 0,7 |
| completion, 1º depois de uma edição | 14 | 21 | 35 | 0,59× | 14 | 16 |
| completion, sem edição | 13 | 36 | 8,6 | 4,19× | 13 | 8,5 |
| definition, sem edição | 2,9 | 2,9 | 0,8 | 3,62× | 2,8 | 0,8 |
| references, 1º depois de uma edição | 4,9 | 5,5 | 1,9 | 2,89× | 4,6 | 1,9 |
| references, sem edição | 4,1 | 4,0 | 1,3 | 3,12× | 4,0 | 1,4 |
| prepareRename, sem edição | 2,9 | 3,0 | 0,3 | 8,57× | 2,9 | 0,5 |
| codeAction, sem edição | 8,9 | 8,9 | 1,1 | 8,43× | 8,8 | 1,1 |
| memória, todos os processos: pico (MiB, *footprint*) | 308 | 274 | 461 | 0,60× | | |
| memória: média da sessão (MiB) | 183 | 175 | 354 | 0,49× | | |
| memória: em repouso no fim (MiB) | 215 | 274 | 458 | 0,60× | | |
| memória: pico de RSS (MiB) | 274 | 257 | 505 | 0,51× | | |

##### limitless_ui (3+3 sessões em 31/10, 3+3 agora; carga 32–82)

| ms (mediana) | DartForge 31/10 | DartForge agora | Dart 3.6.2 | agora/Dart | CPU agora | CPU Dart |
|---|---:|---:|---:|---:|---:|---:|
| partida até a 1ª análise assentar | 16.040 | 10.627 | 4.394 | 2,42× | 4.492 | 4.124 |
| tecla num corpo → assentado | 531 | 142 | 225 | 0,63× | 112 | 150 |
| edição de corpo → diagnóstico publicado | 538 | 171 | 217 | 0,79× | 124 | 94 |
| edição de API privada → diagnóstico publicado | 393 | 311 | 205 | 1,52× | — | — |
| idem → assentado (dependentes) | 3.065 | 1.798 | 306 | 5,88× | 962 | 152 |
| edição de API exportada (*barrel*) → diagnóstico publicado | 221 | 169 | 204 | 0,83× | — | — |
| idem → assentado (dependentes) | 2.772 | 1.402 | 392 | 3,57× | 846 | 140 |
| hover, 1º depois de uma edição | 1,0 | 1,0 | 4,6 | 0,22× | 0,9 | 2,7 |
| hover, sem edição | 0,7 | 0,7 | 1,1 | 0,64× | 0,7 | 1,0 |
| completion, 1º depois de uma edição | 38 | 34 | 28 | 1,20× | 32 | 17 |
| completion, sem edição | 292 | 107 | 20 | 5,29× | 74 | 19 |
| definition, sem edição | 4,2 | 4,2 | 1,4 | 3,00× | 4,2 | 1,4 |
| references, 1º depois de uma edição | 6,6 | 5,8 | 8,4 | 0,69× | 3,0 | 7,9 |
| references, sem edição | 4,7 | 3,7 | 54 | 0,07× | 3,7 | 26 |
| prepareRename, sem edição | 5,1 | 5,0 | 0,9 | 5,56× | 4,9 | 0,9 |
| codeAction, sem edição | 70 | 30 | 4,0 | 7,28× | 29 | 2,2 |
| memória, todos os processos: pico (MiB, *footprint*) | 1.129 | 1.194 | 764 | 1,56× | | |
| memória: média da sessão (MiB) | 480 | 417 | 494 | 0,85× | | |
| memória: em repouso no fim (MiB) | 1.058 | 1.088 | 764 | 1,43× | | |
| memória: pico de RSS (MiB) | 752 | 1.061 | 729 | 1,46× | | |

**Onde passamos a ganhar ou empatar:** o diagnóstico por tecla no `limitless_ui` (142 × 225
ms; CPU 112 × 150) e a edição de corpo nele (171 × 217 ms); no frontend a tecla foi de 1,3 s
para 102 ms (Dart 58; CPU 75 × 50) e no backend de 148 para 62 ms (Dart 38; CPU 61 × 42);
`hover` logo depois de uma edição (frontend 2,6 × 11 ms); o diagnóstico do próprio arquivo
numa exportação do *barrel* no `limitless_ui` (169 × 204 ms); `references` no
`limitless_ui`; a memória do backend (0,5–0,6×).

**O que ainda perdemos:**

* **Mudança de API, privada ou exportada: dependentes 3,6–34× até assentar e o diagnóstico
  do próprio arquivo até 35× no frontend.** Uma mudança de assinatura recusa a troca e a
  carga é completa; falta o *outline* incremental (refazer só as bibliotecas cuja assinatura
  mudou e as que dependem delas).
* **`references` no frontend: 4,5–42×.** Nomes comuns: a busca carrega e infere todos os
  arquivos que citam o nome; falta o índice de nomes referenciados mantido pelo residente.
  Agora roda num filho, então não deixa mais o servidor no pico.
* **`completion` no frontend: 5,6–35×** (dentro de um corpo, quando a troca com a sentinela
  é recusada, a biblioteca é recarregada: ~1 s de CPU); no `limitless_ui`, 1,2–5,3×.
* **Memória: frontend pico 2,2× e repouso 1,36× (o repouso caiu de 1.522 para 1.170 MiB;
  Dart 863); `limitless_ui` 1,56× e 1,43×.** No frontend, cada recarga da biblioteca no
  servidor (depois de uma mudança de API, e as do `completion` acima) deixa páginas que o
  alocador não devolve; no `limitless_ui` é a carga larga retida. No backend, 0,5–0,6×.
* **Partida: 1,9–2,4× em parede, com CPU igual ou menor que a do Dart.**
* `definition`, `prepareRename` e `codeAction` continuam em 3–30 ms contra 0,3–4 ms.

#### Terceira rodada (2026-11-01): o pacote inteiro no programa do residente

Mesmo driver (agora também imprime quantas localizações cada `references` devolve), mesmos
projetos e arquivos, 3 rodadas revezadas com o Dart (carga 29–81). "01/11 manhã" é a segunda
rodada (`b9cbc526`); "agora" é o deste commit. O oráculo de paridade nos quatro projetos de
sempre continua com a tabela de recursos idêntica (references, implementation e definition
100%, completion top-5 100%).

O que mudou:

* **Os lotes de um pacote compartilham um programa** (`Resident::set_context`,
  `crates/parity/src/analysis.rs`; o trabalhador manda os `.dart` do pacote no `Job`). O
  programa de cada análise do residente importa todos os arquivos do pacote, não só os do
  lote, e o `World` é chaveado pelo conjunto de bibliotecas importadas. Então, depois de uma
  mudança de API em `F`, a segunda fase (os dependentes) começa do programa que a análise de
  `F` acabou de montar: não recarrega escopos, *outline* e inicializadores; só infere os
  corpos dos dependentes. Os diagnósticos de sintaxe de uma unidade que a carga serviu do
  cache (e não leu) são lidos de novo para o lote que os pede (`syntax_known`). Teclas em
  outro arquivo do mesmo pacote também reaproveitam o programa. A primeira fase (o próprio
  `F`) ainda recarrega: uma mudança de assinatura recusa a troca no lugar e o *outline*
  incremental (refazer só as bibliotecas afetadas no modelo de elementos e no de tipos) não
  foi feito.
* **`completion` numa assinatura** (o tipo ou o nome de um campo, o nome de um método):
  quando a troca com a sentinela é recusada por mudar a assinatura
  (`Query::refused == Some(Refusal::Signature)`), responde a análise retida do texto real,
  sem recarregar a biblioteca. No frontend, as duas posições sorteadas que ainda recarregavam
  (~1 s de CPU cada: o tipo de um campo e o nome de outro) caíam nesse caso.
* **Memória do servidor**:
  - os resumos das bibliotecas conhecidas (`known_libraries`, o índice do *Import library*
    e dos não importados do `completion`) e o teste de `part of` (`project::is_part`) leem
    sem corpos;
  - a conversão de posições de arquivos fechados guarda só o último arquivo (as localizações
    vêm agrupadas por arquivo): as referências de `String` no frontend tocam centenas de
    arquivos do SDK e dos pacotes, cujos textos ficavam todos no servidor até o pedido
    seguinte;
  - `malloc_zone_pressure_relief` depois de respostas grandes foi testado e não muda o
    *footprint* (como em 2026-10-13, seção "Memória"); não ficou;
  - com `DARTFORGE_LSP_MEMORY=1` o servidor registra o *heap* vivo depois de cada pedido.

**Referências: mesmos resultados que o Dart.** No frontend, nas 5 posições, o DartForge e o
Dart devolvem exatamente o mesmo número de localizações: `String` 53.624, `bool` 22.978,
`responsavelSelect` 4, `anoExercicioListaProcessos` 28, `_parseIntList` 5. O pico de memória
do servidor nas buscas de `String` e `bool` vem de montar essa resposta (as localizações como
valores JSON: ~145 MiB vivos para as 53.624), não de carga. O índice de nomes referenciados
pedido para esta rodada não foi feito: um índice de nomes (sem resolução) escolheria quase os
mesmos arquivos que o filtro textual atual, e o que custa é inferir os corpos de todos os que
citam o nome; ganhar aqui pede um índice de referências resolvidas mantido pelo residente
(identidade de elemento estável entre processos e os corpos dos pacotes, como o do Dart).

Testes novos: `resident_context_shares_the_program_between_batches` (`crates/parity`: a API de
`b.dart` muda, os dependentes `a.dart` e `c.dart` são analisados a partir do programa de
`b.dart` e publicam o mesmo que uma análise do zero, inclusive o erro de sintaxe de `c.dart`
que aquela carga não leu); em `tests/incremental.rs`, posições em assinaturas (tipo e nome de
campo, nome de método) comparadas a cada passo com o servidor sem o caminho incremental.

##### frontend (3+3 sessões na manhã, 3+3 agora; carga 29–81)

| ms (mediana) | DartForge 01/11 manhã | DartForge agora | Dart 3.6.2 | agora/Dart | CPU agora | CPU Dart |
|---|---:|---:|---:|---:|---:|---:|
| partida até a 1ª análise assentar | 10.236 | 8.031 | 4.163 | 1,93× | 3.347 | 4.213 |
| tecla num corpo → assentado | 102 | 83 | 40 | 2,07× | 82 | 44 |
| edição de corpo → diagnóstico publicado | 124 | 109 | 26 | 4,23× | 84 | 34 |
| edição de API privada → diagnóstico publicado | 986 | 1.062 | 26 | 41× | — | — |
| idem → assentado (dependentes) | 2.629 | 1.692 | 54 | 31× | 717 | 60 |
| edição de API exportada (*barrel*) → diagnóstico publicado | 692 | 1.088 | 37 | 29× | — | — |
| idem → assentado (dependentes) | 5.947 | 4.644 | 119 | 39× | 2.179 | 103 |
| hover, 1º depois de uma edição | 2,6 | 2,8 | 8,6 | 0,33× | 2,6 | 8,0 |
| hover, sem edição | 2,4 | 2,4 | 1,2 | 2,00× | 2,4 | 2,1 |
| completion, 1º depois de uma edição | 1.073 | 24 | 77 | 0,31× | 24 | 61 |
| completion, sem edição | 163 | 26 | 5,7 | 4,54× | 14 | 4,6 |
| definition, sem edição | 3,3 | 3,3 | 1,3 | 2,54× | 3,2 | 1,3 |
| references, 1º depois de uma edição | 35.652 | 19.187 | 3.840 | 5,00× | 14.845 | 2.427 |
| references, sem edição | 7.255 | 4.785 | 110 | 44× | 2.046 | 65 |
| prepareRename, sem edição | 3,4 | 3,3 | 0,7 | 4,71× | 3,3 | 0,7 |
| codeAction, sem edição | 20 | 16 | 2,5 | 6,47× | 16 | 2,6 |
| memória, todos os processos: pico (MiB, *footprint*) | 2.096 | 1.857 | 937 | 1,98× | | |
| memória: média da sessão (MiB) | 1.231 | 1.106 | 601 | 1,84× | | |
| memória: em repouso no fim (MiB) | 1.170 | 1.066 | 866 | 1,23× | | |
| memória: pico de RSS (MiB) | 1.323 | 1.423 | 972 | 1,46× | | |

##### backend (3+3 sessões na manhã, 3+3 agora; carga 29–81)

| ms (mediana) | DartForge 01/11 manhã | DartForge agora | Dart 3.6.2 | agora/Dart | CPU agora | CPU Dart |
|---|---:|---:|---:|---:|---:|---:|
| partida até a 1ª análise assentar | 7.360 | 3.822 | 3.166 | 1,21× | 2.178 | 3.193 |
| tecla num corpo → assentado | 62 | 73 | 38 | 1,92× | 70 | 46 |
| edição de corpo → diagnóstico publicado | 52 | 49 | 21 | 2,35× | 51 | 23 |
| edição de API privada → diagnóstico publicado | 191 | 144 | 26 | 5,62× | — | — |
| idem → assentado (dependentes) | 5.300 | 2.526 | 375 | 6,73× | 1.422 | 166 |
| edição de API exportada (*barrel*) → diagnóstico publicado | 106 | 149 | 66 | 2,27× | — | — |
| idem → assentado (dependentes) | 5.504 | 2.918 | 370 | 7,88× | 1.376 | 188 |
| hover, 1º depois de uma edição | 5,1 | 6,3 | 2,4 | 2,62× | 5,9 | 1,6 |
| hover, sem edição | 2,4 | 2,3 | 0,8 | 3,07× | 2,2 | 0,8 |
| completion, 1º depois de uma edição | 21 | 12 | 47 | 0,26× | 8,4 | 7,4 |
| completion, sem edição | 36 | 16 | 12 | 1,28× | 11 | 9,1 |
| definition, sem edição | 2,9 | 2,8 | 0,8 | 3,50× | 2,7 | 0,8 |
| references, 1º depois de uma edição | 5,5 | 4,3 | 1,3 | 3,31× | 4,3 | 1,4 |
| references, sem edição | 4,0 | 4,2 | 1,3 | 3,23× | 4,0 | 1,4 |
| prepareRename, sem edição | 3,0 | 3,0 | 0,4 | 7,50× | 2,9 | 0,4 |
| codeAction, sem edição | 8,9 | 9,0 | 1,1 | 8,18× | 8,8 | 1,2 |
| memória, todos os processos: pico (MiB, *footprint*) | 274 | 269 | 363 | 0,74× | | |
| memória: média da sessão (MiB) | 175 | 160 | 310 | 0,52× | | |
| memória: em repouso no fim (MiB) | 274 | 269 | 320 | 0,84× | | |
| memória: pico de RSS (MiB) | 257 | 303 | 424 | 0,71× | | |

##### limitless_ui (3+3 sessões na manhã, 3+3 agora; carga 31–81)

| ms (mediana) | DartForge 01/11 manhã | DartForge agora | Dart 3.6.2 | agora/Dart | CPU agora | CPU Dart |
|---|---:|---:|---:|---:|---:|---:|
| partida até a 1ª análise assentar | 10.627 | 9.229 | 4.829 | 1,91× | 4.370 | 4.715 |
| tecla num corpo → assentado | 142 | 197 | 218 | 0,91× | 122 | 152 |
| edição de corpo → diagnóstico publicado | 171 | 240 | 161 | 1,50× | 135 | 92 |
| edição de API privada → diagnóstico publicado | 311 | 689 | 125 | 5,52× | — | — |
| idem → assentado (dependentes) | 1.798 | 2.371 | 228 | 10× | 889 | 145 |
| edição de API exportada (*barrel*) → diagnóstico publicado | 169 | 552 | 175 | 3,15× | — | — |
| idem → assentado (dependentes) | 1.402 | 1.989 | 279 | 7,14× | 798 | 145 |
| hover, 1º depois de uma edição | 1,0 | 1,0 | 2,9 | 0,34× | 0,9 | 2,9 |
| hover, sem edição | 0,7 | 0,7 | 1,0 | 0,70× | 0,7 | 1,2 |
| completion, 1º depois de uma edição | 34 | 31 | 37 | 0,84× | 31 | 20 |
| completion, sem edição | 107 | 116 | 14 | 8,03× | 49 | 20 |
| definition, sem edição | 4,2 | 4,2 | 1,4 | 3,00× | 4,2 | 1,4 |
| references, 1º depois de uma edição | 5,8 | 3,2 | 12 | 0,28× | 2,8 | 8,7 |
| references, sem edição | 3,7 | 3,4 | 23 | 0,15× | 3,3 | 26 |
| prepareRename, sem edição | 5,0 | 5,1 | 1,4 | 3,78× | 5,0 | 0,9 |
| codeAction, sem edição | 30 | 55 | 4,7 | 12× | 30 | 4,5 |
| memória, todos os processos: pico (MiB, *footprint*) | 1.194 | 1.104 | 738 | 1,50× | | |
| memória: média da sessão (MiB) | 417 | 506 | 484 | 1,04× | | |
| memória: em repouso no fim (MiB) | 1.088 | 1.030 | 734 | 1,40× | | |
| memória: pico de RSS (MiB) | 1.061 | 1.077 | 823 | 1,31× | | |

**Onde passamos a ganhar ou empatar nesta rodada:** `completion` logo depois de uma edição no
frontend (24 × 77 ms; era 1,07 s) e no backend (12 × 47 ms); `completion` sem edição no
frontend caiu de 163 para 26 ms (Dart 5,7). Os dependentes de uma mudança de API caíram à
metade no backend (5,3–5,5 s → 2,5–2,9 s) e em ~35% no frontend (privada 2,6 → 1,7 s,
exportada 5,9 → 4,6 s; CPU 3,1 → 2,2 s). A memória do frontend em repouso caiu de 1.170 para
1.066 MiB (Dart 866). A partida no backend ficou em 3,8 × 3,2 s.

**O que ainda perdemos:**

* **Mudança de API: o diagnóstico do próprio arquivo (frontend ~1,1 s × 26–37 ms) e os
  dependentes (frontend 31–39×, backend 7–8×, `limitless_ui` 7–10×).** A primeira fase ainda
  recarrega o programa (o *outline* incremental não foi feito). No `limitless_ui` o
  programa do pacote inteiro é maior que o de um arquivo (os testes importam o `package:test`
  e o resto), e a primeira fase e os dependentes ficaram mais lentos que na manhã (311 → 689 ms
  e 1,8 → 2,4 s): é o custo de o programa servir a todos os lotes. Medido à parte no backend,
  o contexto custava ~20 ms de CPU por tecla lendo e testando os arquivos do pacote a cada
  análise; com o teste de `part of` guardado por data e tamanho, ficou em ~5 ms (71 × 67 ms).
* **`references` no frontend: 5–44×,** com os mesmos resultados do Dart.
* **Memória: frontend pico 1,98× e repouso 1,23×; `limitless_ui` 1,50× e 1,40×.** No
  frontend, ~244 MiB vivos são a biblioteca retida pela sessão, e o pico vem das respostas
  de `references` dos tipos do SDK (montadas como JSON) e do residente (~365 MiB vivos com o
  pacote inteiro). No `limitless_ui`, a carga larga retida.
* **Tecla → diagnósticos: frontend 2,1× (CPU 82 × 44 ms), backend 1,9× (70 × 46);** no
  `limitless_ui` empatamos (197 × 218 ms; CPU 122 × 152).
* `definition`, `prepareRename` e `codeAction` continuam em 3–55 ms contra 0,4–4,7 ms.

#### Quarta rodada (2026-11-01): o pacote inteiro só onde paga, respostas grandes como texto

Mesmo driver, projetos e arquivos; 3 rodadas revezando três servidores: o desta rodada
("agora"), o da terceira rodada ("r3", `ddf83232`) e o Dart. A máquina estava muito carregada
por outros agentes (carga 33–87 nas sessões, 37–149 no `uptime` durante a rodada): os tempos
de relógio de pedidos longos variam muito entre sessões; as colunas de CPU e as razões entre
servidores medidos na mesma rodada são o que vale. O oráculo de paridade nos quatro projetos
de sempre continua com a tabela de recursos idêntica (references, implementation e definition
100%, completion top-5 100%).

O que mudou:

* **O programa do pacote inteiro só onde paga** (`Resident::context_pays`,
  `crates/parity/src/analysis.rs`). A terceira rodada carregava o pacote inteiro em toda
  análise do residente; no `limitless_ui` (os testes puxam o `package:test`) isso deixou o
  diagnóstico do próprio arquivo depois de uma mudança de API 2–3× mais lento. Agora o
  residente guarda, por pacote, o tamanho do maior programa carregado (o do pacote inteiro,
  quando algum lote o cobre) e o do maior programa próprio de um lote pequeno (até 4
  arquivos: o documento editado e suas partes), medido nas importações e exportações
  transitivas desse lote mesmo quando o programa tem o pacote inteiro. O pacote inteiro entra
  quando é no máximo 1,5× o do arquivo editado. Pelas importações, frontend 21,5/17,0 MB
  (1,26×) e backend 5,5/4,3 MB (1,28×) ficam com o pacote inteiro; `limitless_ui` 10,4/4,8 MB
  (2,2×) não. A decisão é do pacote, não do lote, para que os lotes de um mesmo trabalho
  compartilhem o programa. `DARTFORGE_RESIDENT_CONTEXT=always|never` força um lado (para
  medir); `DARTFORGE_ENGINE_TIMES=1` marca a carga com o pacote inteiro.
* **Respostas de `references` com milhares de localizações saem como texto JSON**
  (`locations_json` em `crates/lsp/src/server.rs`, `transport::raw_result`). A partir de
  2.048 localizações, o servidor escreve o vetor direto num `String` (cada URI escapado uma vez
  por arquivo), em vez de montar uma árvore de valores JSON (~2,7 KB por localização: ~145 MiB
  vivos para as 53.624 de `String` no frontend; o texto tem ~130 bytes por localização). O
  quadro sai com as mesmas chaves, na mesma ordem e com os mesmos bytes que o `serde_json`
  escreveria para os valores. No frontend, o pico do processo do servidor caiu de 743 para
  357 MiB, e o pico somando todos os processos de 1.988 para 1.623 MiB.

Medido à parte (2 rodadas revezadas, `--positions 0`, mesmo binário com o contexto
forçado):

| ms (mediana) | `limitless_ui`: automático | nunca | sempre | backend: automático | nunca | sempre |
|---|---:|---:|---:|---:|---:|---:|
| API privada → diagnóstico do arquivo | 448 | 524 | 984 | 214 | 249 | 277 |
| API privada → assentado | 2.452 | 2.943 | 3.176 | 4.051 | 5.573 | 4.488 |
| API privada: CPU | 1.002 | 907 | 903 | 1.451 | 1.632 | 1.451 |
| API exportada → diagnóstico do arquivo | 249 | 306 | 708 | 258 | 92 | 305 |
| API exportada → assentado | 2.194 | 2.775 | 2.652 | 3.974 | 4.953 | 4.194 |
| API exportada: CPU | 900 | 816 | 786 | 1.405 | 1.588 | 1.437 |
| cargas com o pacote inteiro na sessão | 1 (a da partida) | 0 | 13 | 13 | 0 | 13 |

##### frontend (3 sessões de cada servidor; carga 37–86)

| ms (mediana) | r3 | agora | Dart | agora/Dart | CPU r3 | CPU agora | CPU Dart |
|---|---:|---:|---:|---:|---:|---:|---:|
| tecla num corpo → assentado | 236 | 219 | 83 | 2,63× | 85 | 83 | 47 |
| edição de corpo → diagnóstico | 257 | 177 | 55 | 3,21× | 80 | 79 | 42 |
| API privada → diagnóstico do arquivo | 1.935 | 1.413 | 89 | 15,8× | | | |
| API privada → assentado | 3.161 | 2.375 | 139 | 17,1× | 766 | 735 | 44 |
| API exportada → diagnóstico do arquivo | 1.689 | 1.334 | 84 | 15,8× | | | |
| API exportada → assentado | 8.731 | 7.409 | 338 | 21,9× | 2.199 | 2.299 | 111 |
| `completion`, 1º depois de edição | 27 | 24 | 191 | 0,13× | 26 | 24 | 54 |
| `references`, 1º depois de edição | 27.642 | 48.758 | 10.113 | 4,82× | 14.598 | 15.534 | 2.457 |
| `references`, sem edição | 9.626 | 9.553 | 344 | 27,8× | 2.102 | 2.157 | 174 |
| partida → primeira análise assentada | 12.540 | 9.976 | 11.003 | 0,91× | 3.379 | 3.453 | 4.436 |
| memória, todos os processos: pico (MiB) | 1.988 | **1.623** | 867 | 1,87× | | | |
| memória: média da sessão (MiB) | 1.165 | 1.038 | 622 | 1,67× | | | |
| memória: em repouso no fim (MiB) | 1.024 | **736** | 781 | **0,94×** | | | |
| memória: pico de RSS (MiB) | 1.359 | **775** | 735 | 1,05× | | | |

`references` depois de edição: a mediana das três primeiras posições (`bool`, `String` e um
campo) é a de `bool` ou `String`, cujo relógio depende da carga do minuto; a CPU ficou igual
(14,6 × 15,5 s).

##### backend (3 sessões de cada servidor; carga 34–87)

| ms (mediana) | r3 | agora | Dart | agora/Dart | CPU r3 | CPU agora | CPU Dart |
|---|---:|---:|---:|---:|---:|---:|---:|
| tecla num corpo → assentado | 72 | 72 | 84 | 0,85× | 64 | 66 | 42 |
| API privada → diagnóstico do arquivo | 454 | 336 | 75 | 4,49× | | | |
| API privada → assentado | 6.686 | 4.932 | 606 | 8,13× | 1.405 | 1.396 | 158 |
| API exportada → diagnóstico do arquivo | 346 | 469 | 122 | 3,86× | | | |
| API exportada → assentado | 6.052 | 6.940 | 598 | 11,6× | 1.405 | 1.466 | 178 |
| `references`, sem edição | 4,5 | 2,1 | 1,3 | 1,65× | 4,3 | 2,0 | 1,4 |
| memória, todos os processos: pico (MiB) | 284 | 281 | 485 | 0,58× | | | |
| memória: em repouso no fim (MiB) | 284 | 281 | 483 | 0,58× | | | |

Diagnósticos finais 99,6% iguais aos do Dart (os mesmos 48 a mais e 24 a menos de antes).

##### limitless_ui (3 sessões de cada servidor; carga 33–87)

| ms (mediana) | r3 | agora | Dart | agora/Dart | CPU r3 | CPU agora | CPU Dart |
|---|---:|---:|---:|---:|---:|---:|---:|
| tecla num corpo → assentado | 456 | 344 | 370 | 0,93× | 129 | 126 | 155 |
| API privada → diagnóstico do arquivo | 998 | **646** | 263 | 2,45× | | | |
| API privada → assentado | 3.062 | 3.638 | 453 | 8,0× | 902 | 951 | 158 |
| API exportada → diagnóstico do arquivo | 746 | **327** | 247 | 1,33× | | | |
| API exportada → assentado | 2.905 | 3.177 | 425 | 7,5× | 799 | 852 | 143 |
| `references`, 1º depois de edição | 5,5 | 5,3 | 14 | 0,37× | 2,8 | 2,8 | 8,0 |
| partida → primeira análise assentada | 14.930 | 17.458 | 7.481 | 2,33× | 4.407 | 4.509 | 4.000 |
| memória, todos os processos: pico (MiB) | 1.194 | 1.027 | 771 | 1,33× | | | |
| memória: em repouso no fim (MiB) | 1.065 | 995 | 754 | 1,32× | | | |
| memória: pico de RSS (MiB) | 805 | 616 | 660 | 0,93× | | | |

No `limitless_ui`, o diagnóstico do próprio arquivo depois de uma mudança de API voltou a
ficar perto do Dart (exportada 327 × 247 ms, era 746; privada 646, era 998). O assentado dos
dependentes ficou dentro do ruído desta máquina (3,2–3,6 × 2,9–3,1 s nesta rodada; 2,2–2,5 ×
2,7–3,2 s na medição à parte), com ~5% a mais de CPU: sem o pacote inteiro, os dependentes
recarregam o programa deles.

**O que não foi feito nesta rodada, e por quê:**

* **(1) *Outline* incremental.** Com o pacote inteiro só onde paga, o diagnóstico do próprio
  arquivo depois de uma mudança de API no `limitless_ui` caiu para 0,3–0,6 s sem ele. O que
  sobra na primeira fase é a carga (no `limitless_ui`, mediana 0,31 s: o modelo de elementos
  do programa a partir do cache de unidades) e o *outline* de tipos (0,15 s), medidos com
  `DARTFORGE_ENGINE_TIMES=1`. No frontend é ~0,7 s de CPU. Refazer só as bibliotecas
  afetadas pede identidade estável de elementos em `crates/elements` (hoje `ClassId`,
  `FunctionElementId` e os demais são índices densos nos vetores do `Program`, refeitos a cada
  carga) e um `OutlineTypes` por biblioteca em
  `crates/types`, com as bibliotecas que dependem da editada (por importação ou
  exportação, transitivamente) invalidadas juntas. É uma mudança nos dois modelos, não no
  residente.
* **(2) Índice de referências resolvidas no residente.** O que custa no frontend são as
  referências de `bool` e `String` (22.978 e 53.624 localizações, as mesmas do Dart): cada
  busca infere os corpos de todos os arquivos que citam o nome (~15 s de CPU no processo de
  busca). O residente infere esses corpos ao analisar o pacote, mas usa o motor de
  `crates/parity`, enquanto `references` usa a identidade de `crates/lsp/src/project.rs`.
  Guardar as referências resolvidas pede registrar o alvo de cada identificador em todos os
  pontos de resolução do verificador (nomes simples e prefixados, acessos a membro,
  construtores, argumentos nomeados, tipos, padrões, combinadores) com uma identidade de
  declaração comum aos dois motores. Um índice parcial quebraria o oráculo (references 100%).
* **(3) Memória do residente e do processo de busca.** O pico dos descendentes no frontend
  continua em ~870 MiB (o programa do pacote inteiro e o processo de busca das referências de
  `String`).

**O que ainda perdemos:** mudança de API (frontend 16–22×, backend 4–12×, `limitless_ui`
1,3–8×), `references` dos tipos do SDK no frontend (5–28×), tecla no frontend (2,6×; CPU 83 ×
47 ms), partida no backend e no `limitless_ui` (2,3×), `definition`/`prepareRename`/
`codeAction` (2–55 ms contra 0,6–4 ms) e o pico de memória do frontend (1,87×; em repouso
ganhamos, 736 × 781 MiB).

Testes novos: `resident_skips_the_context_when_the_package_is_much_larger` (`crates/parity`:
depois de uma análise do pacote inteiro, o lote de um arquivo cujo programa é bem menor não
carrega o arquivo grande que ninguém importa e publica o mesmo que uma análise do zero);
`locations_json_matches_the_values` (`crates/lsp/src/server.rs`: o texto é o que o
`serde_json` escreve para os mesmos valores, inclusive URI com aspas e não ASCII) e
`raw_result_is_written_as_the_result` (`crates/lsp/src/transport.rs`: o quadro é byte a byte
o dos valores).

#### Projeto: reanalisar só o que mudou e índice de referências (2026-11-01)

Plano da prioridade do `ESTADO.md` ("LSP mais rápido e mais leve que o servidor oficial"), em
passos que mantêm tudo funcionando. Cada passo tem teste contra uma análise do zero.

**Onde está o custo.** Medido com `DARTFORGE_ENGINE_TIMES=1` no frontend, com a máquina
calma (carga 18–31). Depois de uma mudança de API, a análise do próprio arquivo leva ~0,4 s
(Dart: 84–89 ms):

| Fase | Tempo |
|---|---:|
| carga com o pacote inteiro: modelo de elementos de ~2.460 bibliotecas | 0,14 s |
| *outline* de tipos, `CoreTypes` e `simple_bounds` do programa inteiro | 0,12 s |
| passes dos inicializadores | 0,10 s |

Das ~2.460 bibliotecas do programa, só ~112 são do pacote. O resto (SDK e dependências)
não muda numa sessão de edição e não importa nada do pacote.

**Passo 1 — prefixo congelado em `crates/elements`.**
- Antes do `build_outline`, as bibliotecas e unidades são permutadas para que as
  *congeladas* (fora das raízes editáveis: SDK, `pub cache`, pacotes por caminho fora do
  projeto) venham primeiro, na ordem relativa de antes. Esse prefixo é fechado por
  dependência.
- `Program::checkpoint()` dá o comprimento de cada arena no fim do prefixo:
  - unidades, bibliotecas, classes, extensões, *typedefs*, funções e variáveis.
- `Program::truncate(checkpoint)` descarta o resto. Um carregamento sobre o prefixo reaproveita
  os `LibraryId` do prefixo pelo URI e não os relê. `build_outline_from(checkpoint)` só cria
  elementos, `exported`, escopos e supertipos do sufixo.
- Os IDs do prefixo sobrevivem às recargas: é a identidade estável de tudo o que não é do
  pacote. Os IDs do pacote são uma visão densa por carga. Uma diretiva nova para fora do
  prefixo recarrega tudo.
- Teste: programa recarregado sobre o prefixo, despejado de forma canônica (URI da biblioteca
  e caminho da declaração), igual ao do zero.

**Passo 2 — *outline* de tipos por faixa em `crates/types`.**
- `OutlineResolver` a partir de um `OutlineTypes` do prefixo:
  - reaproveita os parâmetros de tipo, a hierarquia, os tipos escritos e os tipos inferidos
    das classes, funções e variáveis do prefixo;
  - resolve só os elementos do sufixo.
- A `TypeTable` fica com o residente e cresce. O lixo do sufixo antigo é descartado por uma
  recarga total quando passar de um limite.
- Teste: os diagnósticos e os tipos de cada elemento do sufixo são iguais aos de uma
  resolução do zero.

**Passo 3 — o residente usa os passos 1 e 2 numa mudança de API** (`crates/parity`):
- trunca o programa no ponto de controle, recarrega o pacote e refaz o *outline* dele;
- roda os passes de inicializadores só do sufixo;
- mantém `CoreTypes`, `simple_bounds` e os inicializadores inferidos do prefixo.

Teste: `analyze_resident` igual a `analyze_with` depois de mudanças de API privadas,
exportadas e de hierarquia.

**Passo 4 — dentro do pacote.**
- As bibliotecas do pacote ficam em ordem topológica: componentes fortemente conexos,
  dependências primeiro, e as editadas por último.
- O ponto de truncamento é a primeira biblioteca afetada. As que vêm antes mantêm IDs, tipos
  e corpos inferidos.
- Para que uma edição numa biblioteca muito importada não invalide quem só cita os tipos dela,
  a identidade passa a ser (URI da biblioteca, caminho da declaração), com os índices densos
  como visão. Só refaz o *outline* de quem herda ou infere tipos do que mudou: isso vem da
  assinatura de API por declaração, que `crates/elements/src/incremental.rs` já calcula para o
  arquivo inteiro.

**Passo 5 — índice de referências resolvidas.**
- A inferência de corpos (`crates/types`) registra, para cada nome resolvido, a faixa e o
  alvo, com a identidade (URI da biblioteca, caminho da declaração).
- `crates/lsp/src/project.rs` converte a sua identidade de declaração para essa.
- O residente guarda o índice por biblioteca e o refaz junto com cada análise. As bibliotecas
  do prefixo (SDK e dependências, quando o resultado do Dart as inclui) são indexadas uma
  vez por sessão, em segundo plano.
- `references` consulta o índice só quando ele cobre todos os arquivos que a busca atual
  leria. Senão, cai na busca de hoje. Nunca responde parcial.
- Teste: o oráculo (`references` 100%) e a igualdade do índice com a busca atual nos três
  projetos.

#### Quinta rodada (2026-11-01): prefixo congelado, passos 1–3 do projeto

Os passos 1 a 3 do projeto acima estão feitos (`a91e9b4c`). Os passos 4 e 5 ficam para a
próxima rodada.

* **`crates/elements`.**
  - `load_lenient_frozen` põe as bibliotecas congeladas na frente de todas as arenas:
    SDK e o que está fora da raiz do pacote, fechadas por importação e exportação
    (`frozen::frozen_first`). `Program::frozen` marca onde o prefixo termina.
  - `load_lenient_onto` corta o programa nesse ponto (`Program::truncate`) e carrega o
    pacote sobre o prefixo, sem reler as bibliotecas dele.
  - `build_outline_from` só monta elementos, `exported`, escopos e supertipos depois do
    ponto de controle.
  - O caminho dos compiladores (`Layout::Plain`) não muda.
* **`crates/types`.** `OutlineResolver::onto` e `resolve_outline_onto` resolvem só os
  elementos depois do prefixo, sobre o `OutlineTypes` do prefixo (`OutlineTypes::truncate`).
  Reaproveitam do prefixo:
  - os parâmetros de tipo;
  - os alvos de *typedef*;
  - os dados das classes e a hierarquia (`build_class_hierarchy_onto`);
  - os tipos das variáveis e funções (inclusive os inferidos);
  - os `field_overrides` e os tipos escritos.

  `OutlineTypes::overrides_from` faz a inferência de *overrides* pular as classes do prefixo.
* **`crates/parity`.** Numa mudança de API, `take_world` devolve `Taken::Onto` quando nada
  fora do pacote mudou no disco nem nos textos. O residente devolve as unidades do pacote ao
  cache de unidades, carrega sobre o prefixo e mantém a `TypeTable`, os `CoreTypes` e o
  `OutlineTypes` do prefixo. A cada 16 cargas sobre o prefixo há uma recarga total, que
  descarta os tipos que a tabela acumulou. `DARTFORGE_FROZEN_PREFIX=0` volta à ordem de
  descoberta, para medir.
* **Buscas lineares.** Treze *lints* procuravam a classe de uma declaração andando por
  todas as classes do programa: `rules16`–`rules29`, `typed_lints4` e `typed_lints5`. Duas
  verificações de `phase_deprecated` faziam o mesmo para um construtor. Com o pacote depois
  do prefixo, essas buscas iam até o fim da arena. Agora usam o índice do programa
  (`Program::class_of_decl`, `ElementIndex::unit_functions`).

Testes novos (os dois comparam com uma análise do zero):
- `a_load_onto_the_frozen_prefix_is_a_full_load` (`crates/elements`): o programa carregado
  sobre o prefixo, despejado por URI e nome, é igual ao da carga inteira, e os IDs do
  prefixo sobrevivem;
- `resident_reloads_only_after_the_frozen_prefix` (`crates/parity`): quatro mudanças de API
  — tipo de campo, supertipo, classe nova, sobrescrita de membro do prefixo e classe
  genérica do prefixo — no próprio arquivo e nos dependentes publicam o mesmo que uma
  análise do zero.

Validação:
- testes em *release* de `elements`, `types`, `analysis`, `world`, `emit_js`,
  `emit_js_production`, `parity`, `lsp` e `dev`: 842, sem falhas;
- corpus JS 277/277 em desenvolvimento e em produção `-O2`;
- harness nativo (desenvolvimento): 164/164 (`--native`, `DARTFORGE_LLVM_DIR` com o LLVM 22.1.8);
- oráculo de paridade nos quatro projetos com a tabela de recursos idêntica (references,
  implementation e definition 100%, completion top-5 100%).

**Medição.** Três sessões revezando três servidores: o desta rodada ("agora"), o da quarta
rodada ("r4", `0265b0f2`) e o Dart. A máquina estava mais calma (carga 16–48). O `limitless_ui` foi medido numa rodada à parte
(carga 17–48), com o binário final, que faz a recarga total a cada 16 cargas sobre o
prefixo. No frontend e no backend, uma sessão não chega a 16 dessas cargas, então o
comportamento é o mesmo.

| ms (mediana) | frontend: r4 → agora × Dart | backend | `limitless_ui` |
|---|---|---|---|
| API privada → diagnóstico do arquivo | 478 → **236** × 25 | 127 → 105 × 30 | 272 → **161** × 85 |
| API exportada → diagnóstico do arquivo | 583 → **226** × 50 | 119 → 97 × 61 | 179 → **86** × 53 |
| API privada: CPU | 691 → **448** × 50 | 1.353 → 1.293 × 161 | 919 → 750 × 137 |
| API exportada: CPU | 2.180 → 1.999 × 110 | 1.388 → 1.337 × 183 | 866 → 657 × 101 |
| API privada → assentado | 776 → 668 × 53 | 1.924 → 1.722 × 275 | 1.207 → 924 × 145 |
| API exportada → assentado | 3.150 → 3.906 × 150 | 1.730 → 1.634 × 333 | 1.481 → 717 × 106 |
| tecla num corpo: CPU | 80 → 80 × 46 | 61 → 64 × 41 | 123 → 117 × 151 |
| partida: CPU de todos os processos | 3.328 → 3.381 × 3.961 | 1.956 → 2.051 × 2.792 | 4.376 → 4.279 × 3.802 |
| memória em repouso (MiB) | 744 → 776 × 847 | 286 → 299 × 469 | 1.045 → 1.105 × 773 |
| memória, pico (MiB) | 1.621 → 1.585 × 934 | 286 → 299 × 476 | 1.087 → 1.120 × 780 |

No frontend, sem a disputa por CPU da sessão revezada (`DARTFORGE_ENGINE_TIMES=1`, uma
sessão), o diagnóstico do arquivo depois de uma mudança de API caiu de 412–446 para 154–171 ms:

| Fase | Antes | Agora |
|---|---:|---:|
| carga | 0,14 s | 0,057 s |
| *outline* de tipos | 0,12 s | 0,053 s |
| passes de inicializadores | 0,10 s | 0,011 s |

O assentado da mudança exportada no frontend (3,2 → 3,9 s) é relógio dos dependentes; a CPU
dele caiu (2,18 → 2,00 s). No `limitless_ui`, a memória em repouso subiu 6% (1.045 → 1.105
MiB) e o pico 3%: a tabela de tipos guarda os *outlines* das cargas sobre o prefixo até a
recarga total. Com o limite em 64 cargas, a memória em repouso tinha subido 18%.

**O que falta para a meta:**
* **O diagnóstico do arquivo ainda leva 4–10× o do Dart (25–50 ms).** Sobram por mudança de
  API:
  - a carga das ~112 bibliotecas do pacote (cache de unidades, elementos e escopos);
  - o *outline* delas;
  - `simple_bounds` do programa inteiro;
  - os verificadores do arquivo.

  O passo 4 (reaproveitar as bibliotecas do pacote que não dependem da editada) e um
  `simple_bounds` por faixa são o próximo passo.
* **Os dependentes de uma mudança exportada custam ~2 s de CPU contra 110 ms do Dart** (a
  inferência dos corpos de todos os dependentes). Isso não muda com o *outline*
  incremental: pede reaproveitar os corpos cujas dependências de API não mudaram, como o
  Dart faz com a assinatura de API por biblioteca.
* **`references` dos tipos do SDK (passo 5)** continua de 4 a 37× o Dart.

#### Sexta rodada (2026-11-01): passo 4 (grupos em ordem de dependência) e ondas de arquivos gerados

**Passo 4, primeira parte (`f6b18bad`, `d14740ac`).**
- Depois do prefixo congelado, as bibliotecas do pacote vêm em ordem de dependência
  (`frozen::lay_out`, Tarjan). Cada grupo de bibliotecas que se importam mutuamente fica
  junto, com o início de cada grupo em `Program::groups`. A ordem vale na carga inteira e
  em cada carga sobre um corte.
- Uma mudança de API corta o programa no início do grupo da primeira biblioteca cujo texto
  mudou (`Resident::cut`): nos textos, no disco, ou na entrada, quando o lote importa outros
  arquivos. Antes, o corte era sempre no fim do prefixo congelado.
- As bibliotecas do pacote antes do corte mantêm elementos, *outline*, hierarquia e
  diagnósticos de *outline* e de sintaxe.
- A inferência de *overrides* ainda passa pelas classes do pacote inteiro
  (`overrides_from` = fim do prefixo congelado), para que as bibliotecas mantidas tenham os
  mesmos diagnósticos de inicializador de uma análise do zero.
- No `limitless_ui`, que não carrega o pacote inteiro com cada lote, a troca de lote depois
  de uma mudança de API recarrega só a entrada.
- **Ainda não feito:** refazer só o *outline* de quem depende do que mudou dentro de um grupo
  grande (os *barrels* do frontend formam ciclos com os componentes), e o `simple_bounds`
  por faixa (continua sobre o programa inteiro).
- O teste `resident_reloads_only_after_the_frozen_prefix` agora tem uma biblioteca do pacote
  que não depende da editada. Ela é mantida pelos cortes, e uma edição de corpo nela depois
  das mudanças de API publica o mesmo que uma análise do zero (diagnósticos de
  inicializador, de *override* e de corpo). `a_load_onto_the_frozen_prefix_is_a_full_load`
  ganhou um corte dentro do pacote.

**Ondas de arquivos gerados (`d91c8fbc`).** Uma onda é o `build_runner` ou o `webdev`
reescrevendo muitos arquivos de uma vez, que o editor manda em vários
`didChangeWatchedFiles`. O trabalhador da análise tipada agora espera 250 ms de disco quieto
(sem mudança do editor depois) e analisa a onda uma vez (`DISK_QUIET`). Teste:
`a_wave_of_disk_changes_is_analyzed_once`, com 12 arquivos `.g.dart` reescritos, cada um na
sua notificação.

**Validação do passo 4, separada:**
- testes em *release* de `elements`, `types`, `analysis`, `world`, `emit_js`,
  `emit_js_production`, `parity`, `lsp` e `dev`: 842, sem falhas;
- corpus JS 277/277 em desenvolvimento e em `-O2`;
- harness nativo 164/164;
- *debounce*: `typed_incremental` e clippy;
- oráculo de paridade no fim, com o binário final: tabela de recursos idêntica (references, implementation e definition 100%, completion top-5 100%).

**Medição.** Duas sessões revezando o binário desta rodada ("agora"), o da quinta ("r5") e
o Dart, com carga 29–70 (a máquina voltou a ficar carregada; valem a CPU e as razões). Em
ms, quando não indicado:

| (mediana) | frontend: r5 → agora × Dart | backend | `limitless_ui` |
|---|---|---|---|
| API privada → diagnóstico do arquivo | 460 → **265** × 19 | 127 → 201 × 48 | 223 → 196 × 96 |
| API exportada → diagnóstico do arquivo | 348 → 298 × 63 | 152 → 266 × 85 | 90 → **85** × 116 |
| API privada → assentado | 1.200 → 768 × 48 | 3.673 → 4.998 × 602 | 1.689 → 1.490 × 199 |
| API exportada → assentado | 4.709 → 3.582 × 161 | 4.086 → 5.207 × 576 | 1.298 → 1.345 × 185 |
| API privada: CPU | 498 → 493 × 45 | 1.323 → 1.317 × 181 | 815 → 784 × 161 |
| API exportada: CPU | 2.136 → 2.048 × 112 | 1.385 → 1.308 × 187 | 699 → 680 × 148 |
| memória em repouso (MiB) | 775 → **736** × 870 | 267 → 297 × 477 | 1.032 → 1.047 × 739 |

- **Backend:** o relógio piorou, com a CPU igual. Com duas sessões e carga até 70, isso não
  distingue efeito de ruído. A memória em repouso subiu 11%: o programa mantém as
  bibliotecas do pacote anteriores ao corte, e a tabela de tipos continua acumulando os
  *outlines* das cargas sobre o corte.
- **`limitless_ui`:** a mudança exportada já fica abaixo do Dart no diagnóstico do arquivo
  (85 × 116 ms).

**O que não foi feito nesta rodada:**
- **Reaproveitar a tabela de tipos sem a recarga total a cada 16 cargas.** A tabela é
  *hash-consed* e só cresce, e os tipos do prefixo ficam misturados aos de cada carga: os
  inicializadores do prefixo são inferidos sob demanda durante as análises. Reaproveitar
  pede uma coleta da tabela:
  - marcar os tipos alcançáveis a partir do que fica: o `OutlineTypes` até o corte, os
    `CoreTypes`, a hierarquia, os tipos escritos e os limites dos parâmetros de tipo;
  - copiá-los numa tabela nova e remapear os `TypeId` dessas estruturas;
  - fazer isso a cada carga sobre um corte.

  É o próximo passo da memória e serve também ao item 3 da prioridade (sessões longas com
  `build_runner`).
- **O cenário de sessão longa no `editing_latency.rs`** (horas simuladas, ondas de
  `build_runner`, curva de RSS dos dois servidores, comparação com um servidor recém-aberto)
  não foi escrito. Só o *debounce* das ondas ficou pronto.
- **O passo 5 (índice de referências resolvidas)** não foi começado.

#### Sétima rodada (2026-11-01): coleta da tabela de tipos do residente

**Coleta (`fdb70ba2`).** `TypeTable::collect(raízes)` mantém só os tipos alcançáveis a
partir de:
- as raízes;
- os limites de todos os parâmetros de tipo;
- as decorações (alias de `typedef`, tipo canônico) do que fica.

Os tipos mantidos são renumerados na ordem antiga e o resultado é o mapa de remapeamento. A
tabela refaz o `lookup`, `display`, `decorated`, `canonical` e `fresh_formals`. As
estruturas que guardam `TypeId` aplicam o mapa: `OutlineTypes::for_each_type`,
`ClassHierarchy::for_each_type` e `CoreTypes::for_each_type`.

A cada carga sobre um corte, o residente corta o *outline*, coleta a tabela
(`dartforge_types::collect_types`) e só então resolve o *outline* depois do corte. Os
*outlines* e corpos das cargas anteriores saem da tabela. A recarga total a cada 16 cargas
saiu; `DARTFORGE_LSP_ONTO_LIMIT=N` a traz de volta, só como rede de segurança.

Teste: `resident_reloads_only_after_the_frozen_prefix` repete as mudanças de API e exige
que o tamanho da tabela seja o mesmo a cada repetição.

**Teste do *debounce* (`664cc80b`).** `a_wave_of_disk_changes_is_analyzed_once` conta as
análises (`Server::typed_analyses`), com as notificações espaçadas como o editor manda.
- Com o *debounce*: uma análise para a onda.
- Com `DARTFORGE_LSP_DISK_QUIET_MS=0`: 12 análises, e o teste falha. Isso prova que ele
  detecta o defeito.

**Validação:**
- testes em *release* de `elements`, `types`, `analysis`, `world`, `emit_js`,
  `emit_js_production`, `parity`, `lsp` e `dev`: 843, sem falhas;
- corpus JS 277/277 em desenvolvimento e em `-O2`;
- harness nativo 164/164;
- oráculo de paridade com o binário final: tabela de recursos idêntica (references, implementation e definition 100%, completion top-5 100%).

**Memória em repouso depois de 1 e de 12 rodadas de mudanças de API.** Cada rodada tem uma
mudança privada e uma exportada, cada uma desfeita. *Footprint* somando todos os processos,
em MiB, sessões em sequência revezando os servidores, carga 5–36:

| | agora (coleta) | sexta rodada (recarga a cada 16) | Dart |
|---|---|---|---|
| backend | 206,7 → **205,1 (−1%)** | 215,9 → 206,0 | 396,5 → 279,4 |
| `limitless_ui` | 304,8 → 364,0 (+19%) | 309,1 → 402,3 (+30%) | 482,2 → 426,8 |
| frontend | 442,0 → 471,1 (+7%) | 428,9 → 498,3 (+16%) | 494,3 → 442,2 |

- **Backend:** plano.
- **`limitless_ui` e frontend:** a coleta corta o crescimento pela metade, mas não o zera.
  No `limitless_ui`, o pico de RSS quase não sobe (315,8 → 329,5 MiB) enquanto o
  *footprint* em repouso sobe 19%. Parte é memória que o alocador do macOS guarda e não
  devolve, e parte é o programa mantido: sem o pacote inteiro em cada lote, o programa do
  residente acumula todas as bibliotecas que algum lote já importou.
- **O critério do proprietário não está cumprido:** repouso a poucos % de um servidor
  recém-aberto, sem reiniciar nada. Falta:
  - descartar do programa mantido as bibliotecas que o lote atual não alcança;
  - medir o que é retenção do alocador e o que é dado vivo (`DARTFORGE_LSP_MEMORY=1` no
    residente).

**O que não foi feito nesta rodada:**
- **Cenário de sessão longa no `editing_latency.rs`** (item 3a): ondas de `build_runner`
  intercaladas com edições, centenas de cargas e curva de RSS dos dois servidores. Não foi
  escrito. A tabela acima, com 12 rodadas, é o substituto curto.
- O resto do passo 4 (só os dependentes dentro de um grupo grande) e o passo 5 (índice de
  referências).

#### Oitava rodada (2026-11-01): sessão longa com ondas de arquivos gerados

**O cenário (`df955ec6`).** `editing_latency --long N --generated G --csv arquivo`:
- **cada rodada:**
  - uma onda de G arquivos `.g.dart` reescritos no disco (um `didChangeWatchedFiles` por
    arquivo, a cada 5 ms), como `build_runner` ou `webdev`;
  - uma edição de corpo no arquivo principal, e o desfazer;
  - a cada cinco rodadas, uma mudança de API em `--api`, e o desfazer;
- **amostras:** RSS e *footprint* de todos os processos a cada 2 s, no CSV;
- **fim:** um minuto quieto, depois pico, inclinação (MiB/h e MiB por 100 cargas) e o valor
  em repouso;
- **`--long 0`:** só abre, descansa e mede, para medir o servidor recém-aberto sobre os
  mesmos arquivos finais.

Os arquivos gerados importam o arquivo principal e ficam em `lib/df_long_session` do pacote
dele. Script: `long_a954.sh`; curvas em `bench/results/lsp-long-session/*.csv`.

**Medição.** Workspace `new_sali` inteiro (frontend, core e backend), arquivo principal
`processo_filtros_component.dart`, API em `documento_service.dart`. 150 rodadas de 40 arquivos
gerados: 510 cargas, sendo 150 ondas, 300 edições de corpo e 60 de API. A máquina estava
carregada (carga 26–69; uma validação completa rodava ao mesmo tempo).

*Footprint* de todos os processos, em MiB. O RSS no macOS cai com a compressão de memória
sob carga (o nosso em repouso deu 11,7 MiB de RSS e 506 MiB de *footprint*), então o
*footprint* é a medida que vale.

| | DartForge | Dart |
|---|---:|---:|
| duração da parte ativa | 672 s | 169 s |
| pico | 507 | 845 |
| inclinação por 100 cargas | **4,7** | 60,2 |
| inclinação por hora (no ritmo de cada um) | 116 | 5.710 |
| em repouso depois da sessão (1 min quieto) | **506** | 845 |
| servidor recém-aberto, mesmos arquivos | 339 | 429 |
| sessão longa ÷ recém-aberto | **1,49×** | 1,97× |

- **O Dart cresce sem parar:** ~60 MiB a cada 100 cargas, do primeiro ao último minuto, e
  dobra em relação ao recém-aberto. É o comportamento que o proprietário vê no VS Code.
- **O nosso sobe nas primeiras ~100 cargas** (339 → 490) e depois quase para (490 → 506 em
  410 cargas). Mas fica 49% acima do recém-aberto.
- **O critério do item 3 não está cumprido:** repouso a poucos % do recém-aberto e
  inclinação perto de zero.
- **Por cada hora de relógio, os números não são comparáveis:** o nosso servidor leva 4×
  mais tempo por carga nesta sessão, então a inclinação por 100 cargas é a comparação justa.

**O que não foi feito nesta rodada:**
- **Dado vivo × retenção do alocador** (`vmmap --summary` / `heap` no residente) não foi
  medido. Então não se sabe ainda se os +49% são o programa mantido (o pacote inteiro mais os
  arquivos gerados) ou páginas que o alocador guarda.
- **Descartar do programa mantido as bibliotecas que o lote não alcança** não foi feito. Nem
  a nova medição de planicidade que depende dela.
- **O resto do passo 4 e o passo 5** não foram começados.

#### Nona rodada (2026-11-01): de onde vêm os +49% e o tempo por tipo de carga

**Dado vivo × retenção.** `vmmap --summary` do processo residente em repouso, com carga
18–46. "Sessão longa": 150 rodadas, 510 cargas, depois de um minuto quieto. "Recém-aberto":
o servidor aberto sobre os mesmos arquivos finais. Em MiB:

| residente | recém-aberto | sessão longa | diferença |
|---|---:|---:|---:|
| *footprint* | 319,6 | 494,1 | +174,5 |
| `malloc`: alocado (vivo) | 287,4 | 376,9 | **+89,5** |
| `malloc`: fragmentação | 14,9 (5%) | 67,8 (16%) | **+52,9** |
| resto (fora do `malloc`: regiões, pilhas, binário) | 17,3 | 49,4 | +32,1 |

- **Dado vivo:** cerca de metade do crescimento. Ainda não está dividido por estrutura
  (programa mantido, arquivos gerados, tabela de tipos, caches). O `heap` do macOS não
  separa alocações de Rust por tipo; isso pede contadores próprios no residente.
- **Fragmentação do alocador do sistema:** cerca de 30% do crescimento.
- **Resto:** o restante, fora do `malloc`.
- O processo do servidor e o *driver* não crescem (13–15 MiB e 30 MiB).

**Tempo por tipo de carga** (`af2f56b9`; 30 rodadas, carga 20–34; tempo até assentado):

| carga | DartForge | Dart |
|---|---:|---:|
| onda de 40 `.g.dart` | **1.513 ms** | 10 ms |
| edição de corpo (e desfazer) | 77 ms | 35 ms |
| mudança de API (e desfazer) | 1.840 ms | 128 ms |

- As ondas dominam (45 dos 81 s ativos), seguidas das mudanças de API.
- A onda custa mais que uma mudança de API: cerca de 250 ms são o *debounce*, e o resto é
  recarregar e analisar os 40 arquivos gerados.
- O Dart assenta a onda em 10 ms. É preciso ver se ele de fato reanalisa os arquivos ou só
  os marca, antes de comparar.

**O que não foi feito nesta rodada:**
- **Dividir o dado vivo por estrutura, e com isso a correção correspondente:** descartar o
  que o lote não alcança e limitar caches.
- **Devolver memória ao sistema depois de cada carga:** `malloc_zone_pressure_relief` já se
  mostrou inútil antes. As saídas são uma arena por carga ou um alocador com *purge*
  (mimalloc/jemalloc), o que pede trocar o alocador global do residente.
- **Corrigir o custo da onda**, e rodar de novo o `--long 150` dos dois servidores.

#### Décima rodada (2026-11-01): mimalloc no residente, testado e descartado

**mimalloc com *purge*.** Experimento: mimalloc 0.1.52 (licença MIT, já no cache do
registro) como alocador global do binário `dartforge-lsp`, com `mi_collect(true)` no
residente depois de cada análise. A/B com `--long 150` revezado, carga 17–132. Uma
validação completa de outro agente rodava ao mesmo tempo, então os tempos estão muito
distorcidos. *Footprint* de todos os processos, em MiB:

| | sistema (`r19`) | mimalloc + *purge* (`r20`) | Dart |
|---|---:|---:|---:|
| recém-aberto | 310,5 | 319,2 | 528,5 |
| fim da sessão longa (1 min quieto) | 410,3 | 420,0 | 682,3 |
| fim ÷ recém-aberto | 1,32× | 1,32× | 1,29× |

O repouso não caiu: os mesmos +100 MiB com os dois alocadores. Pelo critério da rodada (só
fica se o repouso cair sem piorar os tempos), o mimalloc **não foi integrado**. Os tempos
por carga do `r19` (corpo 1.138 ms, API 13,3 s) correram com carga 60–130, contra 17–58 do
`r20`, e não servem de comparação.

**A onda de 1.513 ms é artefato do *driver*.** Nas três sessões dos dois binários, a onda
deu exatamente 1.513 ms. O *driver* espera até 500 ms o início da análise (`wait_start`) e
depois 1 s de quietude (`settle`). Com o *debounce* de 250 ms do servidor, a análise começa
depois dessas janelas, e o tempo medido é a soma delas, não a análise. Nesta sessão o Dart
deu 244 ms por onda, não os 10 ms da rodada anterior: o número dele também depende da
carga. Antes de comparar ondas, o *driver* precisa esperar o início da análise além do
*debounce*.

**O que não foi feito nesta rodada:**
- **Item 1:** nenhuma redução de memória integrada. O mimalloc foi revertido.
- **Item 2:** o modelo do Dart (análise preguiçosa dos arquivos fora da prioridade) não foi
  verificado no código dele. A medição da onda no *driver* não foi corrigida, e o custo real
  de recarregar os 40 arquivos não foi perfilado.
- **Item 3:** os contadores por estrutura no residente não foram escritos. Os +89,5 MiB de
  dado vivo continuam sem divisão.
- **Item 4:** com o *driver* ainda medindo a onda errado, a tabela de tempo por carga desta
  rodada não é confiável.

#### Décima primeira rodada (2026-11-02): medida por publicação, memória por estrutura e o que crescia

**1. A medida de cada carga (`6828d91f`).** O *driver* media toda carga pelo
`$/analyzerStatus`: até 500 ms esperando o início, depois 1 s de silêncio. Dois defeitos:
- o `didChangeWatchedFiles` não marcava o nosso servidor como analisando, e o *debounce* de
  250 ms começava a análise depois das janelas: toda onda media 1.513 ms, nos dois
  servidores;
- o Dart não anuncia a análise de boa parte das edições: numa sessão de 150 rodadas, 58 das
  60 mudanças de API saíram sem início anunciado.

Agora cada carga é medida da mudança (a última notificação da onda) até a publicação dos
diagnósticos novos dos arquivos afetados, e até assentar:
- **onda:** cada `.g.dart` tem um nome indefinido que muda a cada onda (`dfWave0..2`, três
  versões, para o conjunto de nomes não crescer). A onda acaba quando os 40 arquivos são
  publicados com o nome novo. Os arquivos são escritos e notificados um a um, como um
  *watcher* os vê;
- **corpo:** a edição põe um local não usado; a mudança de API, uma função pública com erro de
  tipo. Cada edição e cada desfazer acabam quando a publicação do arquivo tem (ou deixa de
  ter) o diagnóstico daquela linha;
- as rodadas começam depois que a primeira onda foi publicada. Com a máquina carregada, o Dart
  ainda analisava o *workspace* 11–14 s depois do "assentado" da partida;
- o pico e a inclinação passaram a ser do *footprint*. Antes eram do RSS, que o macOS
  comprime em repouso;
- `--long 0` escreve os mesmos arquivos gerados, para o recém-aberto ver os mesmos arquivos
  (`acdb1d19`).

O servidor agora anuncia a análise que uma mudança no disco começa, como o Dart faz com o
*watcher* dele, e o trabalhador acorda o laço principal no fim de toda análise. Assim o fim
sai mesmo sem resultados.

Validado nos dois servidores: em 150 ondas de cada um, todas as 40 publicações chegaram, e
nenhuma carga ficou sem publicação. Script: `scripts/long_lsp11.sh`.

**2. Contadores por estrutura (`a7ada9e0`).** O pedido `dartforge/residentMemory` mede o
residente estrutura por estrutura (`Resident::measure_memory`):
- o trabalhador repassa o pedido entre análises;
- o residente solta cada estrutura e lê os bytes em uso do alocador antes e depois;
- a medida é destrutiva (o residente termina vazio);
- o *driver* pede no fim da sessão, depois do minuto quieto, e o Dart responde com erro.

Com `DARTFORGE_LSP_MEMORY=1`, o residente imprime ainda um censo (só contagens) depois de cada
análise. `DARTFORGE_RESIDENT_LIBRARIES=<arquivo>` grava as bibliotecas do programa.

Heap vivo do residente no `new_sali`, em MiB (binário antes do passo 3):

| estrutura | só aberto | 1 rodada | 30 rodadas | 150 rodadas |
|---|---:|---:|---:|---:|
| unidades do programa: pacotes e projeto | 210,15 | 241,69 | 244,45 | 244,45 |
| unidades do programa: gerados | 78,35 | 80,01 | 80,01 | 80,01 |
| unidades do programa: SDK | 19,46 | 20,05 | 21,01 | 21,01 |
| bibliotecas (escopos, *namespaces*) | 61,41 | 68,37 | 68,37 | 68,37 |
| elementos | 18,87 | 22,58 | 22,58 | 22,58 |
| tipos do *outline* | 11,61 | 17,98 | 17,73 | 17,73 |
| tabela de tipos | 10,85 | 11,07 | 12,36 | **16,17** |
| interner | 6,86 | 8,18 | 8,19 | 8,19 |
| índice de elementos | 2,31 | 2,63 | 2,63 | 2,63 |
| cache de unidades (mapas) | 0,27 | 0,27 | 0,73 | 0,73 |
| **heap do residente** | **420,2** | **473,2** | **478,5** | **482,3** |

Depois da medida sobram 0,1 MiB: o motor com o SDK não pesa, e todo o heap é o que o
residente guarda.

**O que era o crescimento:**
- **Só aberto → 1 rodada (+53 MiB):** é um degrau, não um crescimento por carga. Na primeira
  edição, o residente passa a carregar o pacote inteiro com cada lote (o contexto,
  `context_pays`). São 3.164 bibliotecas em vez de 2.771, e o conjunto de bibliotecas não muda
  mais até a 150ª rodada. É isso que separava o "recém-aberto" das rodadas anteriores
  (339 → 506, 310 → 410 MiB): o recém-aberto nunca tinha feito uma edição.
- **1 → 150 rodadas (+9,1 MiB):** o que cresce com o número de cargas é a tabela de tipos
  (+5,1 MiB), mais dois caches que saturam:
  - as unidades (+3,7 MiB): escopos léxicos derivados sob demanda de mais árvores, que param
    em 30 rodadas;
  - os mapas do cache de unidades (+0,46 MiB).

**3. A raiz: os parâmetros de tipo nunca saíam da tabela (`50daa279`).** A coleta da tabela a cada carga
sobre um corte (sétima rodada) mantinha todos os parâmetros de tipo, e os limites deles
contavam como raízes. Cada carga resolve de novo o *outline* depois do corte. Com isso,
cada carga deixava na tabela os parâmetros dos elementos que soltou e os tipos que os limites
deles alcançam.

A correção:
- **`TypeTable::collect_dropping`** (`collect_types_at`):
  - os parâmetros de dono solto (classe, extensão, *typedef* ou função depois do corte) saem;
  - os de tipos de função genérica que nenhum tipo mantido tem também saem;
  - os slots livres servem aos próximos parâmetros (`alloc_type_param`), sem renumerar nada;
  - os limites dos parâmetros soltos deixam de manter tipos vivos;
  - `fresh_formals` e `without_default_type` perdem as entradas dos parâmetros soltos;
  - `DARTFORGE_TYPE_PARAMS_KEPT=1` mantém o comportamento antigo, para medir.
- **O teste `resident_reloads_only_after_the_frozen_prefix`** ganhou declarações genéricas. Ele
  exige que tipos e parâmetros fiquem do mesmo tamanho a cada repetição das mudanças de API.
  Com `DARTFORGE_TYPE_PARAMS_KEPT=1` o teste falha: os parâmetros crescem 13 → 29 e os tipos
  45 → 55 em seis cargas.
- **Soltar o que o lote não alcança:** quando os *imports* da entrada mudam, o corte desce até a
  primeira biblioteca do pacote que os novos *imports* não alcançam (`first_unreached`). O
  programa mantido passa a ser o que uma carga só daquele lote teria antes do corte, e não
  tudo o que algum lote já importou. Teste: `resident_drops_what_the_batch_no_longer_reaches`.
  No `new_sali` todo lote carrega o contexto, os *imports* não mudam e isso não muda nada; vale
  para os pacotes em que o contexto não paga, como o `limitless_ui`. O prefixo congelado (SDK e
  dependências) continua acumulando as dependências que algum lote alcançou: soltá-lo pede uma
  recarga total, que não foi feita.
- **Os mapas do cache de unidades** devolvem a capacidade a mais depois de cada carga
  (`AnalyzedUnitCache::shrink`).

Tabela de tipos no fim de 150 rodadas: **16,17 → 11,19 MiB**, igual à de 1 rodada (11,07 MiB).
São 54.598 tipos e 3.789 parâmetros, contra 2.791 parâmetros em 1 rodada.

**4. Sessão longa, 150 rodadas (510 cargas), os dois servidores.** `new_sali` inteiro, ondas
de 40 `.g.dart`, carga 41–130. *Footprint* de todos os processos, em MiB:

| | DartForge antes (passo 2) | DartForge agora | Dart |
|---|---:|---:|---:|
| recém-aberto (só abrir, 1 min quieto) | 449,0 | 445,9 | 533,1 |
| recém-aberto + 1 rodada (3 cargas) | 508,3 | 506,4 | 450,6 |
| fim da sessão longa (1 min quieto) | 534,2 | 537,0 | 886,0 |
| pico | 598,0 | 598,8 | 1.103,1 |
| inclinação por 100 cargas | +0,6 | **−0,7** | **+62,2** |
| heap vivo do residente: 1 rodada → fim | 473,2 → 482,3 (+1,9%) | 473,2 → **476,9 (+0,8%)** | — |
| fim ÷ recém-aberto + 1 rodada | 1,05× | **1,06×** | 1,97× |
| fim ÷ recém-aberto (só abrir) | 1,19× | 1,20× | 1,66× |

- **Nosso dado vivo ficou plano:** +0,8% de 1 a 150 rodadas, sem nenhuma estrutura crescendo
  com as cargas. A inclinação do *footprint* é zero.
- **O *footprint* em repouso fica 6% acima do de 1 rodada (+30 MiB):** com o dado vivo plano,
  é memória que o alocador guarda depois dos picos (~598 MiB em toda a sessão). Ela sobe nas
  primeiras rodadas e não cresce mais. Não foi atacada nesta rodada (veja abaixo).
- **Contra o servidor só aberto, o fim fica 20% acima.** É o degrau do contexto na primeira
  edição: +53 MiB de dado vivo, uma vez. O critério "a poucos % de um recém-aberto" vale contra
  um servidor que já fez uma edição, e não contra um que nunca editou.
- **O Dart cresceu 62 MiB a cada 100 cargas,** do começo ao fim, e terminou 1,97× acima do de 1
  rodada.

Tempo por carga (mediana; a mesma sessão; o *driver* novo):

| carga | DartForge: diagnósticos | DartForge: assentado | Dart: diagnósticos | Dart: assentado |
|---|---:|---:|---:|---:|
| onda de 40 `.g.dart` | 2.309 ms | 2.309 ms | 748 ms | 767 ms |
| edição de corpo (e desfazer) | 1 ms¹ | 357 ms | 194 ms | 214 ms |
| mudança de API (e desfazer) | 712 ms | 14.688 ms | 226 ms | 3.182 ms |

¹ A publicação imediata do nosso servidor, antes da análise tipada, já traz (ou já tira) o
aviso do local não usado.

A onda de verdade custa ~2,3 s contra 0,75 s do Dart: 250 ms de *debounce* mais a recarga e a
análise dos 40 arquivos. A mudança de API assentada (os dependentes) é 4,6× a do Dart. São os
itens 1 e 2 da prioridade.

**5. Windows: o prefixo congelado do teste (`b2ce641f`).** Pedido do coordenador. No Windows,
`a_load_onto_the_frozen_prefix_is_a_full_load` não congelava nenhuma biblioteca. O teste
montava a raiz "fora do pacote" com `std::fs::canonicalize` (`\\?\C:\…`) e a comparava com os
caminhos das unidades, que o *loader* grava sem esse prefixo (eles voltam de URIs `file:`).
`config::canonical_path` é a forma canônica do *loader* (caixa e *links* resolvidos, sem o
prefixo): a entrada do *loader* e o teste passam por ela. O residente do LSP já comparava
chaves sem o prefixo (`analysis::key`), então o prefixo congelado dele não era afetado.
Verificado no Windows 11 x64: o teste falhava (0 × 2) e agora passa com o resto do *crate*.

**Validação:**
- oráculo de paridade nos quatro projetos de sempre, com o binário final: `references`,
  `implementation` e `definition` 100%, *completion* top-5 100% (top-1 99%);
  - o SDK em `~/tools/dart` é um *link* para `/Volumes/ssd_dock/tools/dart`. O Dart responde
    com o caminho real e o nosso com o dado em `DARTFORGE_SDK_LIB`;
  - com o *link* resolvido nas respostas, os URIs são idênticos;
- testes em *release* de `elements`, `types`, `analysis`, `world`, `emit_js`,
  `emit_js_production`, `parity`, `lsp` e `dev`: 845, sem falhas. Os 19 de
  `emit_js/tests/basic.rs` pedem o `node` no `PATH` e rodaram à parte;
- corpus JS 277/277 em desenvolvimento e em produção `-O2`;
- harness nativo (desenvolvimento): 164/164 (`--native --corpus corpus/native`, LLVM 22.1.8).
  O `--native` sobre o `corpus/js` deu 273/277. As 4 falhas (`253`, `256`, `272`, `273`) estão
  no *backend* nativo, que esta rodada não tocou, e não foram comparadas com o `main`;
- clippy `-D warnings` em `elements`, `types`, `parity` e `lsp` (todos os alvos);
- Windows: `cargo test --release -p dartforge-elements` com a correção do prefixo.

**O que não foi feito nesta rodada:**
- **A memória que o alocador guarda depois dos picos (+30 MiB, 6%) não foi atacada.** Fica a
  hipótese de reconstruir de forma compacta as tabelas de vida longa depois que uma carga
  assenta. Os +53 MiB do contexto na primeira edição também ficam: tirá-los pede decidir o
  contexto já na primeira carga, ou soltá-lo em repouso, e as duas coisas mexem no tempo do
  primeiro diagnóstico.
- **O prefixo congelado não solta as dependências** que nenhum lote alcança mais.
- **Item 4 (análise preguiçosa do Dart) não foi verificado.**
- **O custo da onda (2,3 s contra 0,75 s)** e o das mudanças de API não foram atacados.

#### Décima segunda rodada (2026-11-02): a distância de velocidade para o Dart

Meta do coordenador: no `new_sali`, a sessão longa não pode ser mais lenta que o Dart em
nenhum dos três tipos de carga. Na décima primeira rodada perdíamos nos três: mudança de API
14,7 × 3,2 s, onda 2.309 × 767 ms, edição de corpo 357 × 214 ms.

**Como se mediu.**
- **A CPU de cada carga (`52a2bb17`).** Com a máquina dividida entre agentes, o relógio de uma
  carga conta também a espera por um núcleo. Agora a sessão longa informa, por tipo de carga, a
  CPU do servidor e dos descendentes da mudança até assentar.
- **`--tail MS`** informa a CPU gasta nos MS seguintes a cada carga assentada. Foi assim que se
  viu que o Dart não reanalisa os dependentes de uma mudança de API: ~75 ms de CPU, e nada
  depois de assentar.
- **`--json`** grava a última publicação de cada arquivo, para conferir que dois binários
  publicam o mesmo.
- **O perfil** saiu do `sample` do macOS sobre o residente e sobre o servidor, durante sessões
  de 10 rodadas.

**1. Os verificadores por biblioteca andavam pelo programa inteiro (`40bce2b0`).**
- **As regras de *lint* de árvore:** rodavam todas (~300) em cada arquivo, e a publicação ficava
  só com as poucas que o `analysis_options.yaml` liga. Agora o motor roda só as regras ligadas
  pelas opções que valem para o arquivo (`Engine::lint_rules_from_options`, ligado no LSP).
- **Os verificadores de cada biblioteca:** `main_function`, `variance`, `invalid_overrides`,
  `unused_parameters`, `unnecessary_no_such_method`, os `final` não inicializados e mais
  onze andavam por todas as classes, funções e variáveis do programa para achar as da
  biblioteca. O `unused_elements` procurava cada construtor na arena inteira de funções. Agora
  andam pelos elementos da biblioteca ou da unidade (`Program::index`, que ganhou
  `library_classes`), na mesma ordem de ids.
- **A varredura do disco:** cada carga sobre um corte lia o `stat` de cada unidade mantida
  quatro vezes. Agora lê uma vez por análise.
- **O `package_config.json`:** o `PackageConfig::load` canonicalizava todos os diretórios de
  pacote a cada chamada. Agora há um cache do processo, válido enquanto data, tamanho e a
  existência do diretório gerado não mudam.

CPU por carga, 10 rodadas revezadas com o binário da décima primeira rodada (carga 37–48):
onda 418–437 → 284–290 ms, corpo 102–114 → 72–81 ms, API 2.405–2.797 → 1.537–1.564 ms.

**2. Reanalisar só os dependentes que veem um nome mudado (`e4642ed6`).** É o item 1 da
prioridade, pelo nome. Cada `Facts` agora traz:
- a API declaração por declaração (`incremental::api_digest`): o resumo dos *tokens* de
  assinatura de cada declaração de topo e de cada membro, e os identificadores neles;
- as palavras do texto (`incremental::text_words`).

Numa mudança de API, o plano compara o resumo velho e o novo e reanalisa a biblioteca do
arquivo mudado e só os dependentes que podem ver um nome mudado (`api_affected`):
- **o dono:** um membro mudado muda também o nome do dono;
- **famílias seladas:** uma classe mudada muda as classes da biblioteca que dividem com ela um
  supertipo da biblioteca. Um `switch` pode citar só um subtipo;
- **propagação:** uma declaração cuja assinatura cita um nome mudado muda também (um tipo
  inferido, um supertipo). Vale na biblioteca e em cada dependente afetado, até um ponto
  fixo;
- **quem é afetado:** um dependente cujo texto tem um nome mudado como palavra, comentários
  incluídos, ou que tem `export` e aponta para um arquivo afetado (um *barrel*);
- **o caminho antigo** (todos os dependentes) continua quando as diretivas mudaram, quando uma
  declaração mudada pode ser usada sem o nome (operadores, extensões sem nome, `call`,
  `iterator`, `toString`…), quando falta um resumo, ou com `DARTFORGE_LSP_API_BY_NAME=0`.

Os outros dependentes ficam com a publicação que já tinham.

O teste `typed_incremental` ganhou três casos, e em cada passo publica o mesmo que uma análise
do zero:
- uma família selada percorrida por um `switch` que cita só os subtipos;
- uma base cujo membro abstrato novo uma subclasse duas bibliotecas adiante precisa
  implementar;
- um tipo inferido que passa por uma biblioteca que cita a função mudada até uma que não
  cita.

Uma mudança de API que ninguém cita analisa o arquivo e o *barrel*, e nada mais.

**3. O que cada análise refazia sem precisar (`789d3b9b`).**
- **A varredura do disco:** o residente lia o `stat` das suas ~3.600 unidades. O trabalhador
  agora avisa quando só mudaram textos do editor, e a varredura é pulada, com uma pelo menos a
  cada 30 s (`Resident::disk_unchanged`).
- **A árvore do pacote:** o plano andava pela árvore do pacote atrás dos `.dart`. A lista agora
  fica guardada e é refeita quando um arquivo mudado foi criado ou apagado, quando o
  `analysis_options.yaml` mudou, ou depois de 30 s.
- **A chave de caminho:** era refeita componente por componente para cada unidade. Um caminho
  que já é chave volta como está (`generated::key`, Unix).
- **Os `Facts`:** a assinatura de API e o resumo dividem um só *lexing*.
- **O *debounce* da onda:** caiu de 250 ms para 20 ms, porque os arquivos de uma onda chegam a
  poucos milissegundos um do outro, e uma mudança depois cancela a análise que começou cedo
  demais.

**Sessões curtas, máquina quase parada.** Mediana até assentar (CPU), 10 rodadas revezadas,
em ms:

| carga | r11 (carga 2–17) | depois do passo 2 (carga 2–17) | agora (carga 5–25) | Dart (carga 2–25) |
|---|---:|---:|---:|---:|
| mudança de API | 1.304–1.577 (1.340–1.583) | 152–209 (159–193) | 131–142 (114–119) | 102–343 (72–139) |
| onda de 40 `.g.dart` | 476–479 (268–274) | 185–188 (145–149) | 134–135 (124–127) | 99–373 (178–316) |
| edição de corpo | 75–80 (67–69) | 58 (49) | 44–47 (22) | 34–116 (27–64) |

O Dart varia muito entre as sessões: na janela mais calma (carga 2–17), ele assentou a onda
em 99–108 ms e o corpo em 34 ms. A última publicação dos 492 arquivos (9.763 diagnósticos) é
a mesma em todos os binários.

**4. Sessão longa, 150 rodadas (510 cargas), os dois servidores revezados** (carga 58–63, a
máquina com outros agentes). Medianas, em ms:

| carga | DartForge r11 | DartForge agora | Dart agora | Dart r11 |
|---|---:|---:|---:|---:|
| onda: diagnósticos dos gerados | 2.309 | 588 | 467 | 748 |
| onda: assentado | 2.309 | 588 | 476 | 767 |
| onda: CPU | — | **186** | 216 | — |
| corpo: assentado | 357 | **118** | 136 | 214 |
| corpo: CPU | — | **36** | 38 | — |
| API: diagnóstico do arquivo | 712 | 235 | 127 | 226 |
| API: assentado | 14.688 | 795 | 420 | 3.182 |
| API: CPU | — | 185 | **103** | — |

- **Edição de corpo:** ganhamos, no relógio e na CPU.
- **Onda:** gastamos menos CPU que o Dart (186 × 216 ms), mas no relógio ficamos 112 ms atrás
  com a máquina carregada. A carga passa por mais trocas de processo e de *thread* (servidor,
  trabalhador, residente), e cada uma espera um núcleo. Com a máquina calma, ficamos à frente
  ou empatados.
- **Mudança de API:** caiu de 14,7 s para 0,8 s, mas ainda é 1,9× o Dart no relógio e 1,8× na
  CPU. Ela recarrega e resolve de novo o *outline* de todas as bibliotecas depois do corte
  (~55 ms de carga e ~55 ms de *outline*). Refazer só o *outline* de quem depende do que mudou
  é o resto do item 1 (identidade estável de elementos), que não foi feito.

**Memória** (*footprint* de todos os processos, em MiB, mesmas sessões):

| | DartForge | Dart |
|---|---:|---:|
| recém-aberto (só abrir) | 488,1 | 549,0 |
| recém-aberto + 1 rodada | 550,8 | 508,9 |
| fim da sessão longa (1 min quieto) | **540,6** | 731,5 |
| pico | 578,7 | 759,5 |
| inclinação por 100 cargas | **−0,3** | +44,3 |
| dado vivo do residente, 1 rodada → fim | 473,5 → 478,3 (**+1,0%**) | — |

O resultado de memória da décima primeira rodada se mantém: inclinação ≤ 0, e dado vivo a
+1% do recém-aberto com uma rodada. As curvas estão em
`bench/results/lsp-long-session/*-r22-*.csv`.

**Validação** (com o binário final):
- testes em *release* de `elements`, `types`, `analysis`, `world`, `emit_js`,
  `emit_js_production`, `parity`, `lsp` e `dev`: 846, sem falhas (com o `node` no `PATH`);
- oráculo de paridade nos quatro projetos de sempre, com a tabela de recursos idêntica à da
  décima primeira rodada: `references`, `implementation` e `definition` 100%, *completion*
  top-5 100% (top-1 99%);
- clippy `-D warnings` em `elements`, `types`, `analysis`, `parity` e `lsp` (todos os
  alvos), e `rustfmt` nos arquivos tocados;
- a última publicação dos 492 arquivos do `new_sali`, depois das sessões, é igual à do binário
  da décima primeira rodada, salvo o marcador da onda (`dfWave0` × `dfWave1`) nos 40 gerados.

**O que não foi feito nesta rodada:**
- **A mudança de API ainda é ~2× o Dart.** Ela recarrega tudo o que vem depois do corte no
  programa mantido. Refazer só o *outline* das bibliotecas que dependem do que mudou pede a
  identidade estável de elementos (item 1 da prioridade).
- **A onda com a máquina carregada:** perde no relógio, com menos CPU. Juntar as trocas de
  processo (o trabalhador e o residente) não foi tentado.
- **A medida da onda e do corpo com a máquina calma** variou muito no Dart entre sessões, e
  não houve janela calma longa o bastante para as 150 rodadas.

#### Décima terceira rodada (2026-11-02): recarregar só o que depende da mudança

##### Projeto (escrito antes do código)

**O que a medida mostrou.** O residente imprime agora onde corta o programa mantido
(`DARTFORGE_ENGINE_TIMES=1`). No `new_sali`, o programa tem 3.164 bibliotecas; as 2.185
primeiras são o prefixo congelado (SDK e dependências) e o resto é o pacote. Em 20 rodadas:
- **cada onda** cortava na biblioteca 2.770 e recarregava as **394** seguintes: os 40 `.g.dart`
  e 354 bibliotecas que só estavam depois deles por acaso da ordem;
- **cada mudança de API** em `documento_service.dart` cortava na 2.425 e recarregava **739**,
  das quais ~260 dependem do arquivo.

A ordem das bibliotecas depois do prefixo é uma ordem de dependência (Tarjan). O corte
recarrega tudo o que vem depois da primeira biblioteca mudada, dependa dela ou não.

**Por que não basta o nome (décima segunda rodada).** A análise por nome decide quais
dependentes *publicam* de novo. Mas o programa mantido guarda elementos com ids densos
(`ClassId`, `FunctionElementId`, `VariableId`), e tudo o que vem depois do corte é refeito
com ids novos: escopos, *outline*, hierarquia, tipos. É a "identidade estável" do item 1.

**Etapa 1 (esta rodada): a ordem prefere o que muda por último.** Entre as ordens de
dependência possíveis, `frozen::lay_out` escolhe, a cada passo, entre os grupos (componentes
fortemente conexos) prontos para entrar:
1. primeiro os que não dependem de um arquivo que mudou nesta carga
   (`Program::layout_last`, que o residente preenche com o que o corte viu mudar);
2. depois os que têm dependentes;
3. por último os que mudaram;
4. empate: a ordem antiga.

Isso continua sendo uma ordem de dependência: nada que não dependa do que mudou pode
depender de algo que dependa. Uma biblioteca que ninguém importa (um `.g.dart`) vai para o
fim.

O que fica igual, e por isso a etapa não mexe no que as rodadas 7 e 11 garantiram:
- **o prefixo congelado** e tudo antes do corte: a ordem só muda o trecho que a carga refaz,
  antes de os elementos existirem;
- **a tabela de tipos:** a coleta por corte e o descarte dos parâmetros de tipo de dono solto
  continuam valendo sem mudança.

Efeito esperado: a próxima mudança dos mesmos arquivos recarrega só eles e o que depende
deles:
- a onda: só os 40 gerados;
- a mudança de API: o arquivo e os ~260 dependentes.

**Etapa 2 (projeto, não feita nesta rodada): recarga no lugar com ids estáveis.** Para a
mudança de API custar como no Dart, só o arquivo mudado e os dependentes que a análise por
nome marca deveriam ser refeitos. O conjunto S é o arquivo mudado mais esses dependentes
(no caso medido, o arquivo e os *barrels*). O plano:
1. **Ids por *slot*:** cada declaração de uma biblioteca de S guarda o *slot* (o id) que tinha,
   casada por espécie, dono e nome. Uma declaração nova toma um *slot* livre ou entra no fim
   da arena. Uma removida vira *slot* morto: elemento vazio e sem nome, que nenhum escopo
   alcança. É o mesmo esquema dos parâmetros de tipo da décima primeira rodada.
2. **Refazer só S:** as unidades, os escopos e os *namespaces* das bibliotecas de S, e as
   entradas do `OutlineTypes` dos elementos delas: assinatura, tipo declarado ou inferido,
   hierarquia das classes de S. As bibliotecas fora de S citam elementos de S só por ids que
   não mudaram, e a análise por nome garante que elas não veem o que mudou.
3. **Tabela de tipos:** um `ClassId` estável mantém os tipos que o citam. Os tipos que só
   citavam *slots* mortos saem na coleta seguinte; os parâmetros de tipo de dono morto já
   saem pela regra da rodada 11.
4. **Corte e *checkpoint*:** uma biblioteca de S que ganhou um *slot* no fim da arena quebra a
   regra "tudo depois do *checkpoint* é de bibliotecas depois do corte". A próxima carga
   estrutural (diretivas mudadas, lote novo) volta a cortar no início do pacote, e a ordem é
   refeita. Enquanto isso, só as recargas no lugar valem.
5. **Hierarquia:** as subclasses fora de S de uma classe de S citam o nome dela, então a
   análise por nome já as põe em S.

O critério de aceite é o de sempre: cada passo de `typed_incremental` publica o mesmo que uma
análise do zero, e o oráculo fica a 100%.

##### Resultado

**1. As trocas entre processos (medidas antes de cortar).** Com `DARTFORGE_LSP_TRACE=1`, o
servidor, o trabalhador tipado e o residente imprimem `[trace] <µs> <evento>` a cada
passagem. O *driver* marca a última notificação da onda e as publicações no mesmo relógio
(`2bb97f49`). No `new_sali`, em 20 rodadas com carga ~44, as medianas contadas a partir da
última notificação da onda foram:

| passagem | ms |
|---|---:|
| o trabalhador começa (o *debounce* de 20 ms) | 26 |
| a tarefa chega ao residente | +6 |
| a análise no residente | +512 (206 de CPU) |
| os resultados publicados depois do fim | +1,6 |

As trocas somam ~8 ms; o tempo da onda era o da análise. Por isso não juntamos processos e
cortamos a análise.

**2. O que mudou** (um *commit* cada):
- **A ordem prefere o que muda por último** (`7a67cfc9`, etapa 1 do projeto). O residente
  passa ao *layout* da carga seguinte os arquivos que viu mudar (`Program::layout_last`). A
  onda caiu de 394 bibliotecas recarregadas para 41, e a mudança de API de 739 para 409.
- **Uma mudança só no disco não cancela mais a análise** (`d0138bcb`). A onda chega em vários
  lotes de `didChangeWatchedFiles`, e cada lote cancelava a análise em curso. A análise
  cancelada perdia o mundo do residente, e a carga seguinte voltava ao prefixo congelado
  (2.462 bibliotecas, p90 da onda 1,7 s). Agora a análise de uma raiz sujada só por `.dart` no
  disco termina e publica, e a raiz é analisada de novo. Edição, configuração e fechamento
  continuam cancelando na hora.
- **Os índices das constantes são feitos por unidade, sob demanda** (`7d7e18d0`). A
  verificação de ciclos fazia os índices do programa inteiro a cada análise (~120 ms por
  onda).
- **O índice de elementos sobrevive ao corte** (`0aa2712c`). `Program::truncate` jogava fora
  o `ElementIndex`, e o primeiro verificador de cada carga o refazia sobre 140 mil elementos.
  Agora o corte guarda as listas do que fica (`Program::index_kept`). O próximo `index` só
  acrescenta o que foi carregado depois, na mesma ordem de um índice feito do zero; o teste do
  prefixo congelado compara os dois.
- **A classe de um construtor é buscada nas funções da unidade** (`a0724ed3`). O verificador
  de constantes percorria todas as funções do programa para cada construtor `const`: ~40 ms
  numa onda de 40 gerados.
- **`computeSimplyBounded` roda só sobre as declarações novas** (`6793199c`). As declarações
  antes do corte não citam nada depois dele.
- **A inferência de *overrides* roda só depois do corte** (`d8be1447`). Cada carga inferia de
  novo os *overrides* de todas as classes do pacote, o que era 15% do tempo do residente numa
  sessão longa. A inferência de corpos muda o *outline* no lugar, então as classes mantidas já
  têm os tipos que ela deu, e ela não reporta nada.
  - Quando o *outline* mantido não foi inferido (uma análise parou antes), ela começa no fim
    do prefixo congelado, como antes.
  - O teste do residente com mudança repetida ganhou uma biblioteca mantida cujo erro depende
    de um *override* inferido.
- **Menos trabalho fixo por carga** (`928e899d`). Só uma carga do zero abre o *cache* do SDK.
  A chave de cada unidade é emprestada quando o caminho já é chave.

**3. Sessão longa no macOS, 150 rodadas (510 cargas), os dois servidores revezados.** São
medianas até assentar, em ms, com a CPU entre parênteses. A carga da máquina é a média de
1 minuto durante cada par.

| carga | DartForge r12 (carga 58–63) | Dart r12 | DartForge agora (carga 41–46) | Dart (carga 41–46) | DartForge agora (carga 2–9, 2 sessões) | Dart (carga 2–9, 2 sessões) |
|---|---:|---:|---:|---:|---:|---:|
| onda de 40 `.g.dart` | 588 (186) | 476 (216) | **208 (107)** | 311 (216) | **79–83 (69–70)** | 102–103 (139) |
| edição de corpo | 118 (36) | 136 (38) | **86 (29)** | 91 (35) | 40–43 (17) | **30–31** (17) |
| mudança de API | 795 (185) | 420 (103) | 573 (139) | **250** (93) | 101–105 (81–82) | **90–92** (59–61) |

O resultado por carga:
- **Onda:** ganhamos nas duas condições. Com a máquina carregada, levamos 208 ms contra
  311 ms; com ela calma, 79–83 ms contra 102–103 ms. Gastamos metade da CPU do Dart.
- **Mudança de API:** com a máquina calma ficamos a 10–15% do Dart. Com ela carregada,
  ainda ficamos 2,3× atrás no relógio, porque recarregamos e resolvemos de novo o *outline*
  de 409 bibliotecas.
  - Com a máquina calma, o diagnóstico do próprio arquivo sai em 99 ms contra 29 ms. O
    resto dos dependentes assenta junto.
  - A etapa 2 (recarga no lugar com ids estáveis) é o que falta para ganhar aqui. Ela não foi
    feita.
- **Edição de corpo:** com a máquina carregada, empatamos ou ganhamos. Com ela calma,
  perdemos por ~12 ms. O diagnóstico do arquivo sai antes (18 contra 24 ms), mas a análise
  tipada no residente custa ~40 ms de ponta a ponta.

Outros dados da sessão:
- **Publicações:** a última publicação dos 492 arquivos (9.763 diagnósticos) é igual entre as
  sessões do binário final e à do binário da décima segunda rodada. Esta última comparação foi
  feita antes da queda de energia; o binário é o mesmo.
- **Carga anotada:** a sessão calma do DartForge sozinha, logo depois de a máquina religar
  (carga 5–19), deu onda 78 ms, corpo 44 ms e API 109 ms. A do Dart que veio depois pegou a
  carga subindo para 51 e não entrou na tabela.

**4. Windows (i3, 2 núcleos a 1,2 GHz, parado; antes da reserva da máquina para o agente de
corretude).** Cópia do `new_sali` (`frontend` e `core`) com o SDK 3.6.2, 150 rodadas
revezadas:

| carga | DartForge agora | Dart |
|---|---:|---:|
| onda | 520 | 370–587 |
| edição de corpo | 125 | 61–62 |
| mudança de API | 505 | 282–600 |

- **No i3 perdemos.** Os diagnósticos publicados são idênticos aos do Dart (42 arquivos,
  80 diagnósticos).
- **A análise fica ~3× mais lenta a partir da quarta onda.** A terceira onda leva 52 ms de
  carga, 43 de *outline* e 49 de corpos. Da quarta em diante, com o mesmo corte, são 145, 134
  e 99–119 ms, e a edição de corpo passa de 13 para 38–50 ms.
  - Não é a frequência do processador (fixa em 1,2 GHz) nem o antivírus.
  - No macOS isso não acontece.
- **Experimento com outro alocador, fora do repositório:** o servidor ligado ao `mimalloc`
  ficou 19–21% mais rápido nas três cargas (onda 517–524 → 420–421 ms, corpo 125–126 →
  95–101 ms, API 536–584 → 443–445 ms; 20 rodadas, duas sessões de cada). Parte da perda vem do *heap* do Windows; o resto não foi achado.
- **Decisão pendente:** adotar um alocador próprio no Windows é uma dependência nova e fica
  para o proprietário decidir.

**5. Memória** (*footprint* de todos os processos, em MiB, macOS):

| | DartForge | Dart |
|---|---:|---:|
| recém-aberto (só abrir) | 478,3 | 571,9 |
| recém-aberto + 1 rodada | 544,5 | 460,7 |
| fim da sessão longa (1 min quieto), máquina calma | **564,3** | 655,9 |
| pico, máquina calma | 568,3 | 692,7 |
| inclinação por 100 cargas, nas 4 sessões | −1,0; −0,2; +1,2; +1,5 | +36 a +40 |
| dado vivo do residente, 1 rodada → fim | 474,2 → 474,5 (**+0,06%**) | — |

- O dado vivo não cresce.
- A inclinação do *footprint* oscila em torno de zero: −1,0 e −0,2 nas duas sessões calmas
  revezadas, +1,2 e +1,5 em outras duas. Nas positivas, a curva sobe em degraus de 1–3 MiB
  (picos do alocador nas mudanças de API) e fica plana entre eles.
- As curvas estão em `bench/results/lsp-long-session/*-r23-*.csv`.

**Validação** (binário final, depois da queda de energia):
- testes em *release* de `elements`, `types`, `analysis`, `world`, `emit_js`,
  `emit_js_production`, `parity`, `lsp` e `dev`: 848 passaram e nenhum falhou (com o `node`
  no `PATH`);
- `typed_incremental` repete cada edição de API (subtipo selado novo, membro removido da
  base, tipo inferido que volta, o modelo) e publica o mesmo que uma análise do zero;
- oráculo de paridade nos quatro projetos (35 arquivos, 398 posições):
  - `references`, `implementation` e `definition` a 100%;
  - *completion* top-5 a 100% e top-1 a 99%;
  - o `app` com erros comuns foi recriado depois da queda de energia, porque as cópias em
    `/private/tmp` se perderam;
- clippy `-D warnings` em `elements`, `types`, `analysis`, `parity` e `lsp` (todos os alvos),
  e `rustfmt` nos arquivos tocados.

**O que não foi feito nesta rodada:**
- **A etapa 2, a identidade estável de elementos.** Ela ficou só no projeto, e é o que falta
  para a mudança de API ganhar do Dart com a máquina carregada.
- **A edição de corpo com a máquina calma:** ainda ~12 ms atrás do Dart.
- **O Windows:** perdemos nas três cargas num i3. A lentidão depois da quarta onda tem uma
  parte no alocador; o resto não foi achado.

#### Décima quarta rodada (2026-11-02): a etapa 2, recarga no lugar

##### O que a medida mostrou primeiro

**Um erro do *driver*, não do servidor.** A sessão longa marcava o início de uma edição (corpo
ou API) antes de amostrar a CPU do servidor e dos descendentes.
- A amostra roda um `pgrep` por nível de processo e custa 25–35 ms com a máquina calma, mais
  com ela carregada.
- Esse tempo entrava na latência de toda edição de corpo e de toda mudança de API.
- Pesava mais para quem tem mais processos: os três do DartForge (servidor, trabalhador e
  residente) contra um ou dois do Dart.

O cronômetro agora começa depois da amostra (`dacb7794`). As ondas já eram medidas a partir da
última notificação. Os números de corpo e de API das rodadas 12 e 13 carregam esse erro.

**Custos fixos por carga.** A amostragem do residente numa sessão de 100 rodadas achou três:
- **o pacote de cada arquivo** (`4b38b89a`): a busca do pacote de cada diretiva comparava o
  arquivo com o diretório de cada um dos ~150 pacotes; era metade da fase de carga. Agora o
  ancestral mais próximo que é diretório de pacote é procurado num índice;
- **a coleta da tabela de tipos** (`86434ae6`, `287066bb`): rodava a cada carga e custava 14% do
  tempo do residente. Agora roda quando a tabela cresce um oitavo (com mínimos de 4.096 tipos
  ou 256 parâmetros) ou a cada 16 recargas. Cada parâmetro de tipo guarda a época em que foi
  alocado; só morre o que foi alocado antes de uma recarga que substituiu o dono dele. A
  coleta também passou a ser feita no lugar, sem um segundo `lookup` vivo junto com o
  primeiro;
- **as fases 2 e 3 do *outline*** (`ad5050aa`): copiavam o `exported` inteiro de cada import e
  o do `dart:core` para cada biblioteca carregada. Agora leem no lugar e filtram pelos
  combinadores.

Depois da recarga no lugar, uma segunda amostragem achou mais dois custos fixos:
- **as chaves das unidades com diagnósticos de sintaxe** (`cea887c8`): cada unidade mantida
  (~3.000) era comparada com cada chave (~400). Agora a busca é por nome num conjunto, e a fase
  de *outline* de uma onda ou de uma API no lugar caiu de 16–18 ms para 9–10 ms;
- **a varredura do disco** (`8e18855f`): com arquivos mudados no disco, cada análise lia a data de
  cada unidade lida (~3.600 `stat`). Agora o trabalhador do LSP passa ao residente os arquivos
  que o *watcher* notificou (`Resident::disk_changed_only`). A varredura completa ainda roda
  quando a última tem mais de 30 s. A fase de carga de uma onda caiu de 20–22 ms para
  14–17 ms.

##### A recarga no lugar (etapa 2)

O projeto da décima terceira rodada previa *slots* estáveis casados por nome. A implementação
(`28597b79`) seguiu um caminho mais estreito, que aproveita o corte posicional que já existe.

1. **A biblioteca quente vai por último.** Um arquivo cuja API mudou e de que outras bibliotecas
   dependem entra em `Program::layout_hot`; o residente guarda os quatro mais recentes.
   - O `frozen::lay_out` o tira do grupo de bibliotecas que se importam e o põe depois de
     tudo, inclusive dos dependentes.
   - A ordem deixa de ser de dependência só nesse ponto.
   - A próxima mudança desse arquivo corta nele sozinho.
2. **O plano antes da carga** (`in_place::plan`). Quando uma biblioteca mantida aponta para
   depois do corte, a recarga só a mantém se toda biblioteca recarregada para a qual uma
   mantida aponta (uma biblioteca *estável*) tem uma unidade só e elementos contíguos.
   - O plano guarda o que precisa conferir depois: onde os elementos de cada biblioteca
     estável começavam, um descritor de cada elemento, o `exported` e a API do texto antigo.
   - Guarda também o corte de reserva, antes de todo dependente.
   - Os ponteiros das bibliotecas mantidas são soltos antes da carga, porque o `lay_out`
     renumera o que vem depois do corte.
   - A carga recebe os arquivos estáveis como entradas extras, porque as bibliotecas mantidas
     não são percorridas de novo.
3. **A conferência depois da carga** (`in_place::settle`). A recarga mantém os dependentes
   quando:
   - **as bibliotecas estáveis têm os mesmos ids:** cada uma recebeu os elementos de volta nos
     mesmos ids, porque a lista do texto antigo é prefixo da do novo ou o contrário (uma
     declaração acrescentada ou removida no fim, uma assinatura mudada);
   - **nenhum dependente nomeia o que mudou:** nenhuma assinatura de dependente mantido usa um
     nome mudado. Os nomes mudados vêm da comparação entre a `ApiDigest` antiga e a nova. Uma
     classe que só mudou nos membros (com o mesmo cabeçalho) conta apenas nos cabeçalhos de
     classe (`extends`, `implements`, `with`, `on`);
   - **nada mudou que se use sem o nome** (um operador, `call`…), e as diretivas são as mesmas.

   As palavras das assinaturas dos dependentes saem do texto sem os corpos, sem *lexing*
   (`incremental::signature_words`), e ficam guardadas por unidade.
4. **O reparo.** Os ponteiros voltam para os ids novos.
   - Quando um nome ou o id de uma biblioteca estável mudou, o `exported` das bibliotecas que a
     reexportam e os escopos das que importam qualquer uma delas são refeitos
     (`outline::rebuild_scopes`).
   - Se a conferência falha, a carga é refeita a partir do corte de reserva, o mesmo das rodadas
     anteriores. `DARTFORGE_IN_PLACE=0` corta sempre assim.

**O que continua correto.**
- **O prefixo congelado e os elementos antes do corte** não se mexem: o corte continua
  posicional.
- **A tabela de tipos:** a coleta já mantinha os parâmetros de tipo que um tipo mantido ainda
  usa, e a época de cada parâmetro distingue o dono antigo do novo com o mesmo id.

**O que esta forma não cobre** (vai para o corte de reserva):
- uma declaração inserida no meio de um arquivo, que muda os ids depois dela;
- uma biblioteca com *parts*;
- uma mudança que um dependente mantido nomeia na assinatura.

**Os testes.**
- **O teste do residente** (`resident_reloads_in_place_what_its_dependents_do_not_name`)
  edita um arquivo que uma biblioteca importa e um *barrel* reexporta:
  - uma declaração acrescentada, removida, e outra acrescentada são recarregadas no lugar;
  - uma assinatura que um dependente nomeia e um membro que move os ids caem no corte de
    reserva;
  - cada análise publica o mesmo que uma do zero.
- **O `typed_incremental`** acrescenta uma declaração no fim de um modelo, a remove e muda uma
  assinatura que ninguém nomeia. Nesse teste os lotes trocam de imports e o corte cai no
  prefixo, mas cada passo continua publicando o mesmo que uma análise do zero.

##### Edição de corpo

Com o cronômetro corrigido, a edição de corpo custa no residente ~20 ms:
- a troca da unidade: 5 ms;
- os corpos: 3 ms;
- as constantes citadas: 7 ms;
- os verificadores de tipo: 4 ms.

As constantes citadas são as das bibliotecas cujas constantes o componente usa (do `ngdart` e
de outras dependências): ficam no prefixo congelado e não mudam enquanto o mundo vive. As
tabelas de corpo delas agora ficam guardadas até a próxima coleta da tabela de tipos
(`5877ffc4`), e a etapa caiu para 1–2 ms. As publicações de uma sessão de 10 rodadas são as
mesmas (492 arquivos, 9.763 diagnósticos).

**As tabelas de corpo da onda.** O residente faz uma passada de inferência por biblioteca
analisada, e cada passada dava uma tabela a cada unidade do programa: uma onda de 40 arquivos
montava ~126 mil tabelas, e isso custava tanto quanto a inferência.
- Agora uma passada pega as tabelas da anterior (`SharedInference::recycle`, `d85fa3a8`).
- As tabelas escritas voltam a ficar novas (`UnitBodyTypes::is_fresh`); as outras são
  reaproveitadas como estão.
- Os corpos de uma onda caíram de 23–31 ms para 12 ms, e as publicações são as mesmas.

Duas tentativas anteriores não deram ganho e saíram:
- montar as tabelas por cópia de uma tabela vazia;
- montá-las com um *hasher* compartilhado.

O custo era escrever e liberar as tabelas, não pedir os *hashers*.

##### Resultado

**Sessão longa no macOS, 150 rodadas (510 cargas), dois pares revezados (Dart primeiro).**
- A carga ficou em 8–11 durante os quatro pares, porque o `cargo test` da validação v21 estava
  preso: ficou mais de 4 horas a ~390% de CPU, sem escrever no log. Esperei por ele até as 20h45
  e medi assim, com a carga anotada.
- As medianas são até assentar, em ms, com a CPU entre parênteses.

| carga | DartForge (par a) | DartForge (par b) | Dart (par a) | Dart (par b) |
|---|---:|---:|---:|---:|
| onda de 40 `.g.dart` | **101 (75)** | **100 (69)** | 130 (294) | 142 (338) |
| edição de corpo | **41 (32)** | **42 (32)** | 47 (39) | 48 (40) |
| mudança de API | **64 (57)** | **65 (53)** | 148 (119) | 161 (116) |

- **Mudança de API:** ganhamos por 2,3–2,5× no relógio e gastamos ~2× menos CPU.
  - Na rodada 13, com a máquina calma, eram 101–105 ms contra 90–92 ms. Com ela carregada,
    573 ms contra 250 ms, e esse número ainda carregava o erro do *driver*.
  - Numa sessão com o tempo das fases (`memt`), 59 das 60 mudanças de API recarregaram uma
    biblioteca só.
  - O diagnóstico do próprio arquivo sai em 53–57 ms contra 44 ms. O Dart publica o arquivo
    antes dos dependentes; nós publicamos tudo junto e assentamos antes.
- **Edição de corpo:** ganhamos por 6–7 ms. O diagnóstico de sintaxe do arquivo sai em 1–2 ms
  contra 38–39 ms.
- **Onda:** ganhamos por 22–30%, com um quarto da CPU do Dart.

Outros dados da sessão:
- **Antes da correção do erro**, numa sessão do DartForge sozinho com a carga em 19–37: onda
  86 ms, corpo 34 ms, API 56 ms.
- **Publicações:** a última publicação dos 492 arquivos (9.763 diagnósticos) é a mesma nas
  duas sessões do DartForge. Também é a mesma da comparação com o binário anterior, feita a cada
  passo (`cmp_lsp14`).

**Memória** (*footprint* de todos os processos, em MiB):

| | DartForge | Dart |
|---|---:|---:|
| recém-aberto (só abrir), em repouso | 482,0 | 461,0 |
| recém-aberto + 1 rodada, em repouso | 539,5 | 477,8 |
| fim da sessão longa (1 min quieto), pares a e b | **569,8; 570,4** | 701,4; 707,4 |
| pico, pares a e b | 569,8; 573,6 | 724,7; 715,4 |
| inclinação por 100 cargas | +2,1; −0,1 | +34,2; +36,4 |
| dado vivo do residente, 1 rodada → fim | 474,2 → 476,4 (**+0,46%**) | — |

- **Os degraus nas mudanças de API sumiram com a causa.** Na rodada 13, eram os picos das
  recargas de 409 bibliotecas a cada mudança de API. Agora essas mudanças recarregam uma
  biblioteca.
  - Na sessão `memt`, 59 das 60 mudanças de API e 145 das 150 ondas foram no lugar; as outras
    ondas cortaram nos 41 gerados, sem dependente mantido apontando para depois do corte.
  - O *footprint* ainda sobe em degraus de 2–6 MiB em cargas sem nada de especial e devolve
    memória de vez em quando: −0,6 por 100 cargas nessa sessão, 0,0 em outra.
  - O dado vivo é o mesmo nas três sessões (476,4 MiB): os degraus que restam são do alocador,
    não retenção.
- **Os +2,2 MiB de dado vivo** são as palavras das assinaturas dos dependentes e as tabelas de
  corpo das constantes citadas, guardadas de propósito.
- **As curvas** estão em `bench/results/lsp-long-session/*-r24-*.csv`.

**Validação** (binário final):
- **testes:** em *release* de `elements`, `types`, `analysis`, `world`, `emit_js`,
  `emit_js_production`, `parity`, `lsp` e `dev`, com o `node` no `PATH`: 850 passaram e nenhum
  falhou;
- **`typed_incremental`:** cada passo publica o mesmo que uma análise do zero;
- **teste do residente:** a recarga no lugar mantém os dependentes quando deve, cai no corte de
  reserva quando deve, e publica o mesmo que uma análise do zero;
- **oráculo de paridade:** a mesma tabela da rodada 13 nos quatro projetos (35 arquivos, 398
  posições). `references`, `implementation` e `definition` ficam a 100%; *completion* top-5 a
  100% e top-1 a 99%;
- **clippy e formatação:** clippy `-D warnings` em `elements`, `types`, `analysis`, `parity` e
  `lsp` (todos os alvos), e `rustfmt` nos arquivos tocados.

**O que não foi feito nesta rodada:**
- **A recarga no lugar casando por nome:** uma declaração inserida no meio de um arquivo, ou
  um arquivo com *parts*, ainda cai no corte de reserva. Os *slots* casados por nome do projeto
  da rodada 13 resolveriam isso.
- **Um par com a máquina realmente calma ou muito carregada:** a carga ficou em 8–11 durante
  as sessões finais. O par com a carga acima de 40 da rodada 13 não foi repetido com o
  cronômetro corrigido.
- **O Windows:** a máquina está reservada para o agente de corretude. A lentidão do i3 e a
  questão do `mimalloc` ficaram anotadas para o proprietário.

#### Décima quinta rodada (2026-11-03): o índice de referências resolvidas

##### O que a medida mostrou primeiro

**A busca de antes no `new_sali/frontend`.** O *driver* (`editing_latency`) pede
`references` em 8 identificadores do `processo_filtros_component.dart`: 3 vezes logo depois de
uma edição de corpo e 16 sem edição (carga 6–15):

| `references` | DartForge antes | Dart |
|---|---:|---:|
| `String` (53.897 locais), 1º depois de edição | 9,8 s | 7,7 s |
| `String`, sem edição | 9,1 s | 1,4 s |
| campo privado (4 locais) | 2,0 s | 11–20 ms |
| mediana sem edição | 2,0 s | 15 ms |

- Toda busca não local fazia uma carga ampla num processo filho (o SDK e os pacotes que citam
  o nome): ~2 s até para 4 locais.
- **A sessão semântica não guardava a biblioteca do documento.** O programa de um componente
  do frontend (2.835 bibliotecas) passou do orçamento de 32 MiB de fonte. Cada `hover`,
  `definition` ou `prepareRename` recarregava tudo (~950 ms), e as cargas passageiras deixavam
  o servidor em 830 MiB com as páginas do alocador. Com o programa guardado, ficava em
  570 MiB.

##### O índice (`crates/lsp/src/reference_index.rs`)

**Identidade.** Uma chave é o URI da biblioteca mais o caminho da declaração dentro dela:
`dart:core#CString`, `…#CString.gisNotEmpty`, `…#CModel.knamed`. Ela sobrevive às recargas,
porque não usa ids densos. Duas declarações com a mesma chave (uma declaração duplicada)
tornam a chave *ambígua*, e a busca que a toca volta ao caminho antigo.

**O que cada arquivo guarda.** O registro é calculado sobre um programa com os corpos do
arquivo inferidos, com **as mesmas funções** da busca:
- **as relações:** cada nome resolvido de expressão, anotação de tipo, metadado, construtor
  escrito com o nome da classe e `show`/`hide`, e o que cita construtores e parâmetros
  `this.x`/`super.x`;
- **as declarações** (faixa do nome) e os fatos de classe que a família de um membro e o
  `implementation` pedem: supertipos, supertipos escritos, membros por nome e espécie, campos,
  `this.x` dos construtores, *mixins*;
- **as referências `[ref]` da documentação**, cada prefixo resolvido;
- **os `referencedNames`** (os arquivos candidatos de `_addResults`) e a tabela de linhas (as
  localizações saem sem reler o arquivo);
- **nos arquivos do projeto**, o que o `identify` diz de cada identificador, já em chaves; para
  o que só o arquivo tem (local, parâmetro de tipo, prefixo), a resposta inteira; e a resposta
  do `prepareRename` em cada identificador.

**Uma implementação só.** A busca de construtores e a parte `this.x` da busca de um campo
viraram varreduras de uma unidade para todos os construtores de uma vez
(`unit_constructor_uses`, `unit_parameter_uses`). A busca antiga e o índice usam as mesmas
varreduras. O mesmo vale para os supertipos escritos (`written_supertypes`), o `this.x` citado
na documentação (`documented_field_parameter`) e o pivô do `implementation`
(`implementation_pivot`).

**A pergunta.**
- A busca lê o registro do documento: o fato do identificador dá o alvo, a família de um
  membro é percorrida sobre os fatos de classe, e os usos são as relações dos arquivos
  candidatos. Nada carrega a biblioteca do documento.
- `implementation` percorre os subtipos escritos dos fatos de classe com o pivô do `identify`.
- `prepareRename` lê a resposta guardada.
- Um identificador sem fato vai pelo caminho antigo. É o caso do parâmetro nomeado, cuja busca
  alcança rótulos de outros arquivos.

**Ciclo de vida.**
- **Construção:** a primeira busca de uma sessão sem índice dispara a construção num processo
  filho (`--reference-index`). Ele faz a carga ampla e infere, registra e solta cada biblioteca
  por vez. No `new_sali/frontend` leva 45 s de CPU e chega a 1,1 GB no filho; o arquivo tem
  71 MB. Enquanto isso, a busca segue pelo caminho antigo.
- **Entre sessões:** o índice fica no *cache* do usuário (`~/Library/Caches/dartforge/lsp`). A
  sessão seguinte o lê se os arquivos do SDK e dos pacotes têm a mesma data e o mesmo tamanho.
- **Antes de cada resposta:** os arquivos do projeto cujo registro não é do texto atual são
  registrados de novo. A biblioteca do documento vem da sessão, quando ela a tem; o resto
  (arquivos novos, os dependentes de uma mudança de API) vem de um filho.
  - O projeto inteiro só é comparado com o disco quando o *watcher* avisou de uma mudança, ou a
    cada 30 s (a regra da varredura do residente); senão, só os documentos abertos.
  - Um arquivo do programa fora da árvore do projeto (os gerados em `.dart_tool/build`) só
    conta como apagado se sumiu do disco. Antes disso, cada sessão via os 224 dependentes
    deles como desatualizados.
- **Filho órfão:** um filho cujo servidor morreu para (antes, um ficou 4 min queimando CPU).

**Custos cortados no caminho.**
- O `identify` acha a expressão de um nome por um mapa da unidade, não varrendo todas as
  expressões.
- A documentação vem do *cache* da unidade, sem um *lexing* do arquivo a cada chamada; o
  mesmo na busca de prefixos.
- `node_constructor` olha o índice de elementos, não todas as funções.
- A resposta de um prefixo é compartilhada entre os usos do mesmo tipo.
- As cadeias do índice ficam guardadas uma vez só, por *hash*.

**Orçamento da sessão: 32 → 48 MiB de fonte** (`DEFAULT_BUDGET_MIB`). Ver a primeira seção.

##### Igualdade

`crates/lsp/examples/reference_index_check.rs` compara, em todo identificador de cada arquivo
de um projeto (uma posição por alvo e arquivo), a resposta do índice com a da busca sem ele:
`references`, `implementation` e `prepareRename`.

A busca sem o índice roda com o projeto inteiro carregado, como na sessão do oráculo:

| projeto | alvos comparados | `references` iguais | `implementation` iguais | `prepareRename` iguais | parâmetros nomeados (caminho antigo) |
|---|---:|---:|---:|---:|---:|
| `args` 2.7.0 | 775 | 775 | 775 | 775 | 100 |
| `path` 1.9.1 | 903 | 903 | 903 | 903 | 33 |
| `string_scanner` 1.4.1 | 416 | 416 | 416 | 416 | 25 |
| `app` (erros comuns) | 25 | 25 | 25 | 25 | 1 |

- **O que o oráculo achou primeiro:** a comparação de antes carregava só a biblioteca do
  documento para a busca antiga.
  - O oráculo achou duas posições do `args` em que o índice não tinha os locais de tamanho zero
    dos argumentos de um parâmetro posicional opcional e de um `super.x` em `parser.dart`.
  - A resposta de um local tinha sido guardada sem os corpos dos outros arquivos.
- **Correção:**
  - o índice grava o início de todo argumento posicional opcional de uma chamada, não só o do
    `this.x` de um construtor. O fato de um parâmetro posicional opcional nomeia essas
    relações, e a resposta as lê de todos os arquivos;
  - um parâmetro nomeado vai pelo caminho antigo (rótulos de outros arquivos, homônimos das
    sobrescritas). Esse caminho, como o de qualquer parâmetro sem fato, agora carrega o projeto
    inteiro. Com só a biblioteca do documento, os argumentos e os rótulos dos outros arquivos
    faltavam, a menos que a sessão tivesse o projeto por acaso: um defeito anterior, que o
    índice deixou à mostra;
  - a comparação agora carrega o projeto inteiro;
  - o formato do *cache* subiu para recusar os índices antigos.

O teste `crates/lsp/tests/reference_index.rs` faz o mesmo num projeto com construtores
nomeados, redirecionados e implícitos, `this.x` e `super.x`, sobrescritas em arquivos
diferentes, documentação, combinadores e extensões, antes e depois de uma edição de corpo e de
uma mudança de API.

##### Os pedidos no `new_sali/frontend`

Mesmas 8 posições do `processo_filtros_component.dart`. Cada pedido vai 3 vezes logo depois
de uma edição de corpo (cada edição diferente das anteriores, `--methods`) e 16 vezes sem
edição. O DartForge usa o índice do *cache* (uma sessão antes o construiu). Mediana em ms, com
o pior caso entre parênteses; carga 20–27.

| pedido | DartForge antes (carga 6–15) | DartForge agora | Dart |
|---|---:|---:|---:|
| `references`, sem edição | 2.044 (9.163) | **3,7 (106)** | 20,7 (3.339) |
| `references`, depois de edição | 2.033 (9.764) | 289 (468) | **9,0** (8.662) |
| `implementation`, sem edição | — | **3,1 (46)** | 12,5 (196) |
| `implementation`, depois de edição | — | 44,9 (46) | **4,5** (68) |
| `prepareRename`, sem edição | ~950 | 3,2 (32) | **0,8** (1,3) |
| `prepareRename`, depois de edição | ~950 | 25,7 (36) | **1,8** (7,8) |
| `hover`, sem edição | 964 | 10,7 (11,5) | **1,1** (3,6) |
| `hover`, depois de edição | 966 | **11,5** (1.185) | 35,7 (63) |
| *footprint* em repouso, todos os processos (MiB) | 824 | 1.082 | **798** |
| pico, todos os processos (MiB) | 1.836 | 1.221 | **925** |

O resultado por pedido:
- **`references`:** ganhamos sem edição (3,7 contra 20,7 ms) e no pior caso (`String`: 106 ms
  contra 3,3 s; depois de edição, 468 ms contra 8,7 s).
  - Perdemos a mediana depois de uma edição: o registro do documento é refeito pela sessão
    (~25–45 ms; a primeira edição de uma sessão carrega a biblioteca, ~1,2 s).
- **`implementation`:** ganhamos sem edição e perdemos depois de edição, pelo mesmo motivo.
- **`prepareRename` e `hover`:** ainda perdem por alguns ms. Antes passavam de 900 ms.
- **Memória:** o servidor guarda a biblioteca do documento (~380 MiB) e o índice; o residente,
  ~512 MiB. O Dart, um processo só, fica em 798 MiB. Perdemos 280 MiB nesta sessão de pedidos.
  Na sessão longa sem pedidos, que não carrega nem a sessão nem o índice, ganhamos.
- **Sessão sem *cache*:** as primeiras buscas seguem pelo caminho antigo enquanto o filho
  constrói o índice (até 30 s com a carga, pico de 1,1 GB no filho).
  - Depois disso, `implementation` e `prepareRename` já respondem do índice.
  - O Dart usa o *cache* em disco dele da mesma forma.

##### A sessão longa (macOS)

**Itens 1 e 4: 150 rodadas (510 cargas), dois pares revezados** (o Dart primeiro no par a).
A carga é a média de 1 minuto no início e no fim de cada sessão; a máquina é compartilhada com
a validação e o agente de corretude, e nunca ficou calma (30–59). As medianas são até
assentar, em ms, com a CPU entre parênteses.

| carga | DartForge (par a, carga 52–30) | DartForge (par b, 30–59) | Dart (par a, 48–52) | Dart (par b, 59–20) |
|---|---:|---:|---:|---:|
| onda de 40 `.g.dart` | **79 (57)** | **72 (52)** | 155 (247) | 141 (390) |
| edição de corpo | **33 (26)** | **31 (25)** | 39 (34) | 73 (72) |
| mudança de API | **51 (46)** | **52 (44)** | 130 (89) | 188 (154) |

- Ganhamos nas três cargas nos dois pares, no relógio e na CPU.
- A edição de corpo do par a é a mais apertada: 33 contra 39 ms, com 26 contra 34 ms de CPU.

**Item 3: os degraus de memória.**
- O dado vivo do residente é o mesmo de antes (476,4 MiB nas duas sessões), e os degraus são
  páginas que o alocador guarda depois dos picos.
- **Devolver as páginas livres:** depois de cada análise, já com a resposta enviada, o residente
  pede ao alocador do sistema que devolva as páginas livres (`malloc_zone_pressure_relief`,
  macOS; `DARTFORGE_LSP_RELIEF=0` desliga).

| *footprint* de todos os processos | com devolução (par a) | com devolução (par b) | sem devolução | Dart (a; b) |
|---|---:|---:|---:|---:|
| na carga ~50 → na 510 (MiB) | 568 → 575 | 578 → 583 | 558 → 576 | 566 → 752; 526 → 606 |
| inclinação por 100 cargas | +1,2 | +0,8 | +4,6 | +39,8; +17,4 |
| pico (MiB) | 575,7 | 582,8 | 576,3 | 760,0; 615,7 |

- A devolução não muda os tempos (sem ela: 71, 31 e 48 ms).
- A inclinação cai de +4,6 para +0,8 a +1,2, mas **não chega a ≤ 0** em todas as sessões. No
  par b a curva fica plana da carga 200 até o fim; no par a ainda sobe em degraus de 1–2 MiB.
- O que resta não está no residente: é o *footprint* do servidor e do trabalhador, que não
  pedem a devolução. Não foi atacado nesta rodada.
- As curvas estão em `bench/results/lsp-long-session/*-r25-*.csv`.

##### Windows: o alocador (decisão do proprietário)

Na mesma máquina da rodada 13 (i3-1215U), compartilhada com o agente de corretude:
- **cópia do `new_sali`** (frontend e core, com os 117 pacotes e um `package_config.json`
  reescrito) em `E:\Work\df-lsp`;
- **antes de cada sessão**, o script esperava não haver `cargo`, `rustc` ou `link` rodando;
  nenhum rodou durante as sessões. Mas a CPU ficou em 48–100% de carga o tempo todo.
- **memória:** o *driver* só mede memória no macOS; no Windows, um processo à parte amostrou os
  bytes privados da árvore do servidor a cada segundo.

**1. Com o alocador do sistema** (o código desta rodada, com o cronômetro corrigido; 150
rodadas, dois pares revezados):

| carga (mediana até assentar) | DartForge (a; b) | Dart (a; b) |
|---|---:|---:|
| onda | **373; 358 ms** | 882; 540 ms |
| edição de corpo | 143; 139 ms | **76; 75 ms** |
| mudança de API | **270; 267 ms** | 5.695; 408 ms |
| bytes privados no fim | **370; 370 MiB** | 1.668; 728 MiB |
| inclinação por 100 cargas | **+2,8; +0,2** | +177,5; +47,1 |

O DartForge ainda perde a edição de corpo. Por isso vale o passo 2.

**2. Com o `mimalloc`** (só no binário `dartforge-lsp`, só no Windows, `cfg(windows)`):

| carga | DartForge com mimalloc (a; b) | Dart (a; b) |
|---|---:|---:|
| onda | **317; 317 ms** | 572; 540 ms |
| edição de corpo | 124; 125 ms | **75; 75 ms** |
| mudança de API | **232; 234 ms** | 626; 455 ms |
| bytes privados no fim | 561; 563 MiB | 723; 724 MiB |
| inclinação por 100 cargas | +0,5; +2,2 | +43,1; +42,9 |

**Decisão: o `mimalloc` não foi adotado.** O experimento (`f9f28d17`) foi revertido em
`21843d27`.
- O critério do proprietário era manter a inclinação no máximo igual à do alocador do sistema
  e o dado vivo igual.
- **Ganho:** com o `mimalloc` as cargas ficaram ~12% mais rápidas.
- **Por que não serve:**
  - a inclinação não ficou abaixo da do sistema nos dois pares (+2,2 contra +0,2 no par b);
  - os bytes privados ficaram ~190 MiB acima (561–563 contra 370 MiB no fim; pico 630 contra
    413 MiB);
  - a edição de corpo continua perdendo para o Dart com os dois alocadores.
- **Dado vivo:** no Windows não há estatística do alocador. A contagem das estruturas do censo
  do residente é a mesma (141.808 elementos, 62.543 nomes; a tabela de tipos varia com a
  coleta, 45.841–46.551).

##### Validação (binário final)

- **Oráculo de paridade** nos quatro projetos (35 arquivos, 398 posições), duas vezes:
  - `references`, `implementation` e `definition` a 100%;
  - *completion* top-5 a 100% e top-1 a 99%;
  - a mesma tabela da rodada 14.
- **Igualdade do índice:** a tabela acima (2.119 alvos).
- **Testes em *release*** de `elements`, `types`, `analysis`, `world`, `emit_js`,
  `emit_js_production`, `parity`, `lsp` e `dev`, com o `node` no `PATH`: 852 passaram e nenhum
  falhou.
  - Na primeira volta, 26 testes de `emit_js` e `dev` falharam porque o `runtime/ddc/dart_sdk.js`
    (gerado, fora do controle de versão) não estava neste *worktree*.
  - Com o arquivo copiado, os dois *crates* passaram (53 de 53).
- **Clippy e formatação:** `cargo clippy --workspace --all-targets -- -D warnings` (com o LLVM
  22.1.8) e `cargo fmt --all -- --check` limpos.
- **`reference_index`:** o teste novo e o dos `references` passam.

##### O que não foi feito nesta rodada

- **O registro do documento depois de uma edição** sai da sessão, ~25–45 ms, ou de um filho,
  ~1,3 s, quando a biblioteca não cabe no orçamento.
  - Por isso a primeira busca depois de uma edição ainda perde para o Dart nas buscas pequenas.
  - O residente já infere a biblioteca editada. O próximo passo é ele fazer o registro e
    mandá-lo junto com os diagnósticos.
- **O parâmetro nomeado** ainda vai pelo caminho antigo (rótulos e homônimos das
  sobrescritas).
- **`prepareRename` e `hover`** ainda ficam alguns ms atrás do Dart, e o servidor guarda a
  biblioteca do documento (~380 MiB) que o residente também tem. Responder do residente é o
  passo seguinte.
- **A recarga no lugar casando os elementos por nome** (item 1): não começou. O projeto da
  rodada 13 continua válido.
- **A inclinação ≤ 0 em toda sessão longa** (item 3): com a devolução das páginas, a
  inclinação ficou em +0,8 e +1,2 MiB por 100 cargas. O que sobra é o *footprint* do servidor e
  do trabalhador.
- **Windows:** a edição de corpo ainda perde (125–143 contra 75 ms) com os dois alocadores. A
  causa não foi achada.

#### Décima sexta rodada (2026-11-03): o registro do documento no residente

##### O que a medida mostrou primeiro

Mesmas 8 posições do `processo_filtros_component.dart` da rodada 15. O *driver* agora declara,
como o VS Code, que o cliente observa os arquivos (`workspace.didChangeWatchedFiles.
dynamicRegistration`); o binário da rodada 15 (`base`) foi medido com o mesmo *driver*.

- **`hover` sem edição (10 ms):** a amostragem do servidor (`sample`) deu 85% do tempo em
  `session::disk_equal`: a cada consulta, a sessão comparava com o disco a data e o tamanho de
  cada arquivo que a carga leu (~2.000 `stat`). A resposta em si leva ~0,2 ms.
- **`prepareRename` sem edição (3 ms):** a atualização do índice antes de cada busca percorria o
  caminho de todo registro do projeto e montava os importadores de toda biblioteca, mesmo sem
  mudança de API.
- **`references` e `implementation` depois de uma edição:** o registro do documento saía da
  sessão (~25–45 ms) e, depois de qualquer registro novo, a busca seguinte refazia as tabelas
  derivadas do índice inteiro (palavras, declarações, classes de ~4.000 arquivos, ~20 ms).

##### O registro do documento no residente (item 1)

- **O pedido.** Enquanto o servidor tem um índice (`ResidentRecords::want`), o trabalhador pede
  ao residente os registros dos documentos abertos que mudaram (`Job::records`).
- **O residente.** Com `Resident::keep_bodies`, a análise guarda as tabelas de corpo das
  bibliotecas do lote até a próxima. Depois de escrever os diagnósticos, o residente empresta a
  análise (`Resident::lend_analysis`: programa, *interner*, tabela de tipos, *outline* e corpos)
  ao construtor do índice (`reference_index::records_of_analysis`) e manda os registros
  codificados numa segunda linha (`Response::records`, em hexadecimal). Os diagnósticos não
  esperam pelo registro.
- **O servidor.** Antes de uma busca, `absorb_resident_records` junta os registros que chegaram;
  se um registro do texto atual de um documento aberto ainda está sendo feito, espera por ele
  (até 3 s). Um registro que muda a API do arquivo marca os dependentes para registro, como antes.
- **O que fica entre empréstimos** (`ResidentCache`): os nomes referenciados das unidades que não
  mudaram e os subtipos escritos (`Project::written_subtypes_after` recoloca só as classes das
  unidades trocadas). Cada mundo carregado tem uma geração, e uma troca de corpos anota as
  unidades trocadas. Sem esse *cache* o registro levava 0,5–1,3 s (os nomes referenciados de todo
  o programa, para o `Object` implícito das famílias de membros); com ele, ~20 ms.
- **O SDK do residente tem os *patches* do DDC** (o motor de `dartforge analyze`); o servidor vê
  o SDK do analisador, sem eles. Por isso:
  - um membro *patcheado* recebe a chave da declaração (`Project::patch_keys`: sem `!p`);
  - um alvo declarado no SDK recebe as localizações do registro que o servidor já tinha do
    arquivo (`ReferenceIndex::adopt_sdk_locations`); um alvo novo do SDK perde o fato, e a busca
    o identifica como antes.
- **Tabelas derivadas no lugar** (`Derived::replace`): um registro do mesmo arquivo com as mesmas
  chaves e os mesmos fatos de classe (edição de corpo) atualiza as palavras e as faixas das
  declarações; o resto ainda refaz as tabelas.
- **`prepareRename` depois de uma edição:** enquanto o registro é feito, a sessão já tem a
  biblioteca com o texto novo (a edição trocou a unidade no lugar) e responde na hora.

##### Um defeito do residente achado no caminho

A comparação do índice com registros do residente (`DARTFORGE_INDEX_CHECK_RESIDENT=1`, cada
arquivo analisado duas vezes, com uma edição de corpo e com o texto) achou, no `args`,
`Undefined class 'ArgParser'` publicado pelo residente.
- **Causa** (`frozen::prefer_order`): duas bibliotecas "quentes" (API mudada recentemente) que se
  importam viravam grupos separados que esperavam um pelo outro. Nenhum ficava pronto, e o
  *layout* deixava as duas fora do programa (só um `debug_assert` notava). A análise seguinte
  rodava sem o próprio arquivo e publicava classes indefinidas em quem o importa.
- **Correção** (`45c5f371`): as bibliotecas quentes de um grupo ficam juntas num grupo só, e um
  grupo cuja espera nunca termina ainda entra no fim. O teste
  `resident_loads_every_import_of_the_next_batch` reproduz a sequência do `args`.

##### `hover`, `prepareRename` e a observação do disco (item 2)

- **A sessão confia no *watcher* do cliente** (`Session::trust_watcher`): com o cliente
  observando os arquivos, a entrada só é comparada com o disco depois de um aviso
  (`didChangeWatchedFiles`) ou a cada 30 s (a regra do residente e do índice). Sem essa
  capacidade, compara a cada consulta, como antes.
- **A atualização do índice** só olha os registros dos documentos abertos entre duas
  comparações do projeto inteiro, e só procura dependentes quando uma API mudou.

##### Os pedidos no `new_sali/frontend`

Pares revezados (Dart, DartForge, `base`, DartForge, Dart), carga 24–33; mediana em ms, com o
pior caso entre parênteses quando conta. O índice já está no *cache*.

| pedido | `base` (rodada 15) | DartForge agora (2 sessões) | Dart (2 sessões) |
|---|---:|---:|---:|
| `references`, sem edição | 3,7 | **1,0; 0,8** | 26,5; 26,7 |
| `references`, depois de edição | 207,8 | **26,9; 3,3** | 53,0; 20,9 |
| `implementation`, sem edição | 3,2 | **0,3; 0,3** | 14,0; 12,5 |
| `implementation`, depois de edição | 45,1 | **5,5; 2,5** | 11,7; 11,0 |
| `prepareRename`, sem edição | 3,2 | **0,4; 0,4** | 0,8; 1,1 |
| `prepareRename`, depois de edição | 26,1 | **1,5; 1,5** | 2,6; 4,0 |
| `hover`, sem edição | 10,4 | **0,4; 0,4** | 1,4; 1,3 |
| `hover`, depois de edição | 15,0 | **0,8; 0,8** | 2,2; 14,6 |
| *footprint* em repouso, todos os processos (MiB) | 1.085 | 1.218; 1.227 | **858; 889** |

- Ganhamos todos os pedidos, sem edição e depois de uma edição. Numa janela calma (carga 4–7,
  binário intermediário, antes das tabelas derivadas no lugar): `hover` 0,2 contra 0,6–1,2 ms;
  `references` depois de edição 19,8–43,1 contra 8,0–20,5 ms (então ainda perdia).
- O registro do residente leva ~20 ms com a máquina calma; a espera da busca por ele ficou em
  0–4 ms, porque ele sai junto com o fim da análise.
- A primeira edição de uma sessão ainda sai pelo caminho antigo (o índice só é lido na primeira
  busca, e só então o trabalhador passa a pedir os registros): 1,4–2 s, uma vez.

**`workspace/symbol`** (item 6, o mesmo identificador de cada posição como consulta; carga 27–34):

| | DartForge antes | DartForge agora | Dart |
|---|---:|---:|---:|
| sem edição | 215; 162 | **23; 22** | 39; 68 |
| depois de edição | 219; 201 | **28; 23** | 1.764; 1.869 |
| memória em repouso, todos os processos (MiB) | 620 | **636; 637** | 753; 744 |

- **Paridade:** a consulta `responsavelSelect` dava 1 resultado contra 2 do Dart. O segundo é o
  `_viewQuery_responsavelSelect_2_isDirty` do `.template.dart` gerado: o `PackageBuildWorkspace`
  do analisador resolve os `package:` que só existem em `.dart_tool/build/generated` para os
  gerados, e todo `FileState` criado é um arquivo conhecido do `FindDeclarations`. Agora os
  gerados que as bibliotecas importam entram, por último, na ordem em que os imports os
  alcançam.
- **O custo** era refazer a lista de arquivos a cada consulta e passar cada nome por uma
  `String` para minúsculas. A lista fica guardada enquanto o *watcher* não avisa (30 s no
  máximo), e os nomes ASCII viram minúsculas letra a letra.
- **O que ainda difere** (`symcmp.py`, comparação das listas): nas consultas sem limite os
  conjuntos são iguais e a ordem difere; nas que chegam ao limite de 500 (`String`, `processo`),
  os 500 escolhidos diferem. O Dart ordena os arquivos conhecidos pela ordem em que os cria
  durante a análise (imports, depois `discoverAvailableFiles`); nós, os do SDK e depois os dos
  pacotes. Não foi resolvido nesta rodada.

##### Memória (item 3)

**O que o servidor guarda** (heap vivo, `DARTFORGE_LSP_MEMORY=1`, sessão de pedidos):

| parte | MiB |
|---|---:|
| a biblioteca do documento na sessão | 376 |
| o índice de referências | 156 |
| o resto | ~25 |
| total vivo / *footprint* | 557 / 690 |

- **O índice por parte** (capacidades, sem o alocador): chaves 37, fatos de identificador 30,
  relações 21, classes 8,8, identificadores 7,7, linhas 7,1 MiB.
- **A duplicata não saiu.** A sessão do servidor tem a biblioteca do documento na visão do SDK do
  analisador; o residente, o pacote inteiro na visão do motor (com os *patches* do DDC). Uma
  resposta de `hover` montada no residente mostraria a declaração do *patch* (sem a
  documentação) nos membros *patcheados* do SDK. Servir as consultas do residente pede o mesmo
  SDK nos dois, o que muda o motor de `dartforge analyze`; ficou para o proprietário decidir.
- **Experimento descartado:** carregar sem corpos os outros arquivos do projeto na sessão leva
  a sessão de 376 a 313 MiB, mas `colors` e as *assists* olham corpos de outros arquivos.
- **Devolução de páginas no servidor:** 500 ms ocioso depois de trabalhar, o servidor pede a
  devolução das páginas livres, como o residente. Com o alocador desta versão do macOS
  (*xzone malloc*), `malloc_zone_pressure_relief` devolve 0 bytes nos dois processos: o efeito
  medido na rodada 15 não se repete aqui.
- **Resultado:** em repouso, 1.218–1.227 MiB contra 858–889 do Dart na sessão de pedidos.
  **Perdemos.** Sem pedidos (sessão longa, `workspace/symbol`) ganhamos.

##### A sessão longa (macOS)

150 rodadas (510 cargas), três pares revezados (o Dart primeiro nos pares a e c); a carga é a
média de 1 minuto no início e no fim de cada sessão, e a máquina nunca ficou calma (17–30). As
medianas são até assentar, em ms, com a CPU entre parênteses.

| carga | DartForge (a; b; c) | Dart (a; b; c) |
|---|---:|---:|
| onda de 40 `.g.dart` | **78 (57); 74 (51); 72 (51)** | 143 (233); 138 (240); 158 (228) |
| edição de corpo | **32 (25); 31 (24); 32 (25)** | 47 (34); 41 (33); 44 (33) |
| mudança de API | **51 (47); 45 (41); 50 (43)** | 141 (88); 121 (86); 140 (85) |
| pico do *footprint* (MiB) | **576; 576; 574** | 757; 698; 719 |
| em repouso após 60 s (MiB) | **527; 528; 574** | 756; 687; 686 |
| inclinação por 100 cargas (MiB) | **+2,3; −13,2; +2,4** | +41,7; +30,7; +35,7 |

- Ganhamos nas três cargas nos três pares, no relógio e na CPU. As publicações finais (492
  arquivos, 9.763 diagnósticos) são as mesmas da rodada 15.
- **A inclinação ≤ 0 não foi atingida em toda sessão** (uma em três). Numa sessão à parte com o
  *footprint* de cada processo (`footprint(1)` a cada 5 s) e o heap vivo
  (`DARTFORGE_LSP_MEMORY=1`):
  - o servidor fica em 21–30 MiB de *footprint* com **10,0 MiB vivos do começo ao fim**;
  - o residente fica em 554–559 MiB com **476,4 MiB vivos do começo ao fim**;
  - os degraus de 2–6 MiB são páginas que o alocador guarda, não dados.
- **Por que a devolução não ajuda aqui:** neste macOS (Darwin 27, *xzone malloc*),
  `malloc_zone_pressure_relief` devolve 0 bytes, na zona nula, na padrão ou em cada zona. Um
  programa em C que aloca 236 MiB em blocos pequenos e libera 7 de cada 8 continua com 236 MiB
  depois da chamada. As páginas presas são de fragmentação: cada uma ainda tem um bloco vivo.
- As curvas estão em `bench/results/lsp-long-session/*-r26-*.csv` (a de cada processo em
  `dartforge-lsp-r26-processes.csv`).

##### Windows: a edição de corpo (item 4)

Na mesma máquina da rodada 15 (i3-1215U), em `E:\Work\df-lsp`. Antes de cada sessão o script
espera não haver `cargo`, `rustc` ou `link`; o agente de corretude rodou testes na máquina de
03h50 até a reinicialização das 06h38, e as sessões só começaram depois dela.

**Primeiro, onde a edição de corpo perde.** Numa sessão só de edições de corpo, com só o
`frontend` como pasta (12 edições, carga 17–85%), ganhamos: **43; 56 ms contra 78; 78 ms** do
Dart. A perda da rodada 15 é da sessão longa (`new_sali` inteiro, com ondas e mudanças de API); no par
a com o binário do começo desta rodada ela era de 120 contra 61 ms.

**O tempo de cada fase** (sessão longa de 40 rodadas com `DARTFORGE_LSP_TRACE=1` e
`DARTFORGE_ENGINE_TIMES=1`; mediana das edições de corpo):

| fase | macOS | Windows antes | com a chave | com a chave e o contexto |
|---|---:|---:|---:|---:|
| troca no programa mantido | 9 | 42 | 25 | 13 |
| *outline* | 1 | 11 | 3 | 3 |
| corpos | 3 | 13 | 11 | 12 |
| verificadores de tipo | 2 | 9 | 8 | 9 |
| residente (início ao fim) | — | 88,5 | 62,0 | 52,0 |
| edição de corpo até assentar | 33 | 124 | 92 | 77 |

As causas achadas:
- **A chave de cada caminho** (`71959cd4`): `generated::key_of` só devolvia o próprio caminho,
  sem copiar, no Unix. No Windows, cada busca por chave reconstruía o caminho componente a
  componente, e o residente busca por chave cada unidade do programa várias vezes por análise.
  Agora um caminho de unidade (`C:\…`, só `\`, sem `.`, `..` ou partes vazias) é a própria
  chave, como no Unix.
- **A leitura do contexto** (`137ace98`): cada análise lia a data e o tamanho de cada arquivo do
  contexto do pacote, para saber se é um *part* (~1.000 `stat`, ~30 ms no Windows). Com o disco
  dado como inalterado (a regra dos 30 s da varredura), vale o que foi lido.

O que sobra são as fases de CPU (corpos e verificadores) ~4× mais lentas que no macOS; no Dart a
mesma edição é ~1,4× mais lenta que no macOS. Não foi achado nesta rodada por quê; o próximo passo
é uma amostragem com `xperf` (o *Windows Performance Toolkit* está na máquina).

**Sessão longa no Windows, 150 rodadas, dois pares revezados** (o binário final; carga 22–79%,
sem `cargo`, `rustc` ou `link`; mediana até assentar):

| carga | DartForge rodada 15 (a; b) | DartForge agora (a; b) | Dart (a; b) |
|---|---:|---:|---:|
| onda | 373; 358 | **246; 234** | 392; 433 |
| edição de corpo | 143; 139 | 77; 77 | **61; 60** |
| mudança de API | 270; 267 | **140; 140** | 267; 361 |
| bytes privados durante a sessão (MiB) | — | **368–381** | 524–734 |
| bytes privados no fim (MiB) | 370; 370 | **370; 374** | 709; 704 |

- **Ganhamos a onda e a mudança de API**; a edição de corpo caiu de 139–143 para 77 ms, mas
  ainda perde para os 60–61 ms do Dart.
- **Memória:** os bytes privados da árvore do servidor oscilam entre 368 e 381 MiB do começo ao
  fim, sem subir; os do Dart sobem de 524 para 734 MiB (~36 MiB por 100 cargas).
- Na sessão só de edições de corpo com o binário final (12 edições, carga 1–8%): **29,5 contra
  76,7 ms**.

##### Validação

- **Igualdade do índice** nos quatro projetos do oráculo (2.119 alvos): os registros do índice e
  os do residente dão as mesmas respostas da busca sem o índice (`references`,
  `implementation`, `prepareRename`).
- **Oráculo de paridade** com o binário final: a mesma tabela da rodada 15 (`references`,
  `implementation` e `definition` a 100%; *completion* top-5 a 100% e top-1 a 99%).
- **Testes em *release*** de `elements`, `types`, `analysis`, `world`, `emit_js`,
  `emit_js_production`, `parity`, `lsp` e `dev`, com o `node` no `PATH` e o
  `runtime/ddc/dart_sdk.js` copiado para o *worktree*: **853 passaram, nenhum falhou** (66
  ignorados). O teste novo `resident_loads_every_import_of_the_next_batch` está entre eles.
- **No Windows**, os testes de `generated.rs` passam, com o novo
  `windows_key_is_borrowed_when_already_normal`.
- **Sessão longa:** as publicações finais (492 arquivos, 9.763 diagnósticos) são as da rodada 15.
- **Clippy e formatação:** `cargo clippy --workspace --all-targets -- -D warnings` (com o LLVM
  22.1.8) e `cargo fmt --all -- --check` limpos. Dois arquivos de `emit_js` e um de `frontend`
  fora do formato vieram de `main` e foram formatados num *commit* à parte.

##### O que não foi feito nesta rodada

- **Memória em repouso abaixo do Dart com pedidos:** a sessão do servidor (376 MiB) e o índice
  (156 MiB) ficam ao lado do residente (~540 MiB). Ver acima.
- **A recarga no lugar casando os elementos por nome** (item 5): não começou. O caminho mais
  curto é o carregador alocar os elementos da biblioteca estável na ordem antiga, casada por
  nome, com as declarações novas no fim; a ordem dos diagnósticos que segue os ids precisa ser
  conferida antes.
- **`workspace/symbol`:** a ordem dos arquivos conhecidos (e, com isso, quais 500 resultados
  saem numa consulta ampla).
- **Windows:** a edição de corpo da sessão longa (77 contra 60–61 ms). As fases de CPU do
  residente ficam ~4× mais lentas que no macOS; falta uma amostragem com `xperf`.
- **A inclinação ≤ 0 em toda sessão longa no macOS:** o dado vivo é plano, e a fragmentação do
  alocador não volta com `malloc_zone_pressure_relief` neste macOS.
- **A primeira edição de uma sessão** antes da primeira busca ainda faz o registro pelo caminho
  antigo (1,4–2 s, uma vez).

#### Décima sétima rodada (2026-11-03): as consultas servidas pelo residente

##### As consultas no residente (item 1, decisão do integrador)

O servidor carregava a biblioteca do documento na sua própria sessão semântica (~376 MiB no
`new_sali/frontend`) para `hover`, `definition`, `prepareRename` e as outras consultas de
biblioteca, ao lado do programa do residente, que tem as mesmas bibliotecas. Agora
(`crates/lsp/src/remote.rs`):

- **O pedido.** O servidor manda a consulta (`remote::Call`) ao residente, com os documentos
  abertos (URI, versão, texto), pelo mesmo *pipe* das análises; a resposta (`remote::Answer`)
  volta numa linha própria, que o leitor do processo entrega a quem espera
  (`ResidentRecords::query`). Vão ao residente: `hover`, `definition`, `typeDefinition`,
  `signatureHelp`, `documentHighlight`, `prepareRename`, `closingLabels`, `inlayHint`,
  `semanticTokens`, `prepareCallHierarchy`, `prepareTypeHierarchy`, a navegação para o membro
  sobrescrito, `documentColor`/`colorPresentation`, `codeAction` e `completion`; e os registros
  do índice que o servidor precisaria refazer (`Call::Records`).
- **O residente.** Empresta a última análise (`Resident::lend_world`: programa, *interner*,
  tabela de tipos, *outline* e as tabelas de corpo) a um `SemanticAnalyzer` só dele, cuja sessão
  serve esse programa (`Session::lend`; nenhum outro escopo carrega nada), e roda o mesmo método
  que o servidor rodaria. O que a consulta calcula das unidades que não mudaram (comentários,
  ocorrências, nomes referenciados, o índice caminho → unidade) fica no `ResidentCache` entre
  empréstimos.
- **Quando o programa não é dos textos.** Cada documento aberto que o programa tem precisa ter o
  mesmo texto; senão a consulta espera a próxima análise (até 20 s no residente; o servidor espera
  até 15 s, `DARTFORGE_LSP_QUERY_WAIT_MS`) e, sem resposta, responde como antes, na própria
  sessão. Uma análise cancelada deixa o residente "não pronto" até a seguinte.
- **As consultas antes dos registros.** Depois de mandar os diagnósticos, o residente atende as
  consultas que chegaram antes de fazer os registros do índice (~20 ms): o `hover` logo depois de
  uma edição não espera por eles.
- **Os corpos.** Com as consultas ligadas, o residente guarda as tabelas de corpo das bibliotecas
  dos documentos abertos (`Resident::retain_bodies` solta as do resto de um lote grande) e infere
  na hora as de uma biblioteca que não as tenha.
- **O residente desde o começo.** A primeira análise de um pacote roda num processo à parte; logo
  depois o trabalhador carrega no residente os documentos abertos (sem publicar nada), para que a
  primeira consulta já o encontre. Uma consulta que não acha residente pede essa carga.
  `DARTFORGE_LSP_RESIDENT_QUERIES=0` volta ao caminho antigo.

**O SDK com os *patches* do DDC.** O residente usa o SDK do motor (`dartforge analyze`, com os
*patches* do DDC); o servidor, o do analisador. Um membro que um *patch* substitui tem, no
modelo, a declaração original em `public_declaration`. `Project::public_view` troca a função do
*patch* pela declaração original na definição e no `hover`; o mesmo vale para o item da hierarquia
de chamadas, os parâmetros da `signatureHelp` e a documentação de `completionItem/resolve`
(`Query::function_start`). Assim local, assinatura e documentação ficam iguais às do analisador.
O oráculo achou os dois casos que faltavam na primeira versão:
- `prepareCallHierarchy`, 100% → 98%: `toString`, `==` e `List.unmodifiable` apontavam para
  `core_patch.dart`;
- `signatureHelp`, 100% → 99%: `write(Object? obj)` no lugar de `write(Object? object)`.

Os dois foram corrigidos.

**O primeiro registro de uma sessão (item 5).** Os registros desatualizados do índice que não
vêm prontos do residente são pedidos a ele (`Call::Records`, sobre o programa emprestado) antes de
qualquer carga no servidor; sem resposta, vão a um processo filho, nunca à sessão do servidor.
Por isso a primeira edição antes da primeira busca não carrega mais a biblioteca no servidor
(1,4–2 s e ~376 MiB).

**O índice no servidor.** O *cache* do índice (~70 MB) era lido inteiro para o *heap*, e as
tabelas de cadeias cresciam dobrando: o *footprint* do servidor ficava ~150 MiB acima dos
158 MiB vivos. Agora o arquivo é mapeado (`memory::file_bytes`, Unix) e as tabelas nascem do
tamanho certo. O *footprint* em repouso do servidor caiu de 310 para 257 MiB.

**Os pedidos no `new_sali/frontend`** (mesmo *driver* e posições da rodada 16; Dart, agora,
rodada 16, agora, Dart; carga 11–38; mediana em ms):

| pedido | rodada 16 | agora (2 sessões) | Dart (2 sessões) |
|---|---:|---:|---:|
| `references`, sem edição | 0,4 | **0,8; 0,4** | 30,5; 27,3 |
| `references`, depois de edição | 95,0 | 36,7; 58,6 | **25,4; 19,2** |
| `implementation`, sem edição | 0,3 | **0,3; 0,3** | 12,3; 13,4 |
| `implementation`, depois de edição | 2,5 | **2,4; 2,3** | 32,0; 10,8 |
| `prepareRename`, sem edição | 0,4 | **0,4; 0,4** | 1,1; 1,1 |
| `prepareRename`, depois de edição | 1,5 | **1,9; 1,7** | 11,5; 2,1 |
| `hover`, sem edição | 0,4 | **0,5; 0,6** | 1,2; 1,3 |
| `hover`, depois de edição | 0,9 | **1,2; 1,2** | 24,2; 3,6 |
| *footprint* em repouso, todos os processos (MiB) | 1.220,5 | **793,7; 791,4** | 944,0; 814,0 |
| *footprint* do servidor sozinho (MiB) | 689,3 | 257,0; 259,2 | — |

- **Memória: agora ganhamos** (791–794 contra 814–944 MiB do Dart; eram 1.218–1.227). O
  servidor guarda só o índice (158 MiB vivos); o residente fica em ~545 MiB.
- **`hover` depois de edição** sobe de ~0,9 para ~1,2 ms (a ida e volta ao residente), ainda
  abaixo do Dart. Antes do índice de caminhos (`Project::unit_paths`) levava 4,4 ms: cada consulta
  procurava a unidade do documento comparando o caminho de ~3.600 unidades.
- **`references` depois de edição** (3 amostras) variou muito com a carga: 6,5 e 20,8 ms numa
  rodada anterior com o binário intermediário (Dart 18,4–20,2), 36,7 e 58,6 nesta (Dart
  19,2–25,4; a rodada 16, no mesmo momento, 95,0). **Não dá para dizer que ganhamos** essa
  mediana. O que pesa é a primeira busca larga depois do registro novo (o pior caso, `String`, fica
  em ~320 ms contra 5,4–7,1 s do Dart).

##### A sessão longa (macOS)

150 rodadas (510 cargas), dois pares com a rodada 16 entre eles (Dart, agora, rodada 16, agora,
Dart, rodada 16). Carga de 11 a 30. Mediana até assentar, em ms.

| carga | agora (a; b) | rodada 16 (a; b) | Dart (a; b) |
|---|---:|---:|---:|
| onda de 40 `.g.dart` | **71; 78** | 77; 63 | 109; 114 |
| edição de corpo | **30; 31** | 30; 23 | 34; 39 |
| mudança de API | **46; 49** | 45; 33 | 100; 112 |
| pico do *footprint* (MiB) | **579; 579** | 574; 574 | 717; 726 |
| em repouso após 60 s (MiB) | **579; 579** | 536; 574 | 714; 703 |
| inclinação por 100 cargas (MiB) | **+2,6; +1,1** | −1,0; +1,0 | +35,3; +36,9 |

- Ganhamos as três cargas e a memória nos dois pares.
- A rodada 16 no par b rodou com a máquina mais calma (carga 11) e fez a edição de corpo em 23 ms:
  as sessões não são comparáveis entre si com essa variação de carga. Nas publicações não há
  diferença de comportamento entre as duas versões nessa sessão, que não tem consultas.
- O residente agora guarda as tabelas de corpo dos documentos abertos entre as análises (para as
  consultas): o pico sobe ~5 MiB.
- As curvas estão em `bench/results/lsp-long-session/*-r27-long-*.csv`.

##### A inclinação ≤ 0 (item 2)

**Não foi atingida** (+2,6 e +1,1 MiB por 100 cargas). Não houve tempo para outra estratégia de
alocação nesta rodada. O que foi visto:
- o dado vivo segue plano (rodada 16: residente em 476,4 MiB do começo ao fim);
- `malloc_zone_pressure_relief` continua devolvendo 0 bytes neste macOS.

Fica para a próxima rodada: uma arena para as tabelas de corpo de cada análise, que hoje são
alocadas e soltas a cada carga.

##### Windows: onde vai a CPU da edição de corpo (item 3)

Antes de cada sessão o script esperou não haver `cargo`, `rustc` ou `link`.

**Amostragem com `xperf`** (`PROFILE`, 1 ms, com pilhas, no binário da rodada 16; 30 rodadas;
símbolos do PDB do *release*). O residente teve 11,8 s de amostras:

| onde | parte |
|---|---:|
| código do DartForge | 58% |
| `ntdll` (o *heap* do Windows: `RtlAllocateHeap`/`RtlFreeHeap`, `finish_grow` dos `Vec`) | 16% |
| núcleo (`ntoskrnl`, `Ntfs`, `FltMgr`) | 19% |
| `std::fs::metadata` (`stat`) | 10% |
| caminhos (`key_of`, `Components::next`, `without_verbatim`, `compare_components`) | ~10% |

- **Os `stat` do carregador.** Cada `import` relativo e cada `package:` resolvido perguntava ao
  disco se o arquivo existe, além da configuração de pacotes e dos arquivos de opções.
  `dartforge_elements::fs_cache` guarda essas respostas entre as cargas do residente enquanto o
  disco não mudou (o residente liga o *cache*; uma mudança avisada pelo *watcher*, ou a varredura
  a cada 30 s, o esvazia; `DARTFORGE_LSP_FS_CACHE=0` desliga).
  - Medido no macOS, uma edição de corpo pede ~40 dessas respostas, e uma onda ~375.
  - O ganho fica nas ondas e nas mudanças de API. **A edição de corpo não mudou: 77 ms.** Seus
    `stat` eram poucos.
- **O *segment heap* do Windows** (`FrontEndHeapDebugOptions=8` no IFEO do binário): 77 ms, sem
  mudança. A chave foi removida.
- **A linha do tempo de uma edição de corpo** (`DARTFORGE_LSP_TRACE=1`, 42 edições, mediana em
  ms desde a edição):

  | etapa | macOS | Windows |
  |---|---:|---:|
  | o trabalhador manda o lote ao residente | 3,3 | 8,3 |
  | o residente termina | 22,8 | 59,4 |
  | o servidor recebe os resultados | 24,4 | 60,2 |
  | o *driver* vê o fim (com a espera de 5 ms dele) | 31,7 | 76,9 |

  O residente leva ~50 ms no Windows contra ~19 no macOS. Entre os resultados e o fim visto
  pelo *driver*, o Windows gasta ~17 ms contra ~7: a espera de 5 ms do *driver* usa
  `recv_timeout`, que no Windows tem a resolução do relógio do sistema (~15,6 ms). Essa espera
  pesa igual para o Dart.

**Sessão longa no Windows, 150 rodadas, dois pares** (binário `c0c09e64`, o primeiro desta rodada,
com o *cache* de `stat`; carga 0–99%):

| carga | DartForge (a; b) | Dart (a; b) |
|---|---:|---:|
| onda | **223; 221** | 443; 400 |
| edição de corpo | 76; 76 | **60; 60** |
| mudança de API | **125; 124** | 426; 265 |
| bytes privados no fim (MiB) | **373; 370** | 710; 705 |

A edição de corpo **ainda perde**. A causa está nas fases de CPU do residente: o *heap* do Windows
e as operações de caminho. O `mimalloc` (rodada 15) dava ~12% e mais memória.

Os binários finais (`completion` no residente, índice mapeado) não foram medidos no Windows: o
coordenador reservou a máquina para a validação da versão.

##### Validação

- **Oráculo de paridade** nos quatro projetos (35 arquivos, 398 posições) com o binário final:
  **a mesma tabela das rodadas 15 e 16**. Ficam a 100% `references`, `implementation`,
  `definition`, `prepareRename`, `rename`, `documentHighlight`, `prepareCallHierarchy` e
  `signatureHelp`; *completion* top-5 a 100% e top-1 a 99%. Também foi rodado a cada passo:
  só as consultas de biblioteca, depois `codeAction`, depois `completion`.
- **Igualdade do índice** nos quatro projetos (2.119 alvos): o índice e os registros do residente
  dão as mesmas respostas da busca sem o índice (`references`, `implementation`,
  `prepareRename`).
- **Clippy** `--workspace --all-targets -- -D warnings` (com o LLVM 22.1.8) e **rustfmt**
  `--check` limpos.
- **Testes em *release*** de `elements`, `types`, `analysis`, `world`, `emit_js`,
  `emit_js_production`, `parity`, `lsp` e `dev`, com o `node` no `PATH` e o
  `runtime/ddc/dart_sdk.js` copiado: **não terminaram antes do fechamento desta seção**: com a máquina em carga 50–96, a compilação passou do limite de 90 min do script. Uma segunda volta, que reaproveita o que foi compilado, estava rodando no fechamento.

##### O que não foi feito nesta rodada

- **A inclinação ≤ 0 em toda sessão longa** (item 2): +2,6 e +1,1 MiB por 100 cargas; o dado vivo
  é plano.
- **A edição de corpo no Windows** (item 3): 76 contra 60 ms. Agora há uma amostragem que mostra
  onde vai o tempo; o próximo passo são as alocações e as chaves de caminho das fases do
  residente.
- **A recarga no lugar casando os elementos por nome** (item 4): não começou.
- **`workspace/symbol`** (item 6): a ordem dos arquivos conhecidos do Dart não foi reproduzida.
- **O que ainda carrega no servidor:**
  - as consultas de projeto: `rename`, `references` sem índice, `implementation` sem índice e as
    hierarquias completas;
  - o `implementation` de um identificador sem fato no índice;
  - a sessão larga.

  Numa sessão de editor comum elas são raras, mas cada uma ainda deixa o servidor no pico.
- **`references` depois de edição** não ganha do Dart de forma estável (ver a tabela).

Latência medida (2026-09-27, Linux, release, SDK 3.6.2, cache do SDK
quente; `cargo run --release -p dartforge-lsp --example
latencia_diagnosticos -- <arquivos>`: pelo protocolo, abre o arquivo,
espera a publicação tipada, aplica 10 edições incrementais esperando cada
uma e uma rajada de 20 sem esperar):

| Arquivo (corpus) | Tamanho | Imediata (mediana) | Tipada após edição (mediana; mín–máx) | Rajada de 20 |
|---|---|---|---|---|
| `linguagem/mixin/superclass_test.dart` | 6,5 KB, 218 linhas | 0,4 ms | 28 ms (28–35) | 1 publicação tipada, 48 ms |
| `pacotes/expect/lib/expect.dart` | 36 KB, 1.027 linhas | 2,2 ms | 34 ms (32–40) | 1 publicação tipada, 80 ms |
| `linguagem/void/void_type_usage_test.dart` | 74 KB, 2.256 linhas | 2,0 ms | 35 ms (34–47) | 1 publicação tipada, 106 ms |
| `linguagem/generic/super_bounded_types_error_test.dart` | 196 KB, 3.186 linhas | 11 ms | 56 ms (54–63) | 1 publicação tipada, 297 ms |

Remedido em 2026-10-01 no Windows (release, SDK 3.6.2, 159 códigos
publicados; mesmo exemplo), com a publicação idêntica à do `dartforge
analyze` em todos (24, 0, 81 e 604 semânticos):

| Arquivo (corpus) | Imediata (mediana) | Tipada após edição (mediana; mín–máx) | Rajada de 20 |
|---|---|---|---|
| `linguagem/mixin/superclass_test.dart` | 0,5 ms | 55 ms (49–63) | 1 publicação tipada, 91 ms |
| `pacotes/expect/lib/expect.dart` | 2,0 ms | 57 ms (50–63) | 1 publicação tipada, 114 ms |
| `linguagem/void/void_type_usage_test.dart` | 3,3 ms | 51 ms (48–58) | 1 publicação tipada, 198 ms |
| `linguagem/generic/super_bounded_types_error_test.dart` | 14 ms | 68 ms (62–82) | 1 publicação tipada, 329 ms |

Vivo acima da base depois de fechar os documentos: 0,04 MiB.

A primeira análise da sessão com o cache do SDK frio (construído na hora)
levou 227 ms no arquivo de 6,5 KB. Depois de fechar os documentos, o vivo
acima da base ficou em 0,03 MiB (alocador contador). Na rajada, o tempo é
dominado pelas 20 publicações imediatas (o parser do arquivo inteiro a cada
tecla), não pela tipada. Os arquivos do corpus não importam pacotes
grandes; num projeto com muitas dependências a carga cresce com elas.

Limitações: o custo de cada análise cresce com o número de documentos
abertos do pacote (todos entram no programa, e cada biblioteca aberta tem
os corpos inferidos); dependentes **fechados** não são analisados (não há
publicação para eles); um arquivo importado de outro pacote (dependência
por caminho) editado sem salvar vale para a carga, mas não marca o pacote
dos que o importam; a publicação imediata de uma nova versão substitui a
tipada anterior até a nova tipada chegar (os códigos que dependem de tipos
somem por algumas dezenas de ms a cada edição); as ações de código usam só
os diagnósticos imediatos.

## 04/11/2026: décima oitava rodada — `workspace/symbol` comparado como resposta completa

O oráculo de `crates/lsp/oracle` chamava seis consultas por projeto e comparava só o
conjunto de `(nome, URI)`. A tabela anterior dizia 24/24 iguais; reler os mesmos JSONL
com a ordem e todos os campos mostra apenas 4/24. Isso não era evidência de igualdade
da resposta inteira.

O driver agora acrescenta `a` e `e`, que alcançam os 500 resultados, e o resumo separa
conjunto, lista ordenada de nomes/URIs e lista completa de itens. O último critério não
normaliza URIs, não elimina duplicatas e não descarta campos. Resultados nos quatro
projetos anteriores (`args-2.7.0`, `path-1.9.1`, `string_scanner-1.4.1` e `app`), com o
mesmo Dart 3.6.2 no macOS:

| Critério | Antes | Depois |
|---|---:|---:|
| conjunto de nomes e URIs | 24/32 | 32/32 |
| nomes e URIs em ordem | 4/32 | 32/32 |
| resposta completa em ordem | 4/32 | 32/32 |

O conserto segue os pontos de descoberta do analisador 3.6.2, conferidos no commit
`b0cc5495e0f5e8ae150825a5352e708cb49e65ff` do SDK:

- os arquivos adicionados seguem a enumeração do sistema de arquivos em profundidade,
  como `ContextRoot.analyzedFiles`, sem a ordenação alfabética de `project_files`;
- `AnalysisDriver._discoverDartCore` cria `dart:core` e suas referências diretas antes
  da descoberta geral; as listas de exports são materializadas antes das de imports;
- bibliotecas alcançadas pelas dependências entram antes dos arquivos restantes do SDK
  e dos pacotes. A resolução usa o mapa de nomes do `package_config.json`, incluindo
  a alternativa gerada em `.dart_tool/build/generated`;
- o elemento do operador `-()` chama-se `unary-`. O índice sintático usava `-`, e a
  consulta `a` perdia esses métodos, deslocando também o corte dos 500 resultados.

A ordenação das demais cargas semânticas permanece determinística: só a lista de
arquivos para símbolos ganhou a enumeração compatível. O cache conserva a ordem
descoberta, e há regressões para a descoberta inicial do SDK, dependências de pacotes
antes do SDK ainda não descoberto e os dois operadores de subtração.

Os JSONL desta medição estão em `/tmp/dartforge-workspace-symbols-before.jsonl` e
`/tmp/dartforge-workspace-symbols-final.jsonl`. A comparação serve para corretude;
as latências do driver rodaram junto dos testes do workspace e não são uma nova
medição de desempenho. A suíte completa do LSP, com doctests, passou.

A criação antes da primeira consulta e a primeira comparação em projetos grandes foram
acrescentadas abaixo. Ainda falta fechar a paridade nesses projetos, repetir depois
de edições e com dependências condicionais, e medir CPU, latência e memória sem
compilação ou testes concorrentes. Os 32/32 não fecham essas verificações nem as demais
metas de desempenho da rodada 17.

### Continuação: ciclo de vida de 38/56 para 56/56 depois de duas correções

O novo `crates/lsp/oracle/workspace_symbol_lifecycle.py` cria um pacote temporário com
mais de 620 símbolos e compara sete consultas em oito fases: inicial, edição sem salvar,
fechamento com outro arquivo aberto, criação, mudança em disco, remoção, renomeação e
fechamento do último arquivo. A comparação mantém todos os campos e a ordem, inclusive
o corte de 500 resultados. Antes de consultar, o driver espera o contexto de análise e
um novo `$/analyzerStatus` concluído com intervalo de repouso; não compara a resposta
antiga do Dart enquanto os eventos do disco ainda estão pendentes.

Duas execuções iniciais com Dart 3.6.2 no macOS deram **38/56 respostas completas iguais**
e **46/56 conjuntos de nomes/URIs iguais**. As primeiras três fases deram **21/21**.
Depois das correções, os dois critérios dão **56/56**. Os quatro projetos anteriores
também foram repetidos: **32/32 respostas completas iguais**.

| Critério do ciclo de vida | Antes | Depois |
|---|---:|---:|
| conjunto de nomes e URIs | 46/56 | 56/56 |
| resposta completa em ordem | 38/56 | 56/56 |

Os ajustes seguem as mesmas fontes do analisador 3.6.2:

- **A ordem deve sobreviver aos eventos do disco.** `OwnedFiles.addAdded` do analisador
  mantém os arquivos existentes em um mapa com ordem de inserção e acrescenta os novos
  no fim (`analyzer/lib/src/dart/analysis/driver.dart`). A enumeração nova do DartForge
  colocava um arquivo recém-criado antes dos antigos: `LifecycleCreated` deslocava
  `LifecycleA188` no corte de 500. Agora a listagem retém os sobreviventes na ordem
  anterior e acrescenta os novos. Os eventos de criação/remoção mantêm essa sequência,
  inclusive se um caminho for removido e recriado sem consulta intermediária. A
  atualização mantém as posições em um mapa; a remoção de uma pasta também retira
  seus descendentes da listagem.
- **Fechar o último arquivo deve respeitar a opção de inicialização.** Com
  `onlyAnalyzeProjectsWithOpenFiles=true`, o Dart remove o contexto e todas as sete
  consultas ficam vazias. O DartForge conservava as raízes do `initialize` e retornava
  símbolos do projeto e do SDK. Agora a opção impede raízes permanentes, inclusive nas
  notificações de pastas, e o índice de símbolos é liberado ao fechar o último documento.
  Os pedidos com escopo de raiz usam os projetos dos documentos ainda abertos; uma
  regressão verifica que as quatro apresentações de cor continuam iguais ao modo padrão.

Os JSONL anteriores estão em `/tmp/dartforge-symbol-lifecycle-settled.jsonl` e
`/tmp/dartforge-symbol-lifecycle-settled-repeat.jsonl`; os novos, em
`/tmp/dartforge-symbol-lifecycle-final.jsonl` e
`/tmp/dartforge-workspace-symbols-lifecycle-fixed-initial.jsonl` (os quatro projetos).
A suíte completa dessa etapa passou com **312 testes, zero falhas e zero ignorados**, incluindo
doctests. Formatação, clippy de todo o workspace e documentação com avisos negados
passaram depois dos ajustes. Esses números ampliam a evidência de corretude; não são
medições de desempenho e não encerram as pendências dos projetos grandes e de dependências condicionais.

Para repetir, configure `ORACLE_DART` e `ORACLE_SDK_LIB` para o mesmo SDK 3.6.2 e rode:

```sh
python3 crates/lsp/oracle/workspace_symbol_lifecycle.py /tmp/symbol-lifecycle.jsonl target/debug/dartforge-lsp
python3 crates/lsp/oracle/summary.py /tmp/symbol-lifecycle.jsonl --method workspace/symbol
```

O oráculo termina com código 1 enquanto houver diferenças; não transforma essas
divergências em aprovações. O resumo identifica a fase de cada exemplo e não inventa
latências zero quando o arquivo de corretude não contém tempos.

### Continuação: a primeira busca e os projetos grandes

O modo `--create-before-first-query` cria um arquivo depois da análise inicial e antes
de qualquer `workspace/symbol`. A matriz normal passava em 56/56, mas essa variante
dava só **38/56**: o índice ainda não existia para guardar a ordem anterior aos eventos.
Agora a ordem dos arquivos é capturada ao abrir o contexto, separada da criação do
índice de declarações. Os eventos atualizam essa ordem global desde então; os contextos
fechados saem do registro. As duas matrizes passaram com **112/112 respostas completas**,
e os quatro projetos pequenos mantiveram **32/32** na árvore atual.

Os resultados atuais estão em `/tmp/dartforge-symbol-context-final-warm.jsonl`,
`/tmp/dartforge-symbol-context-final-cold.jsonl` e
`/tmp/dartforge-symbol-context-final-initial.jsonl`. Para repetir a variante:

```sh
python3 crates/lsp/oracle/workspace_symbol_lifecycle.py /tmp/symbol-before-first.jsonl target/debug/dartforge-lsp --create-before-first-query
```

A captura inicial faz a descoberta das fronteiras dos contextos e enumera seus arquivos;
os contextos existentes não são varridos novamente por edições de corpo. O custo adicional
na abertura e na revalidação da listagem ainda precisa ser medido nos projetos grandes.
Esses resultados são de corretude, sem conclusão nova de velocidade ou memória.

O oráculo geral passou a esperar a conclusão real de `$/analyzerStatus`: uma análise
sem erros pode não publicar diagnósticos vazios para todos os arquivos. O limite opcional
`ORACLE_OPEN_FILES=1` abre só o primeiro documento, sem limitar a busca de símbolos a ele;
sem essa variável, continuam sendo abertos todos os arquivos anteriores. Isso permite
examinar os projetos grandes sem criar milhares de buffers no editor simulado.

A primeira execução nos três projetos abriu `lib/core.dart` no `limitless_ui`,
`lib/argon2.dart` no `new_sali/core` e `lib/new_sali_frontend.dart` no frontend. Cada um
recebeu as mesmas oito consultas anteriores, com Dart 3.6.2 e as configurações de packages
encontradas no disco. A auditoria posterior, abaixo, identificou caminhos Windows
inexistentes no core; essa primeira medição não valida suas dependências completas.
Resultado **histórico e ainda incompleto**, depois do ajuste dos contextos aninhados:

| Projeto | Resposta completa igual | Conjunto de nomes/URIs igual |
|---|---:|---:|
| `limitless_ui` | 3/8 | 4/8 |
| `new_sali/core` | 5/8 | 8/8 |
| `new_sali/frontend` | 2/8 | 3/8 |
| Total | **10/24** | **15/24** |

Antes do ajuste eram 9/24 respostas e 13/24 conjuntos. O `ContextLocator` do SDK não
separa uma pasta só por ter `pubspec.yaml`: `benchmarks`, sem configuração própria de
packages, participa do contexto pai. Já `example/.dart_tool/package_config.json` cria
um contexto filho, cujos arquivos vêm depois dos do pai. A descoberta agora segue essas
fronteiras (também `BUILD.gn`), mantendo o limite total de arquivos e a ordem dos contextos.
Uma regressão compara as duas situações. Em `limitless_ui`, a consulta `Arg` passou a
coincidir integralmente, incluindo `AppComponent.rowTarget` do benchmark.

Havia diferenças da ordem das bibliotecas conhecidas: em `new_sali/core`, os 8/8
conjuntos coincidem, mas três consultas divergem em ordem; em `Arg`, o Dart chega a
`developer/timeline.dart` antes de `_http/http.dart`, e o DartForge faz o contrário.
No frontend, a primeira diferença de `Arg` está no item 169: o Dart retorna
`EssentialCoreUtils._accentRegex`, enquanto o DartForge já está em `html_dart2js.dart`.
Também sobram diferenças de conjunto nos outros dois projetos. A configuração e a
descoberta de dependências por contexto precisam ser conferidas; os 112/112 sintéticos
não demonstram paridade nesses casos reais.

Os JSONL estão em `/tmp/dartforge-symbol-context-large.jsonl` (antes) e
`/tmp/dartforge-symbol-context-large-grouped.jsonl` (depois). Para repetir, com as variáveis
do SDK já configuradas:

```sh
ORACLE_METHODS=symbol ORACLE_OPEN_FILES=1 python3 crates/lsp/oracle/oracle.py /tmp/symbol-large.jsonl target/debug/dartforge-lsp /Volumes/ssd_dock/MyProjectsDart/limitless_ui /Volumes/ssd_dock/MyProjectsDart/new_sali/core /Volumes/ssd_dock/MyProjectsDart/new_sali/frontend
python3 crates/lsp/oracle/summary.py /tmp/symbol-large.jsonl --method workspace/symbol
```

Na repetição dos testes, `semantic.rs` expôs uma suposição de sincronização: um `pump`
sem nova análise ainda pode drenar publicações da versão válida anterior. O teste agora
espera esse trabalho terminar antes de enviar a versão obsoleta, mantém a exigência de
fila vazia e verifica que o número de análises não aumentou. A suíte atual, incluindo
essa correção, está em `/tmp/dartforge-symbol-context-verified-tests.log`.

### Continuação: descoberta de dependências e auditoria dos caches do Dart

A investigação da revisão 3.6.2 separou dois caminhos de descoberta. O bootstrap de
`dart:core` usa `discoverReferencedFiles`, que materializa exports antes de imports.
Já `LibraryNode.computeDependencies` materializa imports, acrescenta o core implícito
e só depois materializa exports. O índice usava a primeira ordem também nos ciclos
normais. Agora segue a segunda nesses ciclos e inicia o percurso pelas prioridades
abertas antes dos demais arquivos adicionados.

A enumeração de bibliotecas disponíveis conserva a ordem dos nomes em `libraries.dart`,
mas resolve cada nome pelo `SdkLayout::load_as_analyzer`. O caminho posicional não
necessariamente é o selecionado: `dart2jsPath` substitui, por exemplo, a variante de
`html_common`. Indexar também a variante padrão duplicava declarações de parts
compartilhadas e deslocava o limite de 500 resultados. A regressão usa duas variantes
com a mesma part e exige uma única declaração, tanto no cache frio como no quente.

Outras correções verificadas:

- o grafo guarda as diretivas dos documentos prioritários; mudar um import/export sem
  salvar descobre os novos alvos, enquanto uma edição de corpo atualiza os intervalos
  sem mudar a descoberta. Novas prioridades não reordenam bibliotecas já conhecidas;
- a substituição de um catálogo elimina os índices e listagens que deixaram de participar.
  Uma regressão verifica a resposta nova e, por referência fraca, a liberação da biblioteca
  antiga;
- `rootUri` e `packageUri` relativos são resolvidos como URIs, incluindo segmentos `..`
  e escapes percentuais. Um teste pelo protocolo importa um pacote numa pasta com espaço
  e exige a URI normalizada real, sem caminhos contendo `../`;
- fechar documentos remove sua prioridade e reduz a capacidade do vetor quando ele fica
  esparso. A primeira suíte detectou retenção no teste `plateau_protocol`; corrigir a
  estrutura fez o teste passar com o mesmo limite. Esse teste usou documentos sintéticos,
  pois o corpus opcional não estava configurado; não mede o platô dos projetos grandes.

O rastreador auxiliar `crates/lsp/oracle/trace_discovery.dart` registra contextos,
`OwnedFiles.addedFiles` e arquivos conhecidos antes/depois de `discoverAvailableFiles`.
Ele foi executado com fontes do analyzer exportados da tag 3.6.2, com a licença
preservada (origem em `docs/REFERENCIAS.md`). As dependências auxiliares resolvidas offline
não reproduzem o lock completo do SDK; o servidor do SDK binário continua sendo a
referência de paridade. O rastreador não substitui as respostas LSP.

#### Por que não registrar um único percentual dos projetos grandes

Duas execuções do mesmo binário do DartForge deram respostas idênticas, mas o próprio
Dart variou em cinco consultas do `limitless_ui`. Para separar essa variação de mudanças
nos fontes, `fingerprint_inputs.py` passou a registrar tamanho, mtime e SHA-256 de Dart,
YAML, configurações de packages, metadados do SDK, executável e snapshot do servidor,
incluindo arquivos Dart gerados. Raízes ausentes aparecem explicitamente no manifesto
e agora também causam saída de erro, para impedir uma nova auditoria incompleta.
Erros de leitura ou mudança durante o hash também fazem o script falhar.

A auditoria também encontrou **64 raízes Windows inexistentes** no
`new_sali/core/.dart_tool/package_config.json`. Os resultados anteriores desse caminho
não demonstravam paridade com suas dependências completas. Uma cópia de teste em
`/tmp/dartforge-core-resolved-ue807bom/core` corrigiu somente esses `rootUri` para as mesmas
64 versões já presentes no cache local. Os 554 demais arquivos foram comparados por
SHA-256 e mantidos idênticos. O projeto original não foi alterado, nem houve nova resolução
de versões. O manifesto de substituições está em `/tmp/dartforge-core-resolved-relocations.json`.

Cada projeto foi então executado com um diretório de cache do Dart inicialmente vazio,
seguido de uma execução que reutilizou esse diretório (`ORACLE_DART_CACHE`, opção
`--cache` do servidor). Os manifestos antes e depois foram idênticos em cada projeto:

| Projeto | Arquivos monitorados | Digest do manifesto (prefixo) | Raízes ausentes |
|---|---:|---|---:|
| `limitless_ui` original | 9.761 | `a78e1d65cfd2` | 0 |
| core, cópia com caminhos corrigidos | 6.640 | `aab688b0487d` | 0 |
| `new_sali/frontend` original | 10.894 | `0f834beeaf55` | 0 |

O mesmo documento inicial e as mesmas oito consultas foram usados nos dois lados.
Os números abaixo comparam a resposta inteira em ordem; a coluna de conjunto é separada:

| Projeto | Completa, cache Dart frio | Completa, quente | Conjunto, frio | Conjunto, quente |
|---|---:|---:|---:|---:|
| `limitless_ui` | 2/8 | 4/8 | 2/8 | 4/8 |
| core, cópia corrigida | 2/8 | 8/8 | 6/8 | 8/8 |
| `new_sali/frontend` | 4/8 | 5/8 | 7/8 | 7/8 |
| Total desta matriz | **8/24** | **17/24** | **15/24** | **19/24** |

As respostas do DartForge foram idênticas nas duas condições de cada projeto. As do Dart
mudaram em cinco consultas do `limitless_ui`, seis do core e quatro do frontend. No
`limitless_ui`, uma terceira execução quente repetiu integralmente a segunda. Em `Arg`,
a primeira diferença entre os dois Dart está no item 480: `ffi.VarArgs` no frio e
`ngx_test.TestAlreadyRunningError` no quente; ambos retornam 500 itens. A ordem também
altera o conjunto quando a busca chega ao limite. Isso documenta a variação observada
sob essas condições; ainda falta isolar sua causa interna. Não elimina divergências
nem transforma igualdade de conjuntos em igualdade da resposta.

Há diferenças estáveis a corrigir. `Scanner` no `limitless_ui` inclui no Dart
`cli_pkg/src/last_changelog_section.dart`, ausente no DartForge; o contexto filho do
`example` tem catálogo próprio. Os namespaces e a propriedade de URIs por contexto
ainda estão achatados no índice. Também faltam a criação dos estados de todas as
alternativas condicionais e a descoberta disparada ao resolver bibliotecas durante
uma consulta, visível nas consultas seguintes. Não basta percorrer todas as alternativas
como se fossem imports selecionados: isso também mudaria a ordem.

Os resultados estão em `/tmp/dartforge-symbol-limitless-audit-{cold,warm,repeat}.jsonl`,
`/tmp/dartforge-symbol-core-resolved-{cold,warm}.jsonl` e
`/tmp/dartforge-symbol-frontend-isolated-{cold,warm}.jsonl`. Os manifestos correspondentes
são `/tmp/dartforge-limitless-inputs-{before,cold,warm,repeat}.json`,
`/tmp/dartforge-core-resolved-inputs-{before,cold,warm}.json` e
`/tmp/dartforge-frontend-isolated-inputs-{before,cold,warm}.json`. As linhas `initialize`
do oráculo agora registram raiz, arquivos abertos, SDK e cache de referência para novas
execuções. Para repetir uma dupla, com o SDK e o projeto já configurados:

```sh
export ORACLE_METHODS=symbol ORACLE_OPEN_FILES=1
export ORACLE_DART_CACHE="$(mktemp -d)"
python3 crates/lsp/oracle/fingerprint_inputs.py "$PROJECT" "$DARTFORGE_DART_SDK" /tmp/inputs-before.json
python3 crates/lsp/oracle/oracle.py /tmp/symbol-cold.jsonl target/debug/dartforge-lsp "$PROJECT"
python3 crates/lsp/oracle/fingerprint_inputs.py "$PROJECT" "$DARTFORGE_DART_SDK" /tmp/inputs-cold.json
python3 crates/lsp/oracle/oracle.py /tmp/symbol-warm.jsonl target/debug/dartforge-lsp "$PROJECT"
python3 crates/lsp/oracle/fingerprint_inputs.py "$PROJECT" "$DARTFORGE_DART_SDK" /tmp/inputs-warm.json
python3 crates/lsp/oracle/summary.py /tmp/symbol-cold.jsonl --method workspace/symbol
python3 crates/lsp/oracle/summary.py /tmp/symbol-warm.jsonl --method workspace/symbol
```

Os quatro projetos pequenos mantiveram **32/32**, e as duas matrizes de ciclo de vida,
**112/112 respostas completas**. A suíte dessa etapa terminou com **316 aprovados,
zero falhas e zero ignorados**, incluindo doctests, em
`/tmp/dartforge-symbol-discovery-verified-tests.log`. Clippy de todo o workspace e
documentação com avisos negados passaram novamente (logs de mesmo prefixo,
`verified-clippy.log` e `verified-doc.log`); a formatação passou. Esta rodada valida
corretude e retenção nos testes citados; não repete CPU, latência ou memória nos projetos
grandes, nem conclui a meta de desempenho da rodada 17.

### Continuação: catálogos por contexto e gerados fora de `lib/`

A enumeração anterior reconhecia os arquivos do `example`, mas a busca carregava apenas
o catálogo da raiz aberta. Agora a listagem entrega os contextos e seus arquivos juntos;
cada contexto conserva seu catálogo e seu diretório de gerados. O percurso de uma
dependência compartilhada mantém o contexto de quem a importou, mesmo fora da raiz.
Uma configuração herdada é procurada nos ancestrais quando o contexto não tem a sua.

Três regras do SDK 3.6.2 foram conferidas na revisão já registrada:

- `OwnedFiles` identifica arquivos pela URI canônica. Para uma URI adicionada, fica o
  primeiro dono; para uma conhecida, fica o último, sem mudar a posição de inserção.
  Duas versões físicas de `package:versioned/api.dart` não viram duas bibliotecas na
  resposta. Um arquivo já criado no mesmo driver não recupera a propriedade ao ser
  visitado novamente;
- o catálogo do analyzer vem da árvore de `PackageConfig`, ordenada pelas URIs das
  raízes, e não pela ordem do array JSON. Essa ordenação fica restrita ao catálogo
  dos símbolos; as demais cargas mantêm sua ordem anterior;
- `ContextManager.driverMap` é um `HashMap<Folder, AnalysisDriver>`. A enumeração dos
  arquivos disponíveis percorre seus buckets, não a ordem de criação dos contextos.
  O hash de `Folder` é o `String.hashCode` do caminho. O modelo em
  `workspace_symbols/context_order.rs` usa os code units UTF-16 da VM, as colisões e
  os crescimentos de tabela. Dois testes com 40 caminhos, incluindo pares substitutos,
  comparam hashes e ordem capturados da VM 3.6.2. A adaptação e sua licença constam de
  `THIRD_PARTY_NOTICES.md`.

O rastreador do SDK também passou a usar esse `HashMap` na descoberta e a registrar os
hashes dos contextos. No `limitless_ui`, a ordem de criação é pai/filho, mas a do mapa é
`example`/pai: hashes **391129904** e **780281349**, respectivamente. Assim,
`cli_pkg/src/last_changelog_section.dart` pertence ao filho, e
`sass/src/parse/parser.dart` termina associado à versão **1.66.0** do pai, não à 1.101.2
do filho. O rastreamento está em `/tmp/dartforge-symbol-catalog-limitless-source-trace.json`.
É evidência auxiliar de descoberta; o oráculo binário continua sendo o critério de paridade.

A conferência das URIs expôs duas falhas dos gerados. Um import relativo de uma biblioteca
gerada deve partir de sua URI canônica, não do diretório físico do output. Caso contrário,
`adapter.dart` importando `api.dart` podia criar um arquivo inexistente no output e ocultar
o `api.dart` real do pacote na mesma URI. Além disso, `PackageConfigWorkspace` resolve
gerados de `test/` e `web/`, não apenas de `lib/`. O catálogo agora conserva a raiz real
do pacote separada de seu `packageUri`; os gerados fora de `lib/` usam a identidade da
URI de arquivo original. Isso recuperou os templates de teste que faltavam no frontend.

#### Oráculo de contextos e repetição dos projetos

`crates/lsp/oracle/workspace_symbol_contexts.py` cria pai e filho com versões distintas
de um pacote, uma dependência comum que importa esse pacote, dependência privativa do
filho e gerados dentro/fora de `lib/`. As quatro fases são inicial, repetição, troca de
versão no catálogo e uma troca que tenta fazer uma dependência substituir uma biblioteca
explicitamente adicionada. São seis consultas por fase, comparando todo o JSON em ordem.
O driver espera um intervalo de repouso de análise; nas trocas, também espera que a
definição do import corresponda ao catálogo novo, evitando usar um status atrasado.
Esse pedido de preparação é enviado aos dois servidores e faz parte da sequência do cenário.

O cenário com watcher do cliente passou em **24/24**, em três execuções com diretórios temporários diferentes:
**72/72 respostas completas iguais**. Os arquivos estão em
`/tmp/dartforge-symbol-contexts-final-identical-{1,2,3}.jsonl`. Para repetir:

```sh
python3 crates/lsp/oracle/workspace_symbol_contexts.py /tmp/symbol-contexts.jsonl target/debug/dartforge-lsp
python3 crates/lsp/oracle/summary.py /tmp/symbol-contexts.jsonl --method workspace/symbol
```

A nova dupla dos três projetos usou a mesma cópia corrigida do core, um documento aberto
por projeto e um cache Dart novo por projeto, reutilizado na segunda execução. Os três
manifestos de entrada conservaram os digests e as contagens da auditoria anterior, sem
raízes ausentes. Resultado atual dessa dupla, sem filtrar divergências:

| Projeto | Completa, cache Dart frio | Completa, primeira quente | Conjunto, frio | Conjunto, quente |
|---|---:|---:|---:|---:|
| `limitless_ui` | 3/8 | 7/8 | 5/8 | 7/8 |
| core, cópia corrigida | 4/8 | 8/8 | 7/8 | 8/8 |
| `new_sali/frontend` | 4/8 | 5/8 | 8/8 | 8/8 |
| Total desta dupla | **11/24** | **20/24** | **20/24** | **23/24** |

A dupla anterior, antes dessas correções, dava 8/24 e 17/24 respostas completas, com
15/24 e 19/24 conjuntos. A repetição adicional do frontend, no mesmo cache quente,
deu **8/8 respostas completas**. A resposta do DartForge permaneceu idêntica; o Dart
mudou a ordem de `ctx`, `Ponto` e `join`. Não se deve substituir a coluna da primeira
execução quente pelo melhor resultado. A ordem do mapa de drivers explica uma diferença
entre as implementações, mas não isola a causa de toda a variação entre execuções do Dart.

Os resultados dessa dupla estão em
`/tmp/dartforge-symbol-catalog-final-{limitless,core,frontend}-{cold,warm}.jsonl`; a
repetição adicional, em `/tmp/dartforge-symbol-catalog-final-frontend-repeat.jsonl`.
Os manifestos são `/tmp/dartforge-catalog-final-{limitless,core,frontend}-inputs-{before,cold,warm}.json`
e `/tmp/dartforge-catalog-final-frontend-inputs-repeat.json`. Todos mantiveram os mesmos
digests; a repetição fria/quente também manteve idênticas as respostas do DartForge.

**Achado dessa etapa, corrigido na continuação abaixo:** `Ponto` no `limitless_ui` não incluía o getter
`LiNotificationOutletComponentNgFactory`, a variável `_LiNotificationOutletComponentNgFactory`
e a função `createLiNotificationOutletComponentFactory()` do template de notification.
O rastreador do SDK mostrava esse arquivo conhecido antes da varredura de disponibilidade;
a continuação abaixo identifica a causa e a correção. Continuam faltando
as alternativas condicionais, a descoberta iniciada durante consultas e a paridade
após edições nos projetos grandes. O modelo do mapa de drivers reconstrói os buckets
para os contextos ativos; ainda falta preservar a capacidade histórica quando muitos
contextos são removidos e reabertos depois de uma expansão. Os testes de crescimento
conferem uma tabela criada do zero, não esse histórico.

A suíte dessa etapa passou com **319 testes, zero falhas e zero ignorados**, incluindo
doctests e o platô por protocolo. A regressão de navegação confere as versões escolhidas,
a dependência privativa, gerados em teste e um import relativo do output para a fonte real.
Mantidos **112/112** no ciclo de vida e **32/32** nos quatro projetos pequenos. Logs:
`/tmp/dartforge-symbol-catalog-verified-{tests,clippy,doc}.log`; clippy de todo o workspace,
formatação e documentação com avisos negados passaram. Os JSONL repetidos desses cenários
anteriores têm o prefixo `/tmp/dartforge-symbol-catalog-final-`. As execuções servem à
corretude; não são novas medições de CPU, latência ou memória nos projetos grandes.

### Continuação: o estado de um import inexistente não pode retomar uma URI válida

O diagnóstico das três ausências de `Ponto` isolou uma falha de revalidação, não de
extração: uma primeira busca pelo índice retornou **373 símbolos**, incluindo as três
declarações de `notification_toast.template.dart`, com prefilter e matcher aprovando
os nomes. Depois das consultas anteriores do oráculo, a resposta caía para **370**.
O diagnóstico temporário foi removido; seus dados estão em
`/tmp/dartforge-symbol-coverage-probe.json`.

`Discovery::retain_live` descartava o registro de criação de um arquivo inexistente
no pai, embora sua URI canônica pertencesse a um gerado válido do filho. Ao percorrer
de novo as dependências, esse arquivo parecia recém-criado e retomava a URI. O filho
não a recuperava: seu arquivo já tinha sido criado naquele contexto. A posição era
preservada, mas apontava para um arquivo sem declarações. O efeito aparecia ao revalidar
o disco, inclusive em clientes que não anunciam watcher.

A limpeza agora mantém o registro de criação inexistente enquanto ele compartilhar
uma URI com uma biblioteca válida. Os demais imports inexistentes continuam sendo
descartados; conservar todos eles acumularia o histórico das diretivas inválidas
digitadas no editor. A nova regressão falhava na segunda busca antes da correção.
Agora repete a resposta completa e sua URI após **128 mudanças de import inválido**;
ao final, mantém menos de 12 estados de arquivo e menos de 12 índices, em vez de reter
os 128 alvos anteriores.

O oráculo de contextos ganhou um import que só o filho resolve para o output e o modo
`--without-watcher`. Nesse modo o cliente não anuncia suporte a watcher e não envia
notificações de disco; o Dart continua observando o sistema de arquivos nativamente.
O DartForge revalida as listagens a cada pedido. Resultado antes/depois:

| Cenário, resposta completa em ordem | Antes | Depois |
|---|---:|---:|
| contexto com watcher do cliente | 24/24 | 24/24 |
| contexto sem watcher do cliente | 15/24 | 24/24 |

Os dois modos passaram duas vezes, com novos diretórios de fixture: **96/96 respostas
completas iguais**. Os JSONL anteriores estão em
`/tmp/dartforge-symbol-missing-owner-before.jsonl` e
`/tmp/dartforge-symbol-missing-owner-unwatched-before.jsonl`; os posteriores, em
`/tmp/dartforge-symbol-missing-owner-{watched,unwatched}-{1,2}.jsonl`. O resultado anterior
com watcher não cobria a revalidação que apagava o registro. Para repetir o modo novo:

```sh
python3 crates/lsp/oracle/workspace_symbol_contexts.py /tmp/symbol-contexts-unwatched.jsonl target/debug/dartforge-lsp --without-watcher
python3 crates/lsp/oracle/summary.py /tmp/symbol-contexts-unwatched.jsonl --method workspace/symbol
```

A repetição isolada do `limitless_ui`, com o cache Dart quente anterior, passou em
**8/8 respostas completas**, incluindo `Ponto` com **373/373**. A nova dupla completa
dos projetos, com cache Dart novo por projeto e reaproveitado na segunda execução,
ficou assim:

| Projeto | Completa, cache Dart frio | Completa, quente | Conjunto, frio | Conjunto, quente |
|---|---:|---:|---:|---:|
| `limitless_ui` | 3/8 | 8/8 | 6/8 | 8/8 |
| core, mesma cópia corrigida | 2/8 | 8/8 | 6/8 | 8/8 |
| `new_sali/frontend` | 4/8 | 8/8 | 8/8 | 8/8 |
| Total desta dupla | **9/24** | **24/24** | **20/24** | **24/24** |

A dupla anterior dava 11/24 no frio e 20/24 no quente. A diferença negativa no frio
veio do core: sua resposta do DartForge permaneceu idêntica nas oito consultas, mas
o Dart mudou seis delas. Os digests e as contagens de arquivos continuaram iguais,
sem raízes ausentes, nos três projetos. As quatro diferenças de conjunto no frio
ficam em `Arg` e `parse` do core e do `limitless_ui`, com 500 resultados em cada lado.
Essas divergências continuam reprovadas: os 24/24 quentes não as substituem.

A execução quente foi repetida nos três projetos, reutilizando os mesmos caches:
manteve **24/24 respostas completas**, idênticas às da execução quente anterior nos
dois servidores. São **48/48** nas duas matrizes quentes desta árvore, sem conclusão
de paridade para o frio ou para outros históricos de sessão.

Os resultados estão em `/tmp/dartforge-symbol-missing-owner-{limitless,core,frontend}-{cold,warm}.jsonl`;
as repetições quentes têm sufixo `-repeat.jsonl`. Os manifestos estão em
`/tmp/dartforge-missing-owner-{limitless,core,frontend}-inputs-{before,cold,warm,repeat}.json`.
A verificação completa do LSP passou com **320 testes, zero falhas e zero ignorados**,
incluindo doctests; mantidos **112/112** no ciclo de vida e **32/32** nos quatro projetos
pequenos. Clippy de todo o workspace, formatação e documentação com avisos negados
passaram. Logs: `/tmp/dartforge-symbol-missing-owner-verified-{tests,clippy,doc}.log`.
As execuções continuam sendo de corretude, sem nova medição de desempenho.

**Próxima pendência já reproduzida na VM:** uma tabela com as chaves `/workspace` e
`/projects/catalog_2` enumera nessa ordem quando nova, mas inverte a ordem após inserir
e remover 40 chaves sem `clear`; o `clear` restaura a ordem inicial. O ensaio é da
tabela da VM, não de uma sessão LSP de 40 contextos, e está em
`/tmp/dartforge-driver-capacity-reference.json`. Como o SDK remove os drivers sem limpar
o mapa, faltava preservar esse histórico no modelo de 04/11. A continuação abaixo
amplia o ensaio para uma sessão LSP real.

### 05/11: capacidade da tabela de drivers ao reconstruir e remover contextos

`workspace_symbol_capacity.py` analisa sete projetos, cada um com uma dependência
externa não importada. Um programa executado pela VM 3.6.2 escolhe os caminhos de
modo que haja tanto uma colisão alterada pela expansão de oito para dezesseis baldes
quanto um par cuja ordem difira entre uma tabela nova e uma tabela já expandida.
O oráculo confere a ordem esperada das dependências na resposta do Dart antes de
comparar os JSON completos. Assim a prova não depende de um nome temporário por acaso
produzir o contraexemplo.

A sessão percorre seis fases: sete pastas abertas, reconstrução pelas opções de
análise, redução a duas pastas, remoção de todas, reabertura das duas e outra
reconstrução. A variante `--query-after-reduction` só faz a primeira consulta de
símbolos depois de reduzir para duas, preservando a expansão anterior da tabela.
As barreiras esperam a análise e, quando não resta driver, `FileNotAnalyzed`.
Com algum driver ativo, o Dart pode resolver arquivos fora das raízes: esse erro
não serve como barreira de remoção individual.

| Consulta de todas as dependências não importadas | Antes | Depois |
|---|---:|---:|
| Seis fases, com consultas desde a expansão | 2/6 | 6/6 |
| Primeira consulta só depois da redução | 1/4 | 4/4 |

A capacidade agora pertence à sessão, não ao índice sintático. Reconstruir insere
novamente os drivers usando a capacidade conservada; consultar sem mudança conserva
a ordem calculada, inclusive as cadeias revertidas durante uma expansão. Ao remover
a última raiz, os caminhos e índices ativos são liberados e só o número de baldes
é conservado. A captura ocorre ao abrir a coleção, antes da primeira busca.
Eventos de `pubspec.yaml`, opções, `BUILD.gn` e `.dart_tool/package_config.json`
reconstroem a coleção e invalidam a descoberta anterior. `pubspec.lock` sozinho não
é tratado como uma reconstrução da tabela.

**A matriz completa continua reprovada.** O oráculo também consulta os símbolos dos
próprios projetos e uma dependência específica: foram **13/18 respostas completas**
na sessão inteira e **9/12** na variante com primeira busca adiada. As cinco e três
diferenças, respectivamente, são da ordem dos símbolos dos projetos. Os totais
anteriores eram 12/18 e 9/12, mas os diretórios temporários diferem entre execuções;
esses totais agregados não isolam a correção. A consulta das dependências, cuja ordem
esperada é calculada para os caminhos de cada execução, passou nas dez fases. O
script conserva as diferenças nos JSONL e termina com código 1 enquanto qualquer
resposta divergir.

```sh
python3 crates/lsp/oracle/workspace_symbol_capacity.py /tmp/symbol-capacity.jsonl target/debug/dartforge-lsp
python3 crates/lsp/oracle/workspace_symbol_capacity.py /tmp/symbol-capacity-late.jsonl target/debug/dartforge-lsp --query-after-reduction
python3 crates/lsp/oracle/summary.py /tmp/symbol-capacity.jsonl --method workspace/symbol
```

Use `ORACLE_DART` e `ORACLE_SDK_LIB` do mesmo SDK 3.6.2; `ORACLE_DART_CACHE` pode
isolar o cache de referência. Evidência desta etapa:
`/tmp/dartforge-symbol-capacity-{before,after,late-before,late-after}.jsonl` e seus
logs com os mesmos nomes. As duas regressões unitárias novas usam os hashes e a
ordem já capturados da VM: removem quarenta raízes, reabrem o par e verificam que
uma nova consulta não reconstrói as colisões de uma tabela já expandida.

A suíte completa do LSP passou com **322 testes, zero falhas e zero ignorados**,
incluindo doctests e o platô de memória. Mantidos **112/112** no ciclo de vida e
**24/24** nos dois contextos com watcher. Formatação, clippy do workspace e
documentação com avisos negados passaram. Logs:
`/tmp/dartforge-symbol-capacity-{tests,clippy,doc}.log`.

Naquele ponto ainda faltavam a ordem dos arquivos adicionados nas sessões com várias
pastas e a observação de reconstruções sem eventos do cliente quando as raízes não mudam,
dependências condicionais, descoberta iniciada durante consultas e edições nos
projetos grandes. A matriz grande de 04/11 não foi repetida com esta mudança.
Recriar uma coleção agora também descarta o índice sintático e reenumera as raízes;
seu custo precisa ser medido. Esta etapa é de corretude, sem nova conclusão de
CPU, latência ou memória nos projetos grandes.

### Continuação de 05/11: ordem de criação das raízes e comparação nos mesmos caminhos

O problema dos símbolos dos próprios projetos estava numa etapa anterior ao mapa
de drivers. `ContextLocator._uniqueSortedPaths` cria um `HashSet<String>` novo com
os caminhos incluídos, enumera esse conjunto e chama `List.sort` comparando só o
comprimento UTF-16. A coleção cria os contextos nessa ordem; o mapa persistente
recebe os drivers depois. O DartForge enumerava os arquivos na ordem das pastas
recebidas, sem essa passagem pelo conjunto e pela ordenação de comprimentos.

As duas ordens agora são modeladas separadamente. A enumeração de raízes de entrada
usa o conjunto novo; a descoberta dos contextos aninhados continua em profundidade.
As raízes do cliente conservam sua ordem original para detectar mudanças: uma consulta
sem alteração não recria a coleção porque a enumeração do analisador é diferente.
O quicksort de `sdk/lib/internal/sort.dart` foi adaptado com atribuição no catálogo:
para 34 ou mais raízes ele pode mover caminhos de comprimentos iguais, portanto a
ordenação estável do Rust não seria equivalente. O teste compara os vetores completos
de 27 casos capturados da VM 3.6.2, com até 128 entradas, incluindo duplicatas, pares
UTF-16 e o limite de 33/34. Os dados e seu gerador estão no repositório:

```sh
"$ORACLE_DART" crates/lsp/oracle/context_root_order.dart crates/lsp/tests/data/context_root_order.json
```

O oráculo de capacidade ganhou `--fixture`, que conserva os caminhos absolutos entre
execuções. O diretório precisa estar vazio ou ter o marcador desse oráculo; só as
opções de análise escritas pelas fases conhecidas são removidas ao reiniciar. Não
aponte essa opção para um projeto real: ela prepara os arquivos do cenário. O seletor
da VM agora inclui a ordem de criação antes de simular o mapa de drivers; as respostas
do Dart são conferidas contra ambas as ordens esperadas.

Comparação sequencial nos mesmos caminhos, seleção de projetos e cache de referência:

| Matriz completa, todos os campos e ordem | Antes | Depois |
|---|---:|---:|
| Seis fases, primeira execução comparável | 13/18 | 18/18 |
| Repetição com a mesma configuração | 13/18 | 18/18 |
| Primeira busca só depois da redução | 9/12 | 12/12 |

As quatro execuções das seis fases têm respostas idênticas do Dart; as duas respostas
do DartForge depois também são idênticas. Na variante adiada, a resposta do Dart
permaneceu idêntica antes/depois. Esses resultados isolam a correção, ao contrário
dos totais agregados anteriores com diretórios temporários distintos. A suíte
completa ainda compara os casos vazios e consultas específicas, sem filtrar falhas.
Uma execução adicional com cache Dart isolado e frio também passou em **18/18**,
com respostas idênticas às da repetição quente nos dois servidores. Isso é evidência
deste cenário sintético; não substitui as divergências frias dos projetos grandes.

```sh
python3 crates/lsp/oracle/workspace_symbol_capacity.py /tmp/symbol-order.jsonl target/debug/dartforge-lsp --fixture /tmp/symbol-order-fixture
python3 crates/lsp/oracle/workspace_symbol_capacity.py /tmp/symbol-order-repeat.jsonl target/debug/dartforge-lsp --fixture /tmp/symbol-order-fixture
python3 crates/lsp/oracle/workspace_symbol_capacity.py /tmp/symbol-order-late.jsonl target/debug/dartforge-lsp --fixture /tmp/symbol-order-fixture --query-after-reduction
```

Evidência: `/tmp/dartforge-symbol-order-before-{3,4}.jsonl`,
`/tmp/dartforge-symbol-order-after-{1,2}.jsonl` e
`/tmp/dartforge-symbol-order-late-{before,after}.jsonl`; a execução fria está em
`/tmp/dartforge-symbol-order-cold-after.jsonl`. O binário anterior foi
preservado em `/tmp/dartforge-symbol-order-before`; os caminhos do cenário e do cache
estão em `/tmp/dartforge-symbol-order-fixture` e
`/tmp/dartforge-symbol-order-cache-path.txt`.

Passaram **323 testes do LSP, zero falhas e zero ignorados**, incluindo doctests e
platô de memória. Mantidos **112/112** no ciclo de vida e **24/24 com watcher e 24/24
sem watcher** nos dois contextos. Formatação, clippy de todo o workspace e documentação
estrita passaram. Logs: `/tmp/dartforge-symbol-root-order-{tests,clippy,doc}.log`.
Ainda faltam reconstruções não observadas por eventos, dependências condicionais,
descoberta iniciada durante consultas, edições e a nova repetição nos projetos grandes.
Não há nova medição de desempenho nesta etapa.

### Continuação de 05/11: alternativas condicionais e arquivos descobertos pela consulta

O oráculo `workspace_symbol_conditionals.py` mantém os arquivos alternativos fora
do workspace e dos catálogos de pacotes. Cada alternativa importa outro arquivo
externo. A primeira busca captura as alternativas e os dependentes das primárias;
resolver as alternativas durante essa busca descobre seus dependentes, que só
aparecem na próxima resposta. O roteiro repete consultas, troca uma URI condicional
sem salvar, reverte a edição e fecha o último documento. Todas as comparações usam
a resposta JSON completa, na ordem original.

| Roteiro de 16 consultas | Respostas completas iguais |
|---|---:|
| Antes | 4/16 |
| Depois | 16/16 |
| Repetição com o mesmo cache e os mesmos caminhos | 16/16 |
| Cache Dart isolado e frio, nos mesmos caminhos | 16/16 |

As quatro respostas corretas anteriores eram as do contexto fechado. A referência
permaneceu idêntica antes/depois e nas repetições; o DartForge depois também manteve
suas respostas nas três execuções. As contagens da primeira fase mostram a fronteira
da lista: 11 símbolos na primeira busca, 8 dependentes na consulta seguinte e 17
símbolos nas duas buscas gerais posteriores. A edição sem salvar acrescenta uma
alternativa à história conhecida; reverter a diretiva não esquece arquivos válidos
já descobertos, como no SDK.

Conferido no SDK 3.6.2, commit `b0cc5495e0f5e8ae150825a5352e708cb49e65ff`:

- `FileState._buildConfigurableDirectiveUris` cria a primária e todas as alternativas,
  mas seleciona só a primeira condição cujo valor declarado coincida;
- a coleção criada por `ContextManager` não recebe variáveis declaradas. Portanto
  nem `dart.library.io`, nem uma comparação com `false` ou string vazia, selecionam
  uma alternativa só porque o SDK contém essa biblioteca;
- `FindDeclarations.compute` captura `OwnedFiles` antes de resolver cada biblioteca,
  e `_FindDeclarations` resolve antes de aplicar o filtro do nome. Novas URIs entram
  no estado da próxima busca, inclusive se a busca atual não encontrou símbolos.

O índice agora guarda as alternativas e o contexto dono de cada arquivo. A
descoberta anterior à consulta percorre as primárias; a resolução da lista capturada
conserva os arquivos novos para a próxima consulta. Uma lista confiável que já
resolveu sem criar estados novos não precisa repetir essa passagem pelo grafo.
O bootstrap do SDK também materializa as alternativas, mantendo exports antes de
imports. As duas regressões novas verificam a ordem completa das duas listas, a
diferença entre variável ausente e falsa e a descoberta numa consulta sem resultados.

```sh
python3 crates/lsp/oracle/workspace_symbol_conditionals.py /tmp/symbol-conditionals.jsonl target/debug/dartforge-lsp --fixture /tmp/symbol-conditionals-fixture
python3 crates/lsp/oracle/summary.py /tmp/symbol-conditionals.jsonl --method workspace/symbol
```

O diretório persistente deve estar vazio ou ter o marcador próprio do oráculo;
reutilizá-lo permite manter iguais as URIs dos arquivos externos. Os resultados
estão em `/tmp/dartforge-symbol-conditional-{before,after,repeat,cold}.jsonl`.
Passaram **325 testes do LSP, zero falhas e zero ignorados**, incluindo doctests e
platô de memória. Mantidos **112/112** no ciclo de vida, **48/48** nos dois contextos
com e sem watcher, **18/18** na capacidade e **32/32** nos projetos pequenos.
Clippy de todo o workspace, formatação e documentação estrita passaram; logs em
`/tmp/dartforge-symbol-conditional-{tests,clippy,doc}.log`.

**Nova matriz grande, cache quente:** `limitless_ui` 8/8, core 8/8 e frontend 5/8:
**21/24 respostas completas**, com **24/24 conjuntos de nomes/URIs**. As 24 respostas
do DartForge são idênticas às da matriz quente anterior. As três diferenças vieram
do Dart, que mudou a ordem de `ctx`, `Ponto` e `join` no frontend. Uma nova execução
do frontend, com o mesmo cache, deu 8/8; só as respostas do Dart mudaram outra vez.
As falhas da primeira execução continuam registradas e não são substituídas pelo
8/8. Não há paridade estável entre sessões comprovada por esse resultado.

Os manifestos têm 9.761, 6.640 e 10.894 arquivos e nenhuma raiz ausente; ficaram
iguais antes/depois e também na repetição do frontend. Limitless e frontend mantêm
os digests da rodada anterior. O core foi chamado desta vez por `/private/tmp`,
em vez de `/tmp`: o digest dos caminhos textuais mudou de `aab688b0487d…` para
`8d5e65d4ab65…`, mas comparar os caminhos canônicos mostra os mesmos arquivos,
conteúdos, tamanhos e datas. Isso compara entradas; as respostas LSP continuam
comparadas sem normalizar URIs ou reordenar itens.

Evidência: `/tmp/dartforge-symbol-conditional-large-{limitless,core,frontend}.jsonl`,
`/tmp/dartforge-symbol-conditional-large-frontend-repeat.jsonl` e os manifestos
`/tmp/dartforge-symbol-conditional-large-{limitless,core,frontend}-inputs-{before,after}.json`;
o manifesto da repetição tem sufixo `-frontend-inputs-repeat.json`. A dupla fria dos
projetos grandes não foi repetida nesta etapa. Permanecem a variação de descoberta
entre sessões, reconstruções sem eventos observados, edições nos projetos grandes
e a verificação de URIs relativas que tentam sair da raiz de um `package:`. Essas
verificações vêm antes de repetir CPU, latência e memória; as execuções atuais são
de corretude, com ferramentas de verificação concorrentes em parte da rodada.

### Continuação de 05/11: fronteiras de `package:`, escapes e fontes sombreadas

O índice de `workspace/symbol` resolvia URIs relativas pelo caminho físico. Isso
não equivale a resolver a URI canônica: em `package:p/src/entry.dart`,
`../../target.dart` e `/target.dart` resolvem para `package:p/target.dart`.
O caminho físico podia encontrar um arquivo fora de `lib/`, que não deveria
participar da descoberta. Agora a resolução mantém a fronteira do pacote antes
de escolher o arquivo fonte ou seu gerado. URIs `file:` conservam a resolução
de caminhos de arquivo.

O novo `workspace_symbol_uris.py` coloca arquivos de controle fora de `lib/` e
dependências externas dos alvos corretos. Assim a primeira resposta prova quais
URIs foram resolvidas antes da busca; as respostas seguintes verificam também
a descoberta causada pela própria consulta. O roteiro cobre raiz com `/`, excesso
de `..`, espaço escapado, barra escapada, UTF-8, query, fragmento e gerado.

| Cenário | Antes do ajuste correspondente | Depois |
|---|---:|---:|
| Fronteiras, escapes e gerado | 4/12 | 12/12 |
| Mesmo cenário sem watcher do cliente | — | 12/12 |
| Fonte não-lib sombreada, antes/depois de importá-la | 7/16 | 16/16 |
| Mesma sombra, sem watcher | — | 16/16 |
| Mesma sombra, cache Dart isolado e frio | — | 16/16 |

O baseline de 7/16 é do binário intermediário que já corrigia as URIs; ele ainda
adicionava uma fonte real cujo caminho canônico apontava para outro arquivo.
As referências antes/depois ficaram idênticas em cada comparação. As três
execuções finais da variante de sombra também têm respostas idênticas nos dois
servidores. As quatro respostas fechadas e as consultas específicas continuam
na matriz; não são usadas para esconder diferenças das consultas gerais.

Pontos conferidos no SDK 3.6.2:

- `Uri.resolveUri` mantém o nome do pacote antes de remover segmentos `.` e `..`
  de referências relativas. A normalização de uma URI absoluta tem suas regras
  próprias; os vetores de referência conferem as duas operações separadamente;
- `PackageMapUriResolver` decodifica os segmentos do caminho. Em contraste,
  `PackageConfigPackageUriResolver` usa `Uri.path` ainda escapado no fallback de
  `.dart_tool/build/generated`. Um output com espaço físico no nome não satisfaz
  por si só esse fallback escrito com `%20`;
- `SourceFactory.forUri2` exige `Uri.isAbsolute`, que exclui fragmento não vazio.
  Uma query sem fragmento não impede a resolução de pacote. O arquivo cujo import
  tem fragmento ainda pode ser descoberto pela enumeração do catálogo, mas suas
  dependências não entram na primeira resposta por esse import;
- `AnalysisDriver.addFile` chama `FileSystemState.hasUri`: o caminho deve mapear
  para uma URI que resolva de volta para ele mesmo. Uma fonte de `test/` cujo `file:`
  aponta para o gerado não é adicionada só porque existe no disco. Quando o editor
  importa essa URI, o gerado passa a ser conhecido e seus símbolos aparecem.

O modelo agora verifica essa ida e volta ao adicionar os arquivos e não percorre
o corpo da fonte recusada como se ela tivesse sido analisada. Pacotes continuam
preferindo a fonte existente ao fallback gerado; a resolução `file:` do workspace
tem a precedência de gerado própria do SDK. A regressão testa ambas as regras,
inclusive uma fonte de `lib/` e uma fonte não-lib sombreadas.

A normalização ficou em `workspace_symbols/uri_resolution.rs`, com a atribuição
da remoção de segmentos do Dart em `THIRD_PARTY_NOTICES.md`. Foram adicionadas
quatro regressões, uma delas comparando 60 vetores completos capturados da VM:
limites de raiz, escapes canônicos e malformados, Unicode, barras, caminhos
com espaços, query e referência com autoridade. O gerador é reproduzível:

```sh
"$ORACLE_DART" crates/lsp/oracle/package_uri_paths.dart crates/lsp/tests/data/package_uri_paths.json
python3 crates/lsp/oracle/workspace_symbol_uris.py /tmp/symbol-uris.jsonl target/debug/dartforge-lsp --fixture /tmp/symbol-uri-fixture
python3 crates/lsp/oracle/workspace_symbol_uris.py /tmp/symbol-shadow.jsonl target/debug/dartforge-lsp --fixture /tmp/symbol-shadow-fixture --source-shadow
python3 crates/lsp/oracle/workspace_symbol_uris.py /tmp/symbol-shadow-unwatched.jsonl target/debug/dartforge-lsp --fixture /tmp/symbol-shadow-fixture --source-shadow --without-watcher
```

Use fixtures próprias desse oráculo, vazias na primeira execução, e execute as
repetições de cada fixture sequencialmente. Configure `ORACLE_DART` e
`ORACLE_SDK_LIB` do mesmo SDK 3.6.2; `ORACLE_DART_CACHE` permite isolar o cache.
O índice passou a depender diretamente de `percent-encoding`, que já era uma
dependência transitiva fixada em 2.3.2: nenhuma versão do lockfile foi atualizada.

Evidência principal: `/tmp/dartforge-symbol-uri-{before,final,final-unwatched}.jsonl`
e `/tmp/dartforge-symbol-uri-shadow-{before,after,unwatched,cold}.jsonl`. O binário
intermediário está em `/tmp/dartforge-symbol-uri-before-shadow-fix`.
Passaram **329 testes do LSP, zero falhas e zero ignorados**, incluindo doctests e
platô de memória. Mantidos **112/112** no ciclo de vida, **48/48** nos dois contextos,
**18/18** na capacidade, **16/16** nas condicionais e **32/32** nos projetos pequenos.
Formatação, clippy do workspace e documentação estrita passaram. Logs finais:
`/tmp/dartforge-symbol-uri-final-{tests,clippy,doc}.log`.

**Projetos grandes:** as duas matrizes quentes desta etapa deram **21/24 respostas
completas e 24/24 conjuntos**, com respostas idênticas entre as duas execuções nos
dois servidores: `limitless_ui` 8/8, core 8/8 e frontend 5/8. As respostas do DartForge
também são idênticas às da etapa anterior. As três diferenças de ordem do frontend
permanecem reprovadas. Os manifestos de 9.761, 6.640 e 10.894 arquivos ficaram
idênticos antes/depois e entre as matrizes, sem raízes ausentes. Resultados em
`/tmp/dartforge-symbol-uri-{large,final-large}-{limitless,core,frontend}.jsonl`;
os manifestos têm sufixos `-inputs-before.json` e `-inputs-after.json`.

Ainda faltam as edições e a dupla fria nos projetos grandes, a investigação da
ordem entre sessões e da barreira de análise inicial, reconstruções sem eventos
observados e mudanças dinâmicas de sombra que alterem a URI canônica. Também falta
ampliar a cobertura de nomes com escapes literais. A checagem de ida e volta tem
custo na abertura do índice, ainda sem medição; esta etapa não repete latência,
CPU ou memória dos projetos grandes.

### Continuação: preparação explícita e rastros de status do oráculo (05/11)

A espera inicial antiga observa uma conclusão de análise do Dart e aguarda mais
um segundo. Isso não estabelece que os dois servidores terminaram o trabalho
inicial. A auditoria `workspace_symbol_readiness.py` abre o mesmo primeiro arquivo
de `lib/` ou `bin/` que `ORACLE_OPEN_FILES=1`, conserva as oito consultas e as
respostas brutas, e registra o protocolo em um arquivo `.trace.json` separado.
O rastro contém métodos, identificadores, status e contagens, sem texto fonte nem
corpos completos das respostas. O processo falha se qualquer lista completa,
incluindo ordem, campos e duplicatas, diferir.

Foram executadas três sessões do frontend por modo, sequencialmente, com o mesmo
SDK 3.6.2, binário e diretório de cache Dart. Cada modo acrescenta uma preparação
distinta; os resultados não são intercambiáveis:

| Modo | Preparação antes das consultas | Respostas completas iguais nas três sessões |
| --- | --- | --- |
| `legacy` | espera antiga por status e um segundo | 4/8, 8/8, 8/8 |
| `cycle` | exige também um novo ciclo ocupado/ocioso do Dart | 8/8, 8/8, 8/8 |
| `resolved` | ciclo, depois `documentSymbol` do documento aberto nos dois servidores | 8/8, 8/8, 8/8 |
| `settled` | espera antiga, `documentSymbol` nos dois e intervalo ocioso conjunto | 8/8, 8/8, 8/8 |

Nas nove sessões dos três primeiros modos, o DartForge ainda reportava análise
em andamento imediatamente antes das buscas. Na primeira sessão `legacy`, o
Dart tinha anunciado apenas um ciclo ocupado/ocioso, de cerca de 2,5 ms; voltou a
analisar durante a primeira busca. Nas demais oito, já havia anunciado dois
ciclos. Nas três sessões `settled`, ambos reportavam ociosidade antes das buscas,
após cerca de 5,6–5,7 s desde o início do cliente. Novos ciclos durante as buscas
continuam possíveis pela resolução sob demanda; o intervalo observado não é uma
garantia de que nunca haverá trabalho futuro.

As respostas do DartForge foram idênticas nas 12 sessões. Na primeira `legacy`,
`Scanner`, `ctx`, `Ponto` e `join` diferiram na ordem; os multiconjuntos de itens
completos eram iguais nas oito consultas. Isso ajuda a localizar a diferença,
mas não muda a reprovação da lista ordenada. Fontes e configurações ficaram
iguais; o conteúdo interno do cache Dart foi reutilizado sem congelamento.
Portanto, a auditoria não atribui a variação apenas à espera inicial. As três
diferenças das matrizes anteriores de **21/24** permanecem registradas.

O oráculo geral agora aceita `ORACLE_READINESS=legacy|settled`, mantendo `legacy`
como padrão. `settled` envia os mesmos pedidos `textDocument/documentSymbol`
para cada documento aberto aos dois servidores, repetindo apenas o erro
transitório `FileNotAnalyzed` (-32007). Depois exige que ambos tenham informado
ociosidade por um segundo, com contadores para detectar ciclos entre as
observações. Status desconhecido não vale como ocioso. Um prazo absoluto limita
a resolução e o intervalo conjunto. A linha `oracle/initialReadiness` guarda as contagens e os
estados separadamente; `summary.py` não a inclui nas amostras das funcionalidades.
O modo é registrado também nas linhas de inicialização. Nenhuma resposta de
símbolos é normalizada, ordenada ou filtrada pelo novo modo.

No oráculo geral, essa preparação deu **8/8 em cada projeto grande (24/24)**,
com ambos ociosos nos metadados. Os controles pelo protocolo antigo deram
**32/32** nos quatro projetos pequenos e **16/16** na fonte sombreada sem watcher.
Os manifestos posteriores conservam os 9.761, 6.640 e 10.894 arquivos e os mesmos
digests da etapa anterior, sem raízes ausentes. Nenhum código Rust foi alterado;
esses resultados validam o protocolo novo, sem fechar a paridade no protocolo
antigo nem substituir a dupla fria ainda pendente.

Também foram corrigidos dois problemas do cliente compartilhado: notificações
sem resposta não reiniciam mais o timeout de um pedido, e o intervalo ocioso da
espera antiga não ultrapassa o prazo total. O encerramento aguarda a saída do
processo após o fallback com `kill`. **11 testes Python** passaram sem SDK, com
relógio simulado para os prazos e transições; incluem erros inesperados não
ocultados, sondas iguais nos dois servidores, preservação dos quadros JSON-RPC e
exclusão dos metadados da contagem de amostras. Os **329 testes Rust** e os gates
do workspace registrados acima são da etapa anterior, no commit `38edff4f`;
não foram repetidos para esta alteração exclusiva do oráculo e documentação.

Reprodução, com `ORACLE_DART`, `ORACLE_SDK_LIB` e `ORACLE_DART_CACHE` configurados:

```sh
python3 crates/lsp/oracle/workspace_symbol_readiness.py \
  /tmp/readiness-legacy.jsonl target/debug/dartforge-lsp /caminho/projeto --mode legacy
python3 crates/lsp/oracle/workspace_symbol_readiness.py \
  /tmp/readiness-settled.jsonl target/debug/dartforge-lsp /caminho/projeto --mode settled
ORACLE_READINESS=settled ORACLE_METHODS=symbol ORACLE_OPEN_FILES=1 \
  python3 crates/lsp/oracle/oracle.py /tmp/readiness-main.jsonl \
  target/debug/dartforge-lsp /caminho/projeto
python3 -m unittest discover -s crates/lsp/oracle -p 'test_*.py' -v
```

Evidências: `/tmp/dartforge-readiness-{legacy,cycle,resolved,settled}-{1,2,3}.jsonl`,
com os rastros de mesmo nome terminados em `.trace.json`;
`/tmp/dartforge-readiness-main-{limitless,core,frontend}.jsonl`;
`/tmp/dartforge-readiness-{legacy-small,shared-protocol}.jsonl`;
`/tmp/dartforge-readiness-main-inputs.log` e
`/tmp/dartforge-readiness-unit.log`. Estes ensaios não medem desempenho: ainda
faltam latência, CPU e memória em condições equivalentes e sem concorrência.

### Continuação: normalizar os catálogos antes das buscas de símbolos (05/11)

Um perfil do binário `release`, sem watcher, mostrou trabalho repetido na
resolução dos imports: os caminhos de `package_config.json` conservavam longas
sequências de `..`, e cada comparação com uma raiz de pacote reconstruía seu
caminho normalizado. `Context::load` agora normaliza os diretórios `lib/` dos
pacotes e as raízes fonte dos mapeamentos de gerados ao carregar o catálogo.
A ordem dos pacotes, a seleção das bibliotecas e a revalidação do disco foram
preservadas. Sem watcher, o catálogo continua sendo relido; a alteração não
introduz um prazo adicional durante o qual mudanças ficariam invisíveis.

O perfil diagnóstico está em `/tmp/dartforge-symbol-perf-before-profile.txt`.
Seus tempos não entram na comparação de desempenho. O baseline é o commit
`34ab3f5b`; os dois binários foram construídos com
`cargo build --locked --release -p dartforge-lsp --bin dartforge-lsp`, no mesmo
ambiente e sem alteração dos perfis. Cópias preservadas:
`/tmp/dartforge-symbol-perf-{before,after}` (SHA-256 iniciados em `66ea1d95dd73`
e `5278bd3cee9f`, respectivamente).

O novo `workspace_symbol_performance.py` executa um servidor por vez, conserva
as listas completas e mede recursos da árvore de processos no macOS. O cliente
não anuncia watcher, abre o mesmo primeiro documento do oráculo anterior,
resolve seus símbolos e observa dez segundos adicionais de ociosidade. São
três passagens das oito consultas `Arg`, `parse`, `Scanner`, `ctx`, `Ponto`,
`join`, `a` e `e`. A primeira consulta `Arg` inclui a construção do índice;
as duas últimas passagens dão 16 amostras quentes por sessão. A CPU soma usuário
e sistema do servidor, dos descendentes vivos e dos filhos encerrados já
recolhidos. A memória usa o footprint físico, com RSS também guardada nos JSONL;
as amostras nos limites dos pedidos e no repouso final não medem um pico
simultâneo de toda a árvore nem provam estabilidade numa sessão longa.

Foram **27 sessões**, três por servidor e projeto, executadas sequencialmente,
alternando `antes/depois/Dart`, `depois/Dart/antes` e `Dart/antes/depois`. Compilações,
testes e perfil diagnóstico terminaram antes da matriz. Cada mediana quente usa
48 pedidos (16 por sessão); a primeira busca usa três. O cliente não anuncia
watcher, mas o Dart observa o sistema de arquivos nativamente. Nesse cenário,
o DartForge ainda reenumera e revalida os arquivos em cada busca.

Medianas quentes em **ms de latência / ms de CPU da árvore**:

| Projeto | Antes | Depois | Dart 3.6.2 |
| --- | ---: | ---: | ---: |
| `limitless_ui` | 595,5 / 595,0 | 385,7 / 385,2 | 17,1 / 21,4 |
| core | 275,4 / 274,5 | 158,3 / 157,7 | 13,3 / 13,6 |
| frontend | 877,2 / 876,4 | 615,0 / 613,6 | 18,9 / 26,3 |

A queda de latência/CPU foi de aproximadamente **35% no `limitless_ui`, 43% no
core e 30% no frontend**. Todos os três ainda perdem do Dart nesse protocolo.
A construção inicial e a memória não foram descartadas da coleta:

| Projeto | Primeira busca: antes / depois / Dart (ms) | Footprint final: antes / depois / Dart (MiB) |
| --- | ---: | ---: |
| `limitless_ui` | 1.831 / 1.551 / 731 | 137,6 / 137,2 / 674,4 |
| core | 1.124 / 961 / 529 | 127,8 / 125,2 / 503,2 |
| frontend | 2.393 / 2.037 / 598 | 586,7 / 589,6 / 712,1 |

Não houve redução consistente de memória: no frontend, a mediana subiu 0,5%,
com intervalos das sessões sobrepostos (antes 579,4–592,2 MiB; depois
582,6–589,6 MiB). O footprint final continua menor que o Dart nos três projetos,
mas esse ensaio curto não substitui as ondas de gerados nem o platô da sessão
longa. A primeira busca também continua mais lenta que a referência.

**Respostas completas:** as 72 por projeto coincidiram nas três comparações
`antes/depois`, `antes/Dart` e `depois/Dart`: **216/216 em cada comparação**,
sem alterar ordem, duplicatas, URIs ou campos. Este protocolo acrescenta uma
espera de dez segundos à preparação estabilizada; não transforma os resultados
antigos de **21/24** em aprovações nem valida cache Dart frio. Os manifestos
anteriores e posteriores foram idênticos, inclusive aos da etapa de preparação:
9.761, 6.640 e 10.894 arquivos, sem raízes ausentes e com os mesmos digests.

Reprodução de uma sessão, com os executáveis e o SDK selecionados explicitamente:

```sh
ORACLE_DART=/sdk/3.6.2/bin/dart ORACLE_SDK_LIB=/sdk/3.6.2/lib \
  ORACLE_DART_CACHE=/tmp/cache-do-projeto \
  python3 crates/lsp/oracle/workspace_symbol_performance.py \
  /caminho/dartforge-lsp /caminho/projeto /tmp/symbol-performance.jsonl
```

Use `dart` no primeiro argumento para a referência. Repita com os dois binários
e a referência, alternando a ordem entre sessões. `--profile /tmp/sample.txt`
é diagnóstico; sua linha de inicialização marca `profiled: true`, e seus tempos
devem ser excluídos da comparação.

Artefatos: `/tmp/dartforge-symbol-perf-matrix-{limitless,core,frontend}-{before,after,dart}-{0,1,2}.jsonl`,
logs de mesmo nome, `/tmp/dartforge-symbol-perf-{matrix,summary}.log` e
`/tmp/dartforge-symbol-perf-{limitless,core,frontend}-inputs-{before,after}.json`.
Passaram novamente **329 testes do LSP, zero falhas e zero ignorados**, os
**11 testes Python**, formatação, clippy do workspace sem avisos e documentação
estrita. Logs em `/tmp/dartforge-symbol-perf-{tests,python,clippy,doc}.log`.
Após terminar as medições, passaram **270/270 controles completos**: ciclo de
vida normal e criação antes da primeira busca (112), catálogos com e sem watcher
(48), URIs e fontes sombreadas (44), condicionais (16), capacidade (18) e os
projetos pequenos pelo protocolo antigo (32). Arquivos em
`/tmp/dartforge-symbol-perf-control-*.jsonl` e
`/tmp/dartforge-symbol-perf-controls.log`.

### Continuação: parts ausentes e validação por pedido (05/11)

O índice só guardava os fingerprints dos arquivos que conseguia ler. Se uma
biblioteca declarasse `part 'generated.g.dart'` antes de esse arquivo existir,
a criação posterior do part não alterava nenhum fingerprint guardado. Uma busca
aquecida continuava omitindo as declarações, mesmo quando uma análise do zero
já as encontrava. A regressão unitária reproduziu a diferença antes da correção.

`Fingerprint::Missing` agora registra também as tentativas de leitura sem
arquivo disponível, inclusive os parts. A passagem de ausente para texto aberto
ou arquivo em disco invalida a biblioteca; o fechamento do texto e a remoção do
arquivo também entram na comparação. Arquivos existentes que não puderam ser
lidos voltam a ser tentados nas buscas que revalidam o disco.

O novo `workspace_symbol_parts.py` mantém o texto da biblioteca dona inalterado.
As nove fases são: part ausente, repetição, abertura de um part ainda inexistente
no disco, fechamento desse buffer, criação, edição, remoção, recriação e
fechamento do último documento. Cada fase faz três consultas. Elas verificam
classes, construtores nomeados, campos, métodos, contêineres e intervalos, sem
reduzir as respostas a nomes nem normalizar as URIs.

| Cenário | Antes (`cbc70021`) | Depois |
| --- | ---: | ---: |
| Com watcher | 15/27 | 27/27 |
| Sem watcher | 15/27 | 27/27 |
| Cache Dart frio, com watcher | não repetido antes | 27/27 |

As respostas do Dart foram idênticas nas cinco execuções, e os três resultados
finais do DartForge também coincidiram integralmente. As 12 falhas de cada
baseline eram nas fases de abertura em memória, criação, edição e recriação.
O cache frio acima é o dessa fixture pequena, não uma dupla fria dos projetos
grandes.

A validação da biblioteca também passou a ser compartilhada entre as etapas
de uma mesma busca. Cada entrada guarda o número do pedido em que foi conferida;
a próxima busca usa outro número e volta a verificar os fingerprints quando o
disco não é confiável. O marcador tem tamanho fixo, sem coleção de caminhos por
pedido nem histórico crescente. Ao atingir o limite do contador, os marcadores
são zerados antes de reutilizar seu número inicial. Outra regressão força esse
limite, edita o disco e exige a resposta completa de uma análise do zero.
As consultas aos textos abertos também usam a chave normalizada emprestada,
sem copiá-la apenas para procurar no mapa. Isso não muda o prazo de confiança
do watcher nem dispensa a checagem dos overlays.

Passaram **331 testes do LSP, zero falhas e zero ignorados**, os **11 testes
Python**, formatação, clippy do workspace sem avisos e documentação estrita.
Logs finais: `/tmp/dartforge-symbol-validation-counter-{tests,python,clippy,doc}.log`.
O teste falhando antes está em `/tmp/dartforge-symbol-part-before-test.log`;
as respostas estão em `/tmp/dartforge-symbol-parts-{before,before-unwatched,after,after-unwatched,after-cold}.jsonl`.
O binário anterior preservado é `/tmp/dartforge-symbol-perf-after`, do commit
`cbc70021`; o novo é `/tmp/dartforge-symbol-validation-counter`.

```sh
python3 crates/lsp/oracle/workspace_symbol_parts.py \
  /tmp/parts.jsonl target/release/dartforge-lsp --fixture /tmp/parts-fixture
python3 crates/lsp/oracle/workspace_symbol_parts.py \
  /tmp/parts-unwatched.jsonl target/release/dartforge-lsp \
  --fixture /tmp/parts-fixture --without-watcher
```

Configure `ORACLE_DART` e `ORACLE_SDK_LIB` para o SDK 3.6.2. A fixture deve estar
vazia ou ter sido criada por esse oráculo; ele só reutiliza diretórios com seu
marcador de propriedade. `ORACLE_DART_CACHE` permite isolar a repetição fria.
O driver de desempenho agora aceita `--with-watcher` e registra `watchedFiles`
na inicialização; sem a opção, conserva o protocolo anterior.

**Desempenho sem watcher:** três sessões por servidor/projeto, nas mesmas
condições da etapa anterior, alternaram o baseline `cbc70021`, o novo binário e
o Dart. As 27 sessões deram **216/216 respostas completas iguais em cada uma
das três comparações**. As medianas quentes abaixo usam 48 pedidos por célula;
os valores são **latência / CPU da árvore, em ms**:

| Projeto | Antes | Depois | Dart 3.6.2 |
| --- | ---: | ---: | ---: |
| `limitless_ui` | 381,6 / 381,1 | 366,8 / 366,4 | 16,0 / 20,6 |
| core | 155,1 / 154,6 | 145,1 / 144,7 | 12,8 / 13,2 |
| frontend | 607,5 / 606,9 | 590,2 / 589,3 | 18,5 / 25,8 |

A redução foi de aproximadamente **4%, 6% e 3%**, respectivamente. A melhora
é pequena e os três projetos continuam perdendo do Dart nesse cenário. A
primeira busca e o footprint final também foram registrados (medianas de três
sessões, antes / depois / Dart):

| Projeto | Primeira busca (ms) | Footprint final (MiB) |
| --- | ---: | ---: |
| `limitless_ui` | 1.548 / 1.525 / 727 | 137,5 / 138,4 / 666,1 |
| core | 962 / 954 / 529 | 125,3 / 126,3 / 499,5 |
| frontend | 2.003 / 1.985 / 581 | 585,0 / 585,8 / 709,6 |

Não há ganho de memória nessa matriz: a mediana subiu cerca de 0,8–1,0 MiB,
ainda abaixo do Dart. O marcador e os estados ausentes têm custo fixo por
entrada; a ausência de histórico não prova, por si só, o platô no projeto real.
As amostras finais continuam sendo de sessões curtas, sem ondas de gerados.
Artefatos: `/tmp/dartforge-symbol-validation-matrix-{limitless,core,frontend}-{before,after,dart}-{0,1,2}.jsonl`
e `/tmp/dartforge-symbol-validation-{matrix,summary}.log`. O novo binário tem
SHA-256 `930d86309eaf81fcd093f056a3a4f604506dbe334a146302f7b4e5a2882666b7`.

**Com watcher:** outras 27 sessões repetiram a rotação com `--with-watcher`,
também dando **216/216 respostas completas iguais em cada comparação**. As
medianas quentes (latência / CPU da árvore, ms) foram:

| Projeto | Antes | Depois | Dart 3.6.2 |
| --- | ---: | ---: | ---: |
| `limitless_ui` | 12,0 / 11,6 | 12,2 / 11,7 | 16,4 / 18,2 |
| core | 9,4 / 8,9 | 9,1 / 8,7 | 12,8 / 13,0 |
| frontend | 12,7 / 12,2 | 12,7 / 12,2 | 18,2 / 23,2 |

As variações são pequenas e não demonstram ganho geral nesse modo: a latência
mediana do `limitless_ui` subiu 0,2 ms, enquanto as medianas de suas sessões
individuais tiveram faixas sobrepostas (11,88–12,19 antes; 11,67–12,58 depois).
Os pedidos quentes continuam menores que o Dart em latência e CPU nos três
projetos, comportamento que o baseline já tinha. A construção inicial continua
mais lenta que a referência:

| Projeto | Primeira busca: antes / depois / Dart (ms) | Footprint final: antes / depois / Dart (MiB) |
| --- | ---: | ---: |
| `limitless_ui` | 1.544 / 1.517 / 729 | 136,3 / 137,1 / 673,1 |
| core | 951 / 950 / 528 | 124,3 / 125,7 / 506,0 |
| frontend | 1.998 / 1.981 / 580 | 584,7 / 584,0 / 708,6 |

Essa segunda matriz também não estabelece redução de memória: o core subiu
1,4 MiB na mediana final. As duas capacidades do cliente ficam separadas nos
resultados; os números quentes com watcher não substituem a revalidação sem
watcher, a primeira busca ou a sessão longa. Nenhuma das matrizes substitui os
resultados antigos de 21/24 nem fecha a dupla fria dos projetos grandes.
As 54 sessões terminaram sem compilações, testes ou amostragem de perfil
concorrentes. Seus manifestos anteriores/posteriores são idênticos aos da
etapa anterior, com os mesmos 9.761, 6.640 e 10.894 arquivos e nenhuma raiz
ausente.

Após as medições, os **270/270 controles completos** foram repetidos:
112 do ciclo de vida, 48 dos catálogos, 44 das URIs e sombras, 16 condicionais,
18 de capacidade e 32 dos projetos pequenos. Somam-se aos **81/81** da nova
fixture de parts, sem tratá-los como cobertura de todas as edições reais.
Arquivos em `/tmp/dartforge-symbol-validation-control-*.jsonl` e
`/tmp/dartforge-symbol-validation-controls.log`. A matriz com watcher está em
`/tmp/dartforge-symbol-validation-watched-matrix-{limitless,core,frontend}-{before,after,dart}-{0,1,2}.jsonl`,
com resumo `/tmp/dartforge-symbol-validation-watched-summary.log`;
os manifestos são `/tmp/dartforge-symbol-validation-{limitless,core,frontend}-inputs-{before,after}.json`.

Um novo perfil, coletado depois dos controles e excluído das medianas, ainda
mostra a resolução dos imports comparando repetidamente prefixos de caminhos
e conferindo suas chaves normalizadas. Esse percurso e as enumerações do disco
continuam sendo alvos para as buscas sem watcher. Perfil em
`/tmp/dartforge-symbol-validation-profile.txt`; seu JSONL tem `profiled: true`.

### Continuação: caminhos compartilhados e bases relativas por busca (05/11)

O perfil da etapa anterior mostrava a mesma unidade comparando seu caminho
com os diretórios de pacotes a cada import relativo. `RelativeBases` agora
calcula essa base uma vez por unidade e contexto dentro da busca. A chave
inclui o contexto: o mesmo arquivo físico pode ter uma URI de pacote diferente
em outro catálogo. A base é o caminho canônico do pacote ou a URL de arquivo;
não inclui a escolha final entre fonte e gerado. Essa escolha continua
conferindo a existência dos candidatos em cada resolução, inclusive a
precedência distinta de `file:` e `package:`. As URIs absolutas e os fragmentos
continuam passando pela mesma validação antes de usar qualquer base relativa.

Cada unidade também compartilha seu `Arc<PathBuf>` entre imports e exports,
em vez de copiar o caminho completo em cada diretiva e nas cópias de
`OpenDependencies`. A comparação dos caminhos continua por valor; a identidade
do ponteiro não decide ordem nem invalidação. Unidades sem imports ou exports
não criam essa alocação. A tabela de bases é local ao pedido e é descartada ao
terminar a busca; não retém catálogos, versões ou históricos de sessões antigas.

Duas regressões novas verificam o mesmo arquivo sob raízes de pacotes diferentes
e a criação/remoção de fonte e gerado sem trocar a base. Esta última repete a
resolução com a mesma tabela e exige que o alvo mude com o disco, incluindo
`file:` preferindo o gerado quando `package:` prefere a fonte. Os 60 vetores
da VM Dart 3.6.2 para caminhos de URIs foram mantidos.

Passaram **333 testes do LSP, zero falhas e zero ignorados**, os **11 testes
Python**, formatação, clippy do workspace sem avisos e documentação estrita.
Os controles completos mantiveram **270/270** e o cenário de parts deu
**27/27 com e 27/27 sem watcher**. Logs em
`/tmp/dartforge-symbol-bases-{tests,python,clippy,doc,controls}.log` e respostas
em `/tmp/dartforge-symbol-bases-control-*.jsonl` e
`/tmp/dartforge-symbol-bases-{parts,parts-unwatched}.jsonl`.

O baseline desta comparação é `8c509e11`, preservado em
`/tmp/dartforge-symbol-bases-before` (SHA-256 iniciado em `930d86309eaf`);
o novo binário é `/tmp/dartforge-symbol-bases-after`, SHA-256
`1600d9ebf850cac903de9ddadd59a921a86ceb01d610e478637ba6ee28e16f70`.
Ambos usam o mesmo `release`, SDK 3.6.2, arquivos e capacidades do cliente.
Foram 54 sessões sequenciais, três por servidor/projeto em cada modo, alternando
antes/depois/Dart, depois/Dart/antes e Dart/antes/depois. Compilações, testes e
perfis não concorreram com as medições. A preparação e as oito consultas são
as do driver de desempenho, incluindo dez segundos adicionais de ociosidade.

**Sem watcher**, as medianas quentes (48 pedidos por célula, latência / CPU da
árvore em ms) foram:

| Projeto | Antes | Depois | Dart 3.6.2 |
| --- | ---: | ---: | ---: |
| `limitless_ui` | 353,9 / 353,3 | 180,6 / 180,2 | 16,0 / 20,4 |
| core | 142,1 / 141,5 | 72,4 / 71,9 | 12,6 / 13,3 |
| frontend | 580,1 / 579,5 | 239,4 / 238,9 | 18,7 / 25,5 |

A queda foi de aproximadamente **49%, 49% e 59%** em latência e CPU. Os três
ainda perdem do Dart nesse protocolo. A primeira busca também melhorou, mas
permaneceu mais lenta; os valores abaixo são medianas de três sessões:

| Projeto | Primeira busca: antes / depois / Dart (ms) | Footprint final: antes / depois / Dart (MiB) |
| --- | ---: | ---: |
| `limitless_ui` | 1.478 / 1.300 / 713 | 138,0 / 135,5 / 674,6 |
| core | 922 / 846 / 518 | 126,3 / 123,8 / 499,8 |
| frontend | 1.963 / 1.612 / 566 | 586,2 / 584,5 / 711,0 |

**Com watcher**, as medianas quentes foram:

| Projeto | Antes | Depois | Dart 3.6.2 |
| --- | ---: | ---: | ---: |
| `limitless_ui` | 11,7 / 11,2 | 11,8 / 11,3 | 15,8 / 17,6 |
| core | 8,9 / 8,5 | 9,4 / 9,0 | 12,6 / 12,8 |
| frontend | 12,6 / 12,0 | 12,8 / 12,3 | 18,4 / 23,0 |

Não houve ganho quente nesse modo. O core subiu cerca de 0,5 ms em latência e
CPU; essa regressão pequena continua aberta, mesmo com o novo binário ainda
abaixo do Dart. Os demais aumentos foram de 0,1–0,3 ms. A etapa não satisfaz
o critério de melhorar todos os cenários. Primeira busca e memória final:

| Projeto | Primeira busca: antes / depois / Dart (ms) | Footprint final: antes / depois / Dart (MiB) |
| --- | ---: | ---: |
| `limitless_ui` | 1.484 / 1.306 / 707 | 137,4 / 134,9 / 673,4 |
| core | 921 / 851 / 514 | 125,2 / 123,2 / 505,3 |
| frontend | 1.936 / 1.595 / 558 | 585,0 / 583,2 / 710,7 |

O footprint final caiu cerca de 1,7–2,5 MiB nas duas matrizes. Esse resultado
curto não mede o pico simultâneo da árvore nem substitui a sessão longa com
ondas de gerados. A primeira busca permanece mais lenta que a referência nos
comparativos revezados.

Cada matriz deu **216/216 respostas completas iguais em cada comparação**
antes/depois, antes/Dart e depois/Dart. Os manifestos anteriores e posteriores
ficaram idênticos aos da etapa anterior: 9.761, 6.640 e 10.894 arquivos, sem
raízes ausentes. Artefatos em
`/tmp/dartforge-symbol-bases-{matrix,watched-matrix}-{limitless,core,frontend}-{before,after,dart}-{0,1,2}.jsonl`,
resumos `/tmp/dartforge-symbol-bases-{summary,watched-summary}.log` e manifestos
`/tmp/dartforge-symbol-bases-{limitless,core,frontend}-inputs-{before,after}.json`.

**Cache Dart frio/quente, protocolo estabilizado:** depois das matrizes,
outras 12 sessões usaram um diretório inicialmente vazio para cada projeto.
Primeiro rodou o Dart no cache vazio e depois o DartForge; na repetição, a
ordem foi DartForge/Dart, reutilizando o mesmo cache. Cada processo DartForge
começou sem índice, pois esse índice não é persistido. Os diretórios passaram
de zero para 13.609, 10.547 e 16.911 arquivos após o primeiro ciclo; a repetição
acrescentou um arquivo em cada um, confirmando que o cache não era fictício.
Os manifestos desses diretórios são `/tmp/dartforge-symbol-bases-cache-*.json`.

Foram **72/72 respostas completas no frio e 72/72 no quente**, 24 por projeto
em cada estado. As respostas de cada servidor ficaram idênticas entre os dois
estados. Isso amplia a cobertura desse protocolo com dez segundos adicionais
de ociosidade; não altera os resultados legados de 9/24 no frio ou 21/24 nas
matrizes quentes. O protocolo legado ainda precisa de nova rodada na árvore
atual. Estes pares novos não entram nas medianas de desempenho das matrizes
revezadas. Respostas em
`/tmp/dartforge-symbol-bases-state-{limitless,core,frontend}-{cold,warm}-{dart,dartforge}.jsonl`
e log `/tmp/dartforge-symbol-bases-cold-warm.log`.

Um perfil separado posterior às medições, com `profiled: true`, ainda mostra
consultas de metadados e enumerações de diretórios no percurso sem watcher.
O custo da primeira busca e a regressão quente com watcher continuam exigindo
trabalho. Perfil em `/tmp/dartforge-symbol-bases-profile.txt`; seus tempos não
foram usados nas tabelas.

### Continuação: outlines para construir o índice de símbolos (05/11)

O coletor de `workspace/symbol` lê declarações, assinaturas e intervalos de
código, mas não percorre as instruções dos corpos. O índice passou a usar
`parse_lexed_outline`, API já existente do frontend. Os blocos de funções,
métodos, construtores e acessores viram blocos vazios com os mesmos spans.
Parâmetros, listas de inicialização, inicializadores de campos e variáveis
(incluindo funções literais) e corpos `=>` continuam sendo lidos. Quando as
chaves do bloco não fecham, o parser conserva sua leitura normal e recuperação.
A análise de diagnósticos e os demais consumidores do parser não foram alterados.

Uma regressão compara os metadados completos dos dois modos, não apenas uma
consulta limitada a 500 itens. Inclui a união de nomes, declarações em ordem,
assinaturas, tipos locais, URIs, intervalos, imports, exports, parts e estados
dos arquivos. As sete fontes fixas exercitam inicializadores com closures,
registros, getters, setters, construtores, enums, mixins, extensões, documentação
Unicode e corpos com erros. O teste também aceita manifestos em
`DARTFORGE_SYMBOL_CORPUS_MANIFESTS`, separados pelo delimitador de caminhos da
plataforma. Sem essa variável, as fontes fixas sempre rodam, sem novo requisito
de SDK/corpus para o CI.

Na expansão local, **17.645 arquivos Dart únicos** dos três manifestos produziram
metadados idênticos nos dois modos. É uma comparação com o coletor anterior;
não conta como 17.645 testes nem substitui o servidor oficial. Diagnósticos do
parser não fazem parte dessa comparação, pois o índice não os publica.
O comando usado foi:

```sh
DARTFORGE_SYMBOL_CORPUS_MANIFESTS=/tmp/dartforge-symbol-bases-limitless-inputs-after.json:/tmp/dartforge-symbol-bases-core-inputs-after.json:/tmp/dartforge-symbol-bases-frontend-inputs-after.json \
  cargo test --locked --release -p dartforge-lsp \
  symbol_outlines_preserve_full_parser_metadata --lib -- --nocapture
```

O log é `/tmp/dartforge-symbol-outline-corpus.log`; a comparação terminou em
7,18 s, fora das medições de desempenho. A suíte normal passou com **334 testes
do LSP, zero falhas e zero ignorados**, além dos **11 testes Python**, formatação,
clippy do workspace sem avisos e documentação estrita. Os controles mantiveram
**270/270**, e os parts deram **27/27 com e 27/27 sem watcher**. Logs em
`/tmp/dartforge-symbol-outline-{tests,python,clippy,doc,controls}.log` e respostas
em `/tmp/dartforge-symbol-outline-control-*.jsonl` e
`/tmp/dartforge-symbol-outline-{parts,parts-unwatched}.jsonl`.

O baseline é `c4badf38`, preservado em `/tmp/dartforge-symbol-outline-before`
(SHA-256 iniciado em `1600d9ebf850`); o novo binário é
`/tmp/dartforge-symbol-outline-after`, SHA-256
`34593ff88c41bd3e55487180bcd0aab1d12e2e84bba9d951fbc96fa84918c75e`.
As 54 sessões revezaram os dois binários `release` e o Dart 3.6.2 nos três
projetos, em três ordens, separando clientes com e sem watcher. A preparação
permaneceu com resolução do documento e dez segundos adicionais de ociosidade.
Compilações, testes e verificações de corpus terminaram antes das medições.

**Primeira busca sem watcher**, medianas de três sessões, em ms de
latência / CPU da árvore:

| Projeto | Antes | Depois | Dart 3.6.2 |
| --- | ---: | ---: | ---: |
| `limitless_ui` | 1.310,4 / 1.309,4 | 1.142,5 / 1.141,6 | 712,2 / 958,2 |
| core | 960,2 / 884,4 | 755,1 / 754,3 | 513,7 / 763,3 |
| frontend | 1.592,2 / 1.591,2 | 1.413,4 / 1.412,5 | 562,4 / 733,0 |

A CPU da construção caiu cerca de **13%, 15% e 11%**, respectivamente. A
latência caiu cerca de 13%, 21% e 11%; no core, a diferença entre CPU e tempo
total do baseline inclui tempo fora da CPU da árvore. Os logs não isolam I/O,
escalonamento e transporte do cliente. O ganho não deve ser atribuído
integralmente ao parser usando apenas o tempo total.

**Primeira busca com watcher**, nas mesmas unidades:

| Projeto | Antes | Depois | Dart 3.6.2 |
| --- | ---: | ---: | ---: |
| `limitless_ui` | 1.290,0 / 1.289,0 | 1.145,9 / 1.145,1 | 711,2 / 957,6 |
| core | 852,0 / 851,2 | 755,3 / 754,4 | 513,0 / 760,1 |
| frontend | 1.590,2 / 1.589,3 | 1.405,5 / 1.404,6 | 557,7 / 726,8 |

A CPU caiu aproximadamente **11%, 11% e 12%**. A primeira busca continua
mais lenta que o Dart nos dois protocolos. Um ensaio preliminar do frontend
mediu 2.043,9 ms totais e 1.511,5 ms de CPU, guardado em
`/tmp/dartforge-symbol-outline-quick.jsonl`; ele não faz parte das medianas
das matrizes revezadas. Nenhuma sessão dessas matrizes foi descartada.

**Buscas quentes**, medianas de 48 pedidos por célula, latência / CPU da
árvore em ms:

| Cliente | Projeto | Antes | Depois | Dart 3.6.2 |
| --- | --- | ---: | ---: | ---: |
| Sem watcher | `limitless_ui` | 176,3 / 175,7 | 175,7 / 175,2 | 17,2 / 19,7 |
| Sem watcher | core | 71,6 / 71,1 | 71,7 / 71,1 | 12,4 / 12,8 |
| Sem watcher | frontend | 236,5 / 235,6 | 235,4 / 234,8 | 18,1 / 25,4 |
| Com watcher | `limitless_ui` | 12,0 / 11,5 | 11,6 / 11,1 | 16,3 / 17,8 |
| Com watcher | core | 9,0 / 8,5 | 9,3 / 8,9 | 12,8 / 13,2 |
| Com watcher | frontend | 12,6 / 12,1 | 12,6 / 12,2 | 17,6 / 22,5 |

O índice aquecido já reutiliza os metadados; essa mudança não dá ganho geral
nessas buscas. O core com watcher subiu 0,3 ms de latência e 0,4 ms de CPU:
a regressão quente da etapa anterior não foi resolvida. As buscas quentes
com watcher continuam menores que o Dart; sem watcher, ainda perdem.

**Footprint final da árvore**, medianas em MiB:

| Cliente | Projeto | Antes | Depois | Dart 3.6.2 |
| --- | --- | ---: | ---: | ---: |
| Sem watcher | `limitless_ui` | 135,5 | 134,5 | 671,9 |
| Sem watcher | core | 123,6 | 122,6 | 505,0 |
| Sem watcher | frontend | 585,1 | 580,9 | 712,6 |
| Com watcher | `limitless_ui` | 135,2 | 133,9 | 669,4 |
| Com watcher | core | 123,6 | 122,6 | 505,2 |
| Com watcher | frontend | 584,0 | 579,4 | 710,5 |

A queda final foi de aproximadamente **1–4,6 MiB**. São amostras no fim de
sessões curtas, não um pico simultâneo da árvore nem prova do platô durante
ondas de gerados.

Cada matriz manteve **216/216 respostas completas iguais em cada comparação**
antes/depois, antes/Dart e depois/Dart. Os manifestos anteriores e posteriores
foram idênticos, com os mesmos 9.761, 6.640 e 10.894 arquivos e sem raízes
ausentes. Os 17.645 caminhos Dart únicos também foram relidos como UTF-8 sem
falha, conferindo que a comparação de corpus não passou por entradas primárias
ilegíveis. Resultados em
`/tmp/dartforge-symbol-outline-{matrix,watched-matrix}-{limitless,core,frontend}-{before,after,dart}-{0,1,2}.jsonl`,
resumos `/tmp/dartforge-symbol-outline-{summary,watched-summary}.log` e
manifestos `/tmp/dartforge-symbol-outline-{limitless,core,frontend}-inputs-{before,after}.json`.
O protocolo legado, seus resultados anteriores reprovados e as edições nos
projetos grandes continuam exigindo validação própria; a dupla fria/quente da
etapa anterior não foi repetida com esse novo binário.

### Continuação: abertura de projeto externo e experimento de aliases retirado (05/11)

O novo `workspace_symbol_aliases.py` compara os JSONs completos e ordenados com
o Dart 3.6.2 em nove fases: carga, repetição, edição de corpo, abertura de alias
editado sem salvar, fechamento desse alias, duas substituições do catálogo
(tipos diferentes e iguais), remoção da dependência e fechamento final. Cada
fase faz três consultas, incluindo uma busca ampla; URIs, intervalos e itens
duplicados continuam participando da comparação.

Esse oráculo revelou uma divergência anterior ao experimento: abrir um arquivo
de outro projeto reconstruía a ordem de todos os arquivos adicionados. Quando
a nova raiz vinha antes da existente no `ContextLocator`, seus símbolos passavam
à frente dos símbolos já conhecidos. Na abertura e no fechamento de documentos,
a sessão agora conserva os arquivos sobreviventes na ordem anterior e acrescenta
os novos. Mudanças explícitas de configuração ou das pastas do workspace continuam
reconstruindo a coleção. Uma regressão de integração falhou antes dessa correção
e passou depois. Essa correção e o oráculo são a alteração mantida no código.

Nos mesmos caminhos, o baseline `bf8545d2` deu **26/27 com watcher e 26/27 sem
watcher**; o binário final, sem cache de aliases, deu **27/27 nos dois modos**.
As 27 respostas do próprio Dart permaneceram idênticas entre antes/depois em
cada modo. A diferença era a ordem da consulta ampla na abertura do alias
externo; a declaração duplicada, uma com URI de arquivo e outra de package, é
preservada como no Dart. Respostas anteriores em
`/tmp/dartforge-symbol-alias-before{,-unwatched}.jsonl` e finais em
`/tmp/dartforge-symbol-external-final{,-unwatched}.jsonl`.
Binário final em `/tmp/dartforge-symbol-external-final`, SHA-256
`05a27776f12e36db9cd50146ec781f40bff339038e7c3623eb6a217d07a3e47b`.

Passaram **335 testes do LSP, zero falhas e zero ignorados**, os **11 testes
Python**, clippy do workspace sem avisos, documentação estrita e formatação.
Os controles anteriores mantiveram **270/270 respostas completas iguais**.
Logs finais em `/tmp/dartforge-symbol-external-final-{tests,python,clippy,doc}.log`,
controles em `/tmp/dartforge-symbol-external-control-*.jsonl`. O workspace inteiro
e a suíte nativa não foram repetidos nesta etapa, que altera somente o LSP.

**Experimento retirado:** foi implementado um cache da tabela global de aliases,
com as tabelas de tipos em ordem como chave. Edições só de corpo podiam reutilizar
a tabela local quando seu conteúdo continuava igual. A chave conservava apenas
as tabelas de tipos, sem bibliotecas antigas nem histórico; referências fracas
verificaram a liberação de versões removidas, inclusive na troca por tipos iguais.
O experimento passou 337 testes do LSP e 270 controles antes das medições, mas
foi retirado por seu custo de memória e pela falta de ganho geral sem watcher.
As duas regressões específicas do cache foram retiradas junto com ele.

O baseline experimental é `bf8545d2`, preservado em
`/tmp/dartforge-symbol-alias-before` (SHA-256 iniciado em `34593ff88c41`);
`/tmp/dartforge-symbol-alias-final` contém o experimento retirado, SHA-256
`4697821beb58f475762212c0fb0fb2fba2685674594e5aafac558dbdd17f309e`.
**Esse binário não representa a alteração final.** As 54 sessões revezaram os
dois binários e o Dart 3.6.2 nos três projetos em três ordens, separando clientes
com e sem watcher. A preparação manteve o documento e dez segundos adicionais
de ociosidade. Compilações e testes terminaram antes das medições.

**Buscas quentes do experimento**, medianas de 48 pedidos por célula,
latência / CPU da árvore em ms:

| Cliente | Projeto | Antes | Experimento retirado | Dart 3.6.2 |
| --- | --- | ---: | ---: | ---: |
| Com watcher | `limitless_ui` | 13,1 / 12,8 | 11,8 / 11,3 | 15,5 / 19,8 |
| Com watcher | core | 9,0 / 8,5 | 8,8 / 8,3 | 12,7 / 13,0 |
| Com watcher | frontend | 12,7 / 12,3 | 12,2 / 11,7 | 17,7 / 25,1 |
| Sem watcher | `limitless_ui` | 177,3 / 176,9 | 180,3 / 179,7 | 15,5 / 17,9 |
| Sem watcher | core | 71,9 / 71,5 | 72,0 / 71,5 | 12,9 / 13,1 |
| Sem watcher | frontend | 236,7 / 236,0 | 236,7 / 236,2 | 17,7 / 22,7 |

O ganho agregado com watcher do `limitless_ui` varia entre sessões: medianas
quentes do baseline 11,40 / 13,60 / 11,56 ms; experimento 11,35 / 11,33 / 12,11 ms.
Não demonstra uma redução estável de 10% nesse projeto. No core, as três medianas
novas ficaram abaixo das três anteriores, com uma diferença pequena; no frontend,
caíram de 12,74 / 12,46 / 12,64 para 12,13 / 12,23 / 12,11 ms. Sem watcher, o
`limitless_ui` piorou em duas das três sessões: baseline 177,15 / 178,26 / 177,08 ms,
experimento 177,08 / 181,90 / 180,86 ms. Essa regressão não foi transformada em ganho.

A primeira busca ficou próxima do baseline e continuou mais lenta que o Dart.
Com watcher, antes → experimento: **1.167,8 → 1.163,5 ms**, **774,6 → 767,1 ms** e
**1.420,3 → 1.412,1 ms**, contra **705,7 / 512,1 / 575,7 ms** do Dart. Sem watcher:
**1.154,8 → 1.164,9 ms**, **765,5 → 757,9 ms** e **1.428,6 → 1.422,7 ms**, contra
**698,9 / 515,1 / 560,6 ms**. As CPUs completas estão nos resumos dos artefatos.

**Footprint final do experimento**, medianas em MiB:

| Cliente | Projeto | Antes | Experimento retirado | Dart 3.6.2 |
| --- | --- | ---: | ---: | ---: |
| Com watcher | `limitless_ui` | 134,3 | 134,4 | 673,6 |
| Com watcher | core | 122,3 | 122,8 | 505,0 |
| Com watcher | frontend | 580,1 | 583,7 | 716,5 |
| Sem watcher | `limitless_ui` | 134,5 | 134,8 | 680,3 |
| Sem watcher | core | 122,6 | 123,4 | 505,0 |
| Sem watcher | frontend | 581,4 | 585,2 | 704,2 |

São amostras finais de sessões curtas, sem pico simultâneo ou prova de platô
com ondas de gerados. O aumento de até 3,8 MiB, junto à regressão sem watcher,
não justificou manter o cache por esse ganho quente pequeno. A alteração final
preserva o coletor de símbolos de `bf8545d2`; não reivindica esses ganhos.

Cada matriz experimental manteve **216/216 respostas completas iguais em cada
comparação** antes/depois, antes/Dart e depois/Dart. Manifestos idênticos antes e
depois, com 9.761, 6.640 e 10.894 arquivos e nenhuma raiz ausente. Resultados em
`/tmp/dartforge-symbol-alias-{matrix,watched-matrix}-{limitless,core,frontend}-{before,after,dart}-{0,1,2}.jsonl`,
resumos `/tmp/dartforge-symbol-alias-{summary,watched-summary}.log` e manifestos
`/tmp/dartforge-symbol-alias-{limitless,core,frontend}-inputs-{before,after}.json`.
O protocolo legado, suas reprovações anteriores, a dupla fria/quente e as edições
nos projetos grandes continuam exigindo validação própria.

### Continuação: compartilhar a travessia de diretivas dentro do pedido (05/11)

Um perfil novo das consultas sem watcher mostrou trabalho repetido entre a
descoberta anterior à resposta e a resolução da lista de bibliotecas conhecida.
Cada chamada de `discover_graph` criava seu próprio conjunto de unidades
visitadas. Mesmo com os metadados sintáticos reutilizados, as duas travessias
materializavam de novo as diretivas das unidades em comum: resolviam URIs,
verificavam alvos gerados e tentavam inserir FileStates já criados.

No SDK 3.6.2, `FileKind` conserva as listas `_libraryImports` e
`_libraryExports` depois de materializá-las
(`pkg/analyzer/lib/src/dart/analysis/file_state.dart`). Aqui o compartilhamento
fica limitado ao pedido, preservando a revalidação do próximo pedido quando o
cliente não fornece eventos do disco.

O conjunto de unidades processadas agora pertence ao ambiente de um pedido.
As duas travessias o compartilham, com chave **contexto + caminho normalizado**.
Um arquivo compartilhado por dois drivers continua sendo processado nos dois
contextos. O conjunto sai ao terminar o pedido; não é um cache residente e
não permite confiar no estado do disco entre pedidos. A resolução de cada
diretiva ainda passa pela seleção de fonte/gerado, sem guardar alvos por URI.

A lista de resposta continua sendo capturada antes da resolução das bibliotecas
conhecidas: dependências descobertas nessa etapa entram na próxima resposta.
A regressão de condicionais foi ampliada para editar uma ramificação não
selecionada depois de consultas estáveis. Ela verifica a declaração editada na
resposta atual e a nova dependência somente na seguinte, incluindo a passagem
de uma consulta sem confiança no disco para outra com confiança. Isso também
confere que o conjunto de trabalho não sobrevive ao pedido anterior.

Passaram **335 testes do LSP, zero falhas e zero ignorados**, **11 testes
Python**, clippy do workspace sem avisos, documentação estrita e formatação.
Os controles mantiveram **378/378 respostas completas iguais ao Dart 3.6.2**:
270 anteriores, 27 com e 27 sem watcher nos aliases e outros 27 + 27 nos parts.
O workspace inteiro e a suíte nativa não foram repetidos nesta etapa do LSP.
Logs em `/tmp/dartforge-symbol-traversal-{tests,python,clippy,doc,controls}.log`;
respostas em `/tmp/dartforge-symbol-traversal-control-*.jsonl`,
`/tmp/dartforge-symbol-traversal-aliases{,-unwatched}.jsonl` e
`/tmp/dartforge-symbol-traversal-parts{,-unwatched}.jsonl`.

O baseline é `d3d28dc7`, preservado em `/tmp/dartforge-symbol-traversal-before`,
SHA-256 iniciado em `05a27776f12e`; o novo binário está em
`/tmp/dartforge-symbol-traversal-after`, SHA-256
`d939fe10a0b480fcb0fc085857228dda946e2fc2e0d19f8cd84b644f03012c89`.
O ensaio preliminar do frontend sem watcher manteve **24/24 respostas completas
iguais antes/depois**: mediana quente **241,1 → 210,5 ms**, CPU **240,5 → 210,0 ms**,
footprint final **585,8 → 580,9 MiB**. Foi uma sessão por versão, sem Dart pareado;
esses números não entram nas matrizes revezadas. Artefatos em
`/tmp/dartforge-symbol-traversal-quick-{before,after}.jsonl`.

O perfil inicial está em `/tmp/dartforge-symbol-path-profile.txt`. O segundo,
`/tmp/dartforge-symbol-hot-profile.txt`, fez três buscas de preparação antes de
amostrar oito segundos das buscas quentes. Seus tempos são diagnósticos e não
entram nas comparações de desempenho.

As **54 sessões revezadas** mantiveram os projetos, SDK 3.6.2, caches,
documento de preparação e dez segundos adicionais de ociosidade da etapa
anterior. Compilações e testes terminaram antes das medições. Cada matriz
completa, com ou sem watcher, manteve **216/216 respostas completas iguais em
cada comparação** antes/depois, antes/Dart e depois/Dart. Os manifestos foram
idênticos antes/depois: 9.761 arquivos no `limitless_ui`, 6.640 no core e
10.894 no frontend, sem raízes ausentes.

**Buscas quentes**, medianas de 48 pedidos por célula, latência / CPU da
árvore em ms:

| Cliente | Projeto | Antes | Depois | Dart 3.6.2 |
| --- | --- | ---: | ---: | ---: |
| Sem watcher | `limitless_ui` | 184,5 / 183,8 | 166,5 / 166,1 | 17,0 / 21,1 |
| Sem watcher | core | 72,7 / 72,1 | 65,1 / 64,6 | 12,8 / 12,9 |
| Sem watcher | frontend | 241,6 / 241,2 | 209,0 / 208,6 | 18,6 / 26,3 |
| Com watcher | `limitless_ui` | 11,7 / 11,2 | 11,6 / 11,2 | 16,1 / 17,9 |
| Com watcher | core | 9,4 / 8,9 | 8,8 / 8,4 | 12,6 / 13,2 |
| Com watcher | frontend | 12,7 / 12,2 | 12,8 / 12,3 | 18,1 / 22,6 |

Sem watcher, a latência caiu cerca de **10%, 10% e 14%** e a CPU cerca de
**10%, 10% e 14%**. Ainda perde do Dart nesse modo. Com watcher, o snapshot
estável já evita o grafo: não houve ganho geral. O core caiu nesta série,
mas o frontend subiu 0,1 ms de latência e CPU; os três continuam abaixo do
Dart. Esses resultados não demonstram que todo ganho quente com watcher
seja causado pela eliminação da travessia duplicada.

**Primeira busca**, medianas de três sessões, nas mesmas unidades:

| Cliente | Projeto | Antes | Depois | Dart 3.6.2 |
| --- | --- | ---: | ---: | ---: |
| Sem watcher | `limitless_ui` | 1.171,4 / 1.170,5 | 1.165,3 / 1.164,5 | 719,0 / 976,7 |
| Sem watcher | core | 766,1 / 765,2 | 764,5 / 763,6 | 519,8 / 773,2 |
| Sem watcher | frontend | 1.453,3 / 1.452,3 | 1.426,1 / 1.425,2 | 571,4 / 738,2 |
| Com watcher | `limitless_ui` | 1.155,1 / 1.154,3 | 1.149,5 / 1.148,6 | 717,0 / 966,9 |
| Com watcher | core | 773,9 / 773,1 | 760,1 / 759,2 | 514,9 / 761,8 |
| Com watcher | frontend | 1.429,6 / 1.428,6 | 1.403,3 / 1.393,8 | 560,7 / 730,6 |

A primeira busca melhorou pouco, cerca de 0–2% de latência, e continua
mais lenta que o Dart. A construção inicial do índice não é eliminada pelo
compartilhamento do trabalho entre as duas fases.

**Footprint final**, medianas em MiB:

| Cliente | Projeto | Antes | Depois | Dart 3.6.2 |
| --- | --- | ---: | ---: | ---: |
| Sem watcher | `limitless_ui` | 135,2 | 134,3 | 671,1 |
| Sem watcher | core | 123,4 | 123,0 | 511,2 |
| Sem watcher | frontend | 580,6 | 581,0 | 710,8 |
| Com watcher | `limitless_ui` | 134,5 | 133,9 | 678,5 |
| Com watcher | core | 124,1 | 122,5 | 499,1 |
| Com watcher | frontend | 580,1 | 580,9 | 710,4 |

Não houve redução geral de memória: o frontend subiu 0,4 / 0,8 MiB; os
outros dois projetos caíram até 1,6 MiB. São amostras finais de sessões
curtas, sem pico simultâneo da árvore ou prova de platô com ondas de gerados.
Resultados em
`/tmp/dartforge-symbol-traversal-{matrix,watched-matrix}-{limitless,core,frontend}-{before,after,dart}-{0,1,2}.jsonl`,
resumos `/tmp/dartforge-symbol-traversal-{summary,watched-summary}.log` e manifestos
`/tmp/dartforge-symbol-traversal-{limitless,core,frontend}-inputs-{before,after}.json`.

Após as matrizes, uma nova passada do protocolo **legado com caches aquecidos**
deu **24/24 no baseline e 24/24 no novo binário** nos projetos grandes, com
respostas do próprio Dart idênticas antes/depois. Essa passada não apaga os
21/24 aquecidos ou os 9/24 frios anteriores e não demonstra estabilidade com
caches inicialmente vazios. Resultados em
`/tmp/dartforge-symbol-traversal-legacy-{limitless,core,frontend}-{before,after}.jsonl`.
A dupla fria/quente com caches isolados e as edições nos projetos grandes
continuam exigindo validação própria.
O perfil posterior, feito depois das medições com três buscas de preparação,
está em `/tmp/dartforge-symbol-traversal-profile.txt`. A revalidação no disco
e a normalização/comparação de caminhos continuam sendo custos relevantes;
seus tempos também foram excluídos das tabelas.

### Continuação: reutilizar as raízes normalizadas do catálogo (05/11)

O perfil posterior ainda mostrou `RelativeBase::new` verificando, a cada
comparação de prefixo, se as raízes do catálogo já eram chaves lexicais. Mesmo
sem alocar um caminho novo, essa verificação percorria seus bytes e validava
UTF-8 repetidamente. As raízes dos pacotes já eram normalizadas ao carregar o
contexto. O carregamento agora também normaliza as raízes dos mapeamentos de
gerados e registra essa garantia no contexto.

A criação das bases relativas e a identificação de package URIs passam a
reutilizar diretamente essas raízes carregadas. A comparação de prefixos e a
ordem do catálogo continuam iguais. Catálogos avulsos, incluindo o resolvedor
geral, conservam a normalização: a marca só é definida pelo carregamento depois
de normalizar os mapeamentos. Não há tabela adicional por arquivo nem cache de
alvos de URI; a existência da fonte e dos gerados continua sendo consultada.

Uma regressão nova compara o catálogo carregado com raízes avulsas contendo
`..`, partindo tanto da unidade fonte quanto de sua contraparte gerada. Inclui
imports relativos, travessia até a raiz do package, escapes, fragmentos e URI de
arquivo. Ela também expôs uma lacuna no caminho geral: depois de mapear um
gerado para uma raiz fonte não normalizada, a unidade fonte precisava ser
normalizada antes de procurar seu package. Essa passagem foi corrigida.
A regressão que cria e remove fonte/gerado foi ampliada para os dois caminhos,
mantendo os 60 vetores de URI da VM 3.6.2 e o isolamento das bases por contexto.

Passaram **336 testes do LSP, zero falhas e zero ignorados**, **11 testes Python**,
clippy do workspace sem avisos, documentação estrita e formatação. Os controles
mantiveram **378/378 respostas completas iguais ao Dart**: 270 anteriores,
54 de aliases e 54 de parts, com e sem watcher. O workspace inteiro e a suíte
nativa não foram repetidos nesta etapa do LSP. Logs em
`/tmp/dartforge-symbol-roots-{unit,tests,python,clippy,doc,controls}.log`;
respostas em `/tmp/dartforge-symbol-roots-control-*.jsonl`,
`/tmp/dartforge-symbol-roots-aliases{,-unwatched}.jsonl` e
`/tmp/dartforge-symbol-roots-parts{,-unwatched}.jsonl`.

O baseline é `0a2daac6`, preservado em `/tmp/dartforge-symbol-roots-before`,
SHA-256 iniciado em `d939fe10a0b4`; o novo binário está em
`/tmp/dartforge-symbol-roots-after`, SHA-256
`05ab494ee0b1db810fb175f8b07eb44a4bd1858f537204fefd24cc09cd18f559`.
O ensaio preliminar do frontend sem watcher manteve **24/24 respostas completas
iguais antes/depois**: primeira busca **1.413,8 → 1.362,6 ms**, mediana quente
**206,3 → 186,3 ms**, CPU quente **205,7 → 185,8 ms**, footprint final **580,4 →
583,8 MiB**. Foi uma sessão por versão, sem Dart pareado; seus tempos ficam fora
das matrizes revezadas. Artefatos em
`/tmp/dartforge-symbol-roots-quick-{before,after}.jsonl`.

As **54 sessões revezadas** repetiram os três projetos, SDK 3.6.2, caches,
documento de preparação e dez segundos adicionais de ociosidade. Compilações,
testes e demais controles terminaram antes das medições. Cada matriz, com ou
sem watcher, manteve **216/216 respostas completas iguais em cada comparação**
antes/depois, antes/Dart e depois/Dart. Manifestos idênticos antes/depois, com
9.761, 6.640 e 10.894 arquivos e nenhuma raiz ausente.

**Buscas quentes**, medianas de 48 pedidos por célula, latência / CPU da árvore
em ms:

| Cliente | Projeto | Antes | Depois | Dart 3.6.2 |
| --- | --- | ---: | ---: | ---: |
| Sem watcher | `limitless_ui` | 164,1 / 163,5 | 153,0 / 152,6 | 16,8 / 21,8 |
| Sem watcher | core | 64,6 / 64,3 | 61,6 / 61,1 | 12,8 / 13,1 |
| Sem watcher | frontend | 206,4 / 206,0 | 189,7 / 189,0 | 19,5 / 26,6 |
| Com watcher | `limitless_ui` | 11,8 / 11,4 | 11,9 / 11,5 | 16,2 / 18,0 |
| Com watcher | core | 9,2 / 8,8 | 9,1 / 8,6 | 12,8 / 13,2 |
| Com watcher | frontend | 12,8 / 12,3 | 12,8 / 12,3 | 19,4 / 23,1 |

Sem watcher, a latência caiu cerca de **7%, 5% e 8%**, e a CPU cerca de
**7%, 5% e 8%**. Ainda perde do Dart. Com watcher, as buscas quentes ficaram
próximas do baseline: o `limitless_ui` subiu 0,1 ms de latência e CPU, sem
ganho geral nesse modo. Os três continuam menores que o Dart nesse protocolo.

**Primeira busca**, medianas de três sessões, nas mesmas unidades:

| Cliente | Projeto | Antes | Depois | Dart 3.6.2 |
| --- | --- | ---: | ---: | ---: |
| Sem watcher | `limitless_ui` | 1.163,9 / 1.163,1 | 1.130,1 / 1.129,3 | 710,6 / 956,0 |
| Sem watcher | core | 758,0 / 757,2 | 750,4 / 749,6 | 522,0 / 773,1 |
| Sem watcher | frontend | 1.392,5 / 1.391,6 | 1.363,6 / 1.362,7 | 568,5 / 738,5 |
| Com watcher | `limitless_ui` | 1.146,4 / 1.137,5 | 1.110,3 / 1.109,4 | 717,3 / 973,9 |
| Com watcher | core | 750,9 / 749,9 | 741,1 / 740,3 | 536,2 / 758,7 |
| Com watcher | frontend | 1.393,4 / 1.392,5 | 1.383,7 / 1.382,8 | 604,7 / 732,1 |

A primeira busca melhorou cerca de **1–3%** de latência nos dois modos,
mas continua mais lenta que a do Dart. Não é o ganho de aproximadamente 4%
do ensaio preliminar do frontend em uma única sessão.

**Footprint final**, medianas em MiB:

| Cliente | Projeto | Antes | Depois | Dart 3.6.2 |
| --- | --- | ---: | ---: | ---: |
| Sem watcher | `limitless_ui` | 134,5 | 134,3 | 673,4 |
| Sem watcher | core | 123,2 | 123,1 | 505,9 |
| Sem watcher | frontend | 581,4 | 583,4 | 705,3 |
| Com watcher | `limitless_ui` | 133,7 | 133,9 | 676,5 |
| Com watcher | core | 122,7 | 123,3 | 504,3 |
| Com watcher | frontend | 581,9 | 582,9 | 720,5 |

O frontend subiu **2,0 / 1,0 MiB**. A marca de raízes normalizadas não introduz
cache por arquivo, mas isso não demonstra redução de memória do servidor:
as amostras finais aumentaram em quatro das seis células. Não são pico
simultâneo da árvore nem prova do platô com ondas de gerados. Resultados em
`/tmp/dartforge-symbol-roots-{matrix,watched-matrix}-{limitless,core,frontend}-{before,after,dart}-{0,1,2}.jsonl`,
resumos `/tmp/dartforge-symbol-roots-{summary,watched-summary}.log` e manifestos
`/tmp/dartforge-symbol-roots-{limitless,core,frontend}-inputs-{before,after}.json`.

Depois das medições, a dupla com **caches Dart isolados e inicialmente vazios**
foi repetida no novo binário: **72/72 respostas completas iguais no frio e
72/72 no quente**. Os arrays de cada servidor também foram idênticos entre
os dois estados. Foram 12 sessões adicionais, sem watcher, com o mesmo documento
de preparação e os dez segundos extras de ociosidade. Seus tempos não entram
nas tabelas acima; os estados se referem ao cache Dart em disco, não ao cache
do sistema operacional.

Cada projeto começou com um diretório de cache novo e vazio; o harness verificou
que ele continuava vazio depois da primeira sessão DartForge e antes de iniciar
o Dart. Após o Dart, os diretórios tinham 13.609 / 10.547 / 16.911 arquivos;
após a sessão quente, 13.610 / 10.548 / 16.912. Caches preservados em
`target/scratch-symbol-roots-caches/`, inventário em
`/tmp/dartforge-symbol-roots-cache-manifest.json`, respostas em
`/tmp/dartforge-symbol-roots-cache-{limitless,core,frontend}-{cold,warm}-{after,dart}.jsonl`
e log em `/tmp/dartforge-symbol-roots-caches.log`. Os manifestos de entrada
também permaneceram iguais após essas sessões, em
`/tmp/dartforge-symbol-roots-{limitless,core,frontend}-inputs-after-caches.json`.

Essa dupla usa preparação explícita e não transforma as reprovações anteriores
do protocolo legado em aprovações. As edições nos projetos grandes, a primeira
busca e a sessão longa continuam exigindo trabalho.

### Experimento retirado: antecipar a extração dos metadados em paralelo (05/11)

**Não integrado ao código principal.** As matrizes mantiveram as respostas,
mas aumentaram a CPU inicial e o footprint final. O protótipo foi retirado,
incluindo sua regressão específica e a expansão de comparação com metadados
antecipados; o oráculo de parts com arquivos adicionais foi mantido.

Na primeira busca, o índice lê declarações de milhares de bibliotecas de forma
sequencial. Os diagnósticos iniciais são produzidos em outro processo e cobrem
as bibliotecas alcançadas pelos arquivos analisados; não fornecem ao despachante
os metadados do restante do catálogo. O experimento separou a extração dos
metadados da descoberta: preparou bibliotecas independentes antes das duas
travessias, preservando a escolha de donos de URI e a ordem da resposta no
fluxo sequencial.

O lote usa no máximo **quatro trabalhadores, incluindo o despachante**, apenas
com pelo menos 256 arquivos ainda não indexados e somente na construção inicial
do índice. Com um único processador disponível, o caminho continua sequencial;
se uma thread auxiliar não puder ser criada, os trabalhadores restantes,
incluindo o despachante, terminam o lote. As threads têm a mesma reserva de
pilha do parser no despachante, para os inicializadores gerados profundos, e
são encerradas antes de retornar do lote. Não há fila ou pool persistente.

Os trabalhadores leem a mesma fotografia dos documentos abertos, usam o coletor
de outlines já existente e entregam somente os metadados e as impressões
digitais. A descoberta continua resolvendo imports e exports em sua ordem
original. Ao terminar a primeira resposta, os metadados antecipados que nenhuma
das travessias ou documentos prioritários usou são descartados. As buscas
seguintes continuam pelo caminho normal de validação e cache.

Uma regressão compara os metadados integrais antecipados com a indexação
sequencial de 320 unidades, incluindo aliases, parts compartilhadas, texto
malformado, Unicode, URI do cliente, arquivo ausente e caminho repetido. A
expansão opcional do corpus comparou **17.645 arquivos Dart únicos** entre a
leitura completa, outlines e metadados antecipados: todos iguais. Continua
sendo uma expansão de um teste, não 17.645 testes nem prova isolada de paridade
com o servidor oficial. Log em `/tmp/dartforge-symbol-prefetch-corpus.log`.

O oráculo de parts ganhou `--padding-files N`: com 320 arquivos adicionais,
inclui uma consulta dos 320 símbolos para conferir sua ordem integral durante
abertura, criação, edição, remoção, recriação e fechamento do part. Passaram
**36/36 com e 36/36 sem watcher**, com respostas do Dart idênticas no baseline
e no experimento nos mesmos caminhos. O binário final foi repetido nos dois
modos. O oráculo conserva suas 27 consultas padrão quando a opção não é usada.
Artefatos em `/tmp/dartforge-symbol-prefetch-final-padded-parts{,-unwatched}.jsonl`;
baseline em `/tmp/dartforge-symbol-prefetch-parts-before.jsonl` e
`/tmp/dartforge-symbol-prefetch-padded-parts-before-unwatched.jsonl`.

Antes das medições, passaram **337 testes do LSP**, sem falhas ou ignorados,
os **11 testes Python**, clippy do workspace sem avisos, documentação estrita e
formatação. Os controles mantiveram **450/450 respostas completas iguais ao
Dart**: 378 anteriores e as 72 dos parts com arquivos adicionais. O workspace
inteiro e a suíte nativa não foram repetidos nesta etapa. Logs em
`/tmp/dartforge-symbol-prefetch-{tests,clippy,doc,python,controls}.log` e respostas
em `/tmp/dartforge-symbol-prefetch-control-*.jsonl`,
`/tmp/dartforge-symbol-prefetch-aliases{,-unwatched}.jsonl` e
`/tmp/dartforge-symbol-prefetch-parts{,-unwatched}.jsonl`.

O baseline é `d79cce98`, em `/tmp/dartforge-symbol-prefetch-before`, SHA-256
iniciado em `05ab494ee0b1`; o experimento está em
`/tmp/dartforge-symbol-prefetch-final` (também copiado para `-after`), SHA-256
`25ff43d8fba9fe049fe757bd5ad53235e2a3b7869aa912e9eed9c584f6c61e5e`.
O ensaio inicial do frontend, antes de limitar a retenção ao trabalho usado,
mediu primeira resposta **2.012,9 → 996,8 ms**, CPU **1.451,4 → 1.467,9 ms**,
mediana quente **181,1 → 191,2 ms** e footprint final **570,3 → 587,5 MiB**.
Com o descarte, uma nova dupla mediu **1.626,3 → 974,1 ms**, CPU **1.404,2 →
1.446,1 ms**, mediana quente **181,4 → 183,6 ms** e footprint **581,4 → 586,8 MiB**.
O tempo fora da CPU do baseline varia; a diferença de latência não deve ser
atribuída integralmente ao paralelismo. As duas duplas mantiveram **24/24**
respostas iguais antes/depois; são diagnósticas e não entram nas medianas das
matrizes revezadas. Artefatos em `/tmp/dartforge-symbol-prefetch-{quick,pruned}-{before,after}.jsonl`.

As **54 sessões revezadas** mantiveram **216/216 respostas completas iguais em
cada comparação** antes/depois, antes/Dart e depois/Dart, tanto com quanto
sem watcher. Manifestos idênticos, com 9.761 / 6.640 / 10.894 arquivos e nenhuma
raiz ausente. Compilações, testes e comparação do corpus terminaram antes das
medições; preparação com documento resolvido e dez segundos extras de ociosidade.

**Primeira busca do experimento**, medianas de três sessões, latência / CPU da
árvore em ms:

| Cliente | Projeto | Antes | Experimento retirado | Dart 3.6.2 |
| --- | --- | ---: | ---: | ---: |
| Sem watcher | `limitless_ui` | 1.110,5 / 1.109,6 | 709,4 / 1.213,1 | 708,9 / 953,3 |
| Sem watcher | core | 745,2 / 744,5 | 468,7 / 817,1 | 511,6 / 758,2 |
| Sem watcher | frontend | 1.357,4 / 1.356,5 | 987,6 / 1.461,8 | 568,7 / 735,7 |
| Com watcher | `limitless_ui` | 1.116,8 / 1.115,9 | 709,2 / 1.211,1 | 697,6 / 948,5 |
| Com watcher | core | 747,5 / 746,6 | 475,5 / 826,8 | 516,3 / 763,9 |
| Com watcher | frontend | 1.347,2 / 1.346,3 | 981,2 / 1.457,6 | 558,9 / 726,0 |

A latência caiu **27–37%**, mas a CPU subiu **8–11%**. No core, a primeira
resposta ficou mais rápida que a do Dart, enquanto a CPU passou de menor para
maior que a dele. Esse resultado não atende ao objetivo conjunto de latência,
CPU e memória.

**Buscas quentes**, medianas de 48 pedidos por célula, nas mesmas unidades:

| Cliente | Projeto | Antes | Experimento retirado | Dart 3.6.2 |
| --- | --- | ---: | ---: | ---: |
| Sem watcher | `limitless_ui` | 151,5 / 150,9 | 150,7 / 150,2 | 17,4 / 20,0 |
| Sem watcher | core | 60,8 / 60,3 | 61,1 / 60,7 | 12,8 / 13,1 |
| Sem watcher | frontend | 185,5 / 185,0 | 187,1 / 186,7 | 18,1 / 25,6 |
| Com watcher | `limitless_ui` | 11,7 / 11,2 | 11,5 / 11,1 | 16,1 / 17,6 |
| Com watcher | core | 9,1 / 8,7 | 8,9 / 8,5 | 12,8 / 12,9 |
| Com watcher | frontend | 12,6 / 12,2 | 12,3 / 11,9 | 18,1 / 22,4 |

**Footprint final**, medianas em MiB:

| Cliente | Projeto | Antes | Experimento retirado | Dart 3.6.2 |
| --- | --- | ---: | ---: | ---: |
| Sem watcher | `limitless_ui` | 134,3 | 141,1 | 672,4 |
| Sem watcher | core | 124,3 | 129,0 | 506,8 |
| Sem watcher | frontend | 585,3 | 586,3 | 706,6 |
| Com watcher | `limitless_ui` | 134,2 | 143,3 | 673,6 |
| Com watcher | core | 123,5 | 127,7 | 503,4 |
| Com watcher | frontend | 583,7 | 586,4 | 708,6 |

O footprint subiu em todas as células, entre **1,0 e 9,1 MiB**. As amostras
finais não incluem o pico simultâneo dos trabalhadores durante a construção.
O experimento não virou opção ou caminho padrão do servidor. Código preservado
em `/tmp/dartforge-symbol-prefetch-source-final.{rs,patch}` e binário no caminho
e SHA acima; respostas em
`/tmp/dartforge-symbol-prefetch-{matrix,watched-matrix}-{limitless,core,frontend}-{before,after,dart}-{0,1,2}.jsonl`,
resumos `/tmp/dartforge-symbol-prefetch-{summary,watched-summary}.log` e manifestos
`/tmp/dartforge-symbol-prefetch-{limitless,core,frontend}-inputs-{before,after}.json`.

### Continuação: compartilhar o nome do contêiner entre seus membros (05/11)

Cada campo, método, construtor e acessor conservava uma cópia própria do nome
da classe ou do mixin em `containerName`. O índice agora guarda uma única
cadeia imutável compartilhada pelos membros da declaração. Os campos e o
construtor primário dos extension types participam do mesmo compartilhamento.
O texto pertence aos metadados da biblioteca e sai quando sua última referência
é descartada; não há tabela global de nomes ou histórico de versões.

A representação interna passou de `Option<String>` para `Option<Arc<str>>`.
Isso também reduz o espaço do campo em cada declaração, inclusive quando
ausente. A resposta JSON continua contendo o mesmo texto de `containerName`.
Uma regressão verifica o compartilhamento dentro da classe e do extension type,
a substituição do nome após uma edição, a igualdade com um índice novo e a
liberação da cadeia antiga por referência fraca. A resposta anterior permanece
com seu texto original.

O oráculo de parts ampliado no experimento anterior foi mantido. Com
`--padding-files 320`, passaram **36/36 com e 36/36 sem watcher** na versão
sequencial com nomes compartilhados. Somados aos 378 controles anteriores,
foram **450/450 respostas completas iguais ao Dart**. Passaram **337 testes
do LSP, zero falhas e zero ignorados**, **11 testes Python**, clippy do workspace
sem avisos, documentação estrita e formatação. A expansão da comparação entre
parsers manteve metadados iguais em **17.645 arquivos Dart únicos**, sem contar
cada arquivo como um teste. O teste de comparação com a extração paralela não
integra esta versão. O workspace inteiro e a suíte nativa não foram repetidos.

Logs em `/tmp/dartforge-symbol-container-{unit,tests,clippy,doc,python,corpus,controls}.log`;
respostas em `/tmp/dartforge-symbol-container-control-*.jsonl`,
`/tmp/dartforge-symbol-container-aliases{,-unwatched}.jsonl`,
`/tmp/dartforge-symbol-container-parts{,-unwatched}.jsonl` e
`/tmp/dartforge-symbol-container-padded-parts{,-unwatched}.jsonl`.
O baseline continua `d79cce98`, preservado em `/tmp/dartforge-symbol-container-before`,
SHA-256 iniciado em `05ab494ee0b1`; o novo binário está em
`/tmp/dartforge-symbol-container-after`, SHA-256
`bd326c1bf6655146d423983721efeb6d20a9770f2d61e106c1b4340fe0f1c549`.

No ensaio preliminar do frontend, a primeira busca passou de **1.385,0 para
1.352,5 ms**, CPU **1.384,0 → 1.351,6 ms**, mediana quente **185,8 → 185,8 ms**
e footprint final **582,7 → 570,8 MiB**. As **24/24 respostas completas** foram
iguais antes/depois. É uma sessão por versão, sem Dart pareado; esses tempos
ficam fora das matrizes revezadas. Artefatos em
`/tmp/dartforge-symbol-container-quick-{before,after}.jsonl`.

As **54 sessões revezadas** mantiveram **216/216 respostas completas iguais em
cada comparação** antes/depois, antes/Dart e depois/Dart nos dois modos de
watcher. Projetos, SDK 3.6.2, caches, documento de preparação e dez segundos
extras de ociosidade foram mantidos. Compilações e testes terminaram antes
das medições. Manifestos idênticos antes/depois, com 9.761 / 6.640 / 10.894
arquivos e nenhuma raiz ausente.

**Footprint final**, medianas de três sessões em MiB:

| Cliente | Projeto | Antes | Depois | Dart 3.6.2 |
| --- | --- | ---: | ---: | ---: |
| Sem watcher | `limitless_ui` | 134,7 | 132,2 | 673,3 |
| Sem watcher | core | 123,8 | 124,8 | 499,6 |
| Sem watcher | frontend | 583,4 | 575,3 | 705,5 |
| Com watcher | `limitless_ui` | 133,8 | 132,3 | 673,8 |
| Com watcher | core | 123,2 | 124,4 | 505,6 |
| Com watcher | frontend | 582,0 | 574,2 | 715,8 |

A queda do frontend foi de **8,1 / 7,8 MiB**, menor que os 11,9 MiB do ensaio
preliminar. O `limitless_ui` caiu **2,5 / 1,5 MiB**; o core subiu **1,0 / 1,2 MiB**.
Compartilhar o texto reduz cópias nos metadados, mas não demonstra redução
geral do footprint: o efeito do alocador também participa dessas amostras
finais. Não medem pico simultâneo da árvore nem o platô com ondas de gerados.

**Primeira busca**, medianas de três sessões, latência / CPU da árvore em ms:

| Cliente | Projeto | Antes | Depois | Dart 3.6.2 |
| --- | --- | ---: | ---: | ---: |
| Sem watcher | `limitless_ui` | 1.129,7 / 1.128,9 | 1.110,0 / 1.109,3 | 717,6 / 962,5 |
| Sem watcher | core | 741,1 / 740,3 | 736,5 / 735,8 | 514,0 / 761,9 |
| Sem watcher | frontend | 1.361,8 / 1.360,9 | 1.351,0 / 1.350,0 | 560,1 / 728,5 |
| Com watcher | `limitless_ui` | 1.108,3 / 1.107,4 | 1.118,4 / 1.117,6 | 699,9 / 944,4 |
| Com watcher | core | 737,2 / 736,5 | 737,7 / 737,0 | 522,5 / 777,9 |
| Com watcher | frontend | 1.344,9 / 1.344,1 | 1.344,6 / 1.343,8 | 558,1 / 725,3 |

Sem watcher, os tempos caíram pouco, cerca de 1–2%; com watcher não houve
ganho geral. A primeira resposta continua mais lenta que a do Dart nos três
projetos. O compartilhamento de nomes não resolve o custo da construção inicial.

**Buscas quentes**, medianas de 48 pedidos por célula, nas mesmas unidades:

| Cliente | Projeto | Antes | Depois | Dart 3.6.2 |
| --- | --- | ---: | ---: | ---: |
| Sem watcher | `limitless_ui` | 150,4 / 150,0 | 150,5 / 149,9 | 16,7 / 20,4 |
| Sem watcher | core | 60,9 / 60,4 | 60,7 / 60,3 | 12,9 / 13,2 |
| Sem watcher | frontend | 185,6 / 185,0 | 186,7 / 186,1 | 18,4 / 25,5 |
| Com watcher | `limitless_ui` | 11,7 / 11,2 | 11,8 / 11,3 | 15,7 / 17,6 |
| Com watcher | core | 9,0 / 8,5 | 9,0 / 8,5 | 12,8 / 13,2 |
| Com watcher | frontend | 12,5 / 12,0 | 12,6 / 12,1 | 18,1 / 22,7 |

As buscas quentes ficaram próximas do baseline. O frontend sem watcher subiu
1,1 ms de latência e CPU; com watcher, frontend e `limitless_ui` subiram 0,1 ms.
O ganho desta etapa é a memória final nos dois projetos, sem vitória geral
em tempo ou CPU. Resultados em
`/tmp/dartforge-symbol-container-{matrix,watched-matrix}-{limitless,core,frontend}-{before,after,dart}-{0,1,2}.jsonl`,
resumos `/tmp/dartforge-symbol-container-{summary,watched-summary}.log` e manifestos
`/tmp/dartforge-symbol-container-{limitless,core,frontend}-inputs-{before,after}.json`.
A dupla fria/quente, o protocolo legado, as edições nos projetos grandes e a
sessão longa não foram repetidos nesta etapa; os resultados anteriores e as
reprovações conhecidas continuam registrados separadamente.

### Metadados de disco por pedido (05/11)

Sem notificações confiáveis do cliente, cada `workspace/symbol` revalida os
arquivos no disco. A mesma busca consultava os metadados de um caminho ao
reter os arquivos conhecidos, ao reter as colisões de URIs e ao validar a
biblioteca. Uma reconstrução ainda podia consultá-los de novo antes da leitura.
Essas fases agora compartilham o estado do disco somente durante aquele pedido.

O mapa existe apenas ao revalidar uma descoberta anterior, com os mesmos
arquivos adicionados, SDK e catálogos, sem confiança no disco. A primeira
indexação, a substituição dos catálogos e os pedidos com watcher confiável
continuam sem esse mapa. As chaves conservam a grafia exata do caminho no
sistema operacional; aliases não são fundidos. O estado distingue arquivo de
diretório e conserva separadamente o fingerprint de data e tamanho.

O mapa sai ao terminar o pedido, inclusive seus registros de arquivos ausentes.
A busca seguinte consulta o disco novamente. Documentos abertos continuam
usando o texto do editor. A seleção entre fonte e arquivo gerado no resolvedor
de URIs continua consultando sua existência a cada resolução, sem congelar o
alvo escolhido.

Uma regressão verifica criação, edição e exclusão entre snapshots, estabilidade
dentro de um pedido, leitura fresca quando o compartilhamento está desativado
e distinção entre arquivo e diretório. Passaram **338 testes do LSP**, sem
falhas ou ignorados, **11 testes Python**, clippy do workspace sem avisos,
documentação estrita e formatação. Os **450/450 controles** mantiveram as
respostas completas iguais ao Dart, incluindo as transições de parts com e
sem watcher e a ordem dos 320 símbolos adicionais. O workspace inteiro e a
suíte nativa não foram repetidos nesta etapa.

Logs em `/tmp/dartforge-symbol-stat-{tests,clippy,doc,python,controls}.log`;
respostas em `/tmp/dartforge-symbol-stat-control-*.jsonl`,
`/tmp/dartforge-symbol-stat-aliases{,-unwatched}.jsonl`,
`/tmp/dartforge-symbol-stat-parts{,-unwatched}.jsonl` e
`/tmp/dartforge-symbol-stat-padded-parts{,-unwatched}.jsonl`.
O baseline é `e437a160`, preservado em `/tmp/dartforge-symbol-stat-before`,
SHA-256 `bd326c1bf6655146d423983721efeb6d20a9770f2d61e106c1b4340fe0f1c549`;
o novo binário está em `/tmp/dartforge-symbol-stat-after`, SHA-256
`010d09c8d2194f61e58f9be6980ac24fa4778761eacf970c455a2486867e8351`.

As **54 sessões revezadas** mantiveram **216/216 respostas completas iguais em
cada comparação** antes/depois, antes/Dart e depois/Dart, em cada modo de
watcher. As configurações, o documento de preparação, os caches e os dez
segundos extras de ociosidade foram iguais. Compilações e testes terminaram
antes da medição. Os manifestos antes/depois permaneceram idênticos, com
9.761 / 6.640 / 10.894 arquivos e nenhuma raiz ausente.

**Buscas quentes**, medianas de 48 pedidos por célula, latência / CPU da árvore em ms:

| Cliente | Projeto | Antes | Depois | Dart 3.6.2 |
| --- | --- | ---: | ---: | ---: |
| Sem watcher | `limitless_ui` | 150,8 / 150,2 | 141,0 / 140,5 | 16,6 / 19,1 |
| Sem watcher | core | 61,3 / 61,0 | 54,9 / 54,4 | 12,7 / 13,1 |
| Sem watcher | frontend | 184,8 / 184,3 | 175,4 / 174,9 | 18,0 / 24,6 |
| Com watcher | `limitless_ui` | 11,8 / 11,3 | 11,8 / 11,3 | 16,0 / 17,7 |
| Com watcher | core | 9,3 / 8,8 | 8,9 / 8,4 | 12,7 / 13,1 |
| Com watcher | frontend | 12,5 / 12,1 | 12,4 / 11,9 | 18,0 / 22,5 |

Sem watcher, a latência caiu **6,4% / 10,4% / 5,1%**, e a CPU,
**6,5% / 10,8% / 5,1%**. Ainda perde do Dart nessa modalidade. Com watcher,
as diferenças ficaram entre zero e 0,4 ms; o mapa novo não participa do
caminho confiável. A busca quente com watcher continua ganhando do Dart.

**Primeira busca**, medianas de três sessões, nas mesmas unidades:

| Cliente | Projeto | Antes | Depois | Dart 3.6.2 |
| --- | --- | ---: | ---: | ---: |
| Sem watcher | `limitless_ui` | 1.112,5 / 1.111,6 | 1.125,1 / 1.124,3 | 699,9 / 950,7 |
| Sem watcher | core | 739,4 / 738,6 | 755,1 / 752,0 | 514,3 / 764,8 |
| Sem watcher | frontend | 1.350,3 / 1.349,5 | 1.347,4 / 1.346,5 | 567,7 / 735,1 |
| Com watcher | `limitless_ui` | 1.123,4 / 1.122,5 | 1.100,3 / 1.099,4 | 694,7 / 943,3 |
| Com watcher | core | 738,7 / 737,9 | 733,8 / 733,0 | 510,4 / 761,2 |
| Com watcher | frontend | 1.348,4 / 1.347,6 | 1.338,4 / 1.337,5 | 552,6 / 720,8 |

O mapa permanece desativado na construção inicial. Sem watcher, as medianas
de primeira latência subiram **12,6 / 15,7 ms** nos dois primeiros projetos
(1,1% / 2,1%); no frontend caíram 2,9 ms. Com watcher caíram nos três projetos.
Não há ganho geral demonstrado na primeira busca, que continua mais lenta que
a do Dart. As variações individuais e a ordem das sessões ficam nos artefatos.

**Footprint final**, medianas de três sessões em MiB:

| Cliente | Projeto | Antes | Depois | Dart 3.6.2 |
| --- | --- | ---: | ---: | ---: |
| Sem watcher | `limitless_ui` | 132,2 | 132,8 | 678,5 |
| Sem watcher | core | 124,8 | 125,3 | 506,8 |
| Sem watcher | frontend | 574,8 | 576,0 | 704,4 |
| Com watcher | `limitless_ui` | 132,3 | 131,5 | 670,9 |
| Com watcher | core | 124,6 | 124,5 | 508,4 |
| Com watcher | frontend | 574,8 | 574,9 | 711,8 |

O custo final sem watcher foi **+0,6 / +0,5 / +1,2 MiB**. Descartar o mapa
ao fim do pedido não obriga o alocador a devolver suas páginas ao sistema.
A mudança reduz tempo e CPU na revalidação, mas não reduz a memória final
dessa matriz. Os três projetos continuam abaixo do Dart. Essas amostras não
medem pico simultâneo da árvore ou estabilidade em sessões longas.

Os ensaios preliminares de uma sessão por versão ficam fora das matrizes:
`/tmp/dartforge-symbol-stat-quick-{before,after}.jsonl` avaliou o mapa em todos
os pedidos; `/tmp/dartforge-symbol-stat-selective-{before,after}.jsonl`, a
ativação restrita mantida. A primeira tentativa da matriz parou antes de
iniciar o Dart por falta de `ORACLE_DART` explícito. Foi preservada em
`/tmp/dartforge-symbol-stat-aborted-matrix-*` e reiniciada integralmente;
nenhuma dessas sessões integra as tabelas.

Resultados em
`/tmp/dartforge-symbol-stat-{matrix,watched-matrix}-{limitless,core,frontend}-{before,after,dart}-{0,1,2}.jsonl`,
resumos `/tmp/dartforge-symbol-stat-{summary,watched-summary}.log` e manifestos
`/tmp/dartforge-symbol-stat-{limitless,core,frontend}-inputs-{before,after}.json`.
A dupla fria/quente, o protocolo legado, as edições nos projetos grandes e a
sessão longa não foram repetidos; as reprovações e limitações anteriores
continuam registradas separadamente.

## Retorno à recarga incremental: slots por nome, em validação (05/11)

A recarga no lugar reconhecia somente a lista posicional de elementos: uma
declaração acrescentada no fim podia manter os dependentes; uma inserção no
meio deslocava os IDs e exigia o corte de reserva. O protótipo em andamento
casa classes, extensions, typedefs, funções e variáveis por espécie, dono e
nome. Getter e setter implícitos têm papéis distintos, mesmo quando compartilham
o nome e a espécie da função.

O casamento inicial era limitado ao intervalo contíguo de cada biblioteca e
não bastou no frontend real: a ordenação das bibliotecas deslocava uma classe;
depois de conservar essa ordem, uma função nova ainda deslocava a biblioteca
seguinte. Agora todas as bibliotecas estáveis reservam os slots sobreviventes
antes de distribuir os restantes às novas declarações. Uma biblioteca pode
ocupar slots descontínuos. A ordem anterior das bibliotecas é reaproveitada
somente quando suas URIs e as arestas de imports/exports continuam iguais.
Mudanças no grafo usam a ordenação normal por dependências.

Nomes ambíguos, sobreviventes fora do sufixo novo e permutações que mudariam
a ordem dos construtores recusam o reparo. Não há tombstones; parts também
continuam no fallback. Slots descontínuos podem impedir um checkpoint antes
de outra biblioteca: nesse caso, a próxima edição recarrega a partir do prefixo
congelado. A identidade geral e o outline de tipos por biblioteca continuam abertos.

**A ordem das fases foi corrigida durante a validação.** Restaurar IDs depois
de resolver supertypes deixava bibliotecas novas vendo bindings antigos dos
barrels mantidos. Uma regressão acrescenta `Added extends self.NewBase`, onde
`self` é um barrel que reexporta a própria biblioteca. A primeira versão
deixava a superclasse sem alvo; a corrigida encontra o ID de `NewBase` e publica
os mesmos diagnósticos de uma análise do zero.

Agora a criação das declarações precede o callback de reparo. Ele valida os
slots e os nomes usados nas assinaturas dos dependentes, restabelece os imports
e exports mantidos e devolve os exporters/importers afetados. Essas bibliotecas
participam do mesmo fixpoint de exports e da construção de escopos das novas,
antes da resolução de supertypes. A recusa interrompe o outline parcial antes
dessas fases; o residente então recarrega pelo corte de reserva.

Antes do reparo de dependências de aliases descrito abaixo, passaram **456 testes**
nas suítes completas de `elements`, `parity` e LSP,
incluindo ignorados e doctests, além de clippy de todos os alvos do workspace
e documentação estrita. A regressão do residente cobre IDs estáveis, genéricos,
acessores, aliases, alteração e reversão de assinatura, edição de corpo após a
permutação, mudança de assinatura do construtor sem nome, ciclo com barrel e
os fallbacks. Dois testes da arena verificam reservas entre bibliotecas e
recusas; o teste de layout verifica grafo igual, aresta alterada e biblioteca nova.
O teste de publicação do LSP ganhou
sete passos de inserções/reversões e compara cada publicação com uma análise
do zero, nos modos local e residente.

Logs atuais: `/tmp/dartforge-named-slots-global-{focused,clippy2,doc}.log`.
A falha real da superclasse nova está em
`/tmp/dartforge-named-slots-new-export-before-final.log`; o caso corrigido,
em `/tmp/dartforge-named-slots-new-export-after.log`. O arquivo
`/tmp/dartforge-named-slots-alias-before.log` contém um teste aprovado de
fallback, não uma reprovação de propagação de aliases.

**A primeira execução ampla não é um gate verde.** O comando completo do
workspace terminou com código 101 e dois alvos de doctests reprovados:
`elements --doc` e `parity --doc`. As fontes foram alteradas durante a execução;
o rustdoc encontrou as APIs novas, mas recebeu as bibliotecas antigas, com a
assinatura anterior do callback. A repetição direcionada com a API atual passou.
O log integral permanece em `/tmp/dartforge-named-slots-workspace.log`; será
necessária uma execução ampla com a árvore estável. A repetição em
`/tmp/dartforge-named-slots-stable-workspace.log` foi interrompida com código 143
durante os builders nativos, após encontrar o erro de alias abaixo. Não é um
gate aprovado. A nova execução está em `/tmp/dartforge-named-slots-alias-workspace.log`,
precedida da construção de um CLI nativo preservado em
`/tmp/dartforge-named-slots-native-current`, usado pelos dois seletores de compiler
dos builders. Depois vêm CLI `native,jit`, clippy e documentação estrita.
A suíte independente anterior do CLI passou em **117
testes**, antes da ampliação para slots globais; não substitui essa repetição.

**O projeto grande ainda não demonstrou o benefício.** O driver agora aceita
`--api-offset` (byte UTF-8), também por projeto no `editing_compare.py`, e grava
`api_edit_diagnostics` após cada edição e reversão. A opção `--api-queries`
acrescenta as respostas completas de documentSymbol, workspace/symbol, hover
e references após o limite cronometrado; fica desligada nas medições de edição.
No frontend, a função pública
com erro foi inserida no byte 114 de `documento_service.dart`, antes da classe,
mantendo os arquivos no disco intactos. As quatro capturas completas foram
iguais antes/depois e iguais ao Dart ao omitir somente o identificador `source`.
Houve um diagnóstico em cada edição e nenhum após cada reversão; nenhuma
publicação esperada faltou.

São sessões de correção durante outros testes, excluídas das medições de
desempenho. O rastro inicial mostrou o intervalo da arena de classes mudando
de `8223/1` para `8224/1` e a biblioteca de `3122` para `3123`. Conservar a ordem
resolveu esse deslocamento, mas ainda deixou funções da outra biblioteca
deslocadas em um slot. O reparo global mantém os dependentes nas quatro recargas
posteriores ao aquecimento. As seis capturas completas de diagnósticos e o
estado final coincidiram com o baseline. A comparação ampliada de consultas
terminou nos três projetos; não há ganho de API reivindicado para nenhum deles.

O baseline é `e13d8e35`, em `/tmp/dartforge-named-slots-before`, SHA-256
`010d09c8d2194f61e58f9be6980ac24fa4778761eacf970c455a2486867e8351`.
O release com slots globais anterior ao reparo de aliases está em `/tmp/dartforge-named-slots-global-release`,
SHA-256 `23251f0e5bbbce0e80064fa2a60a0bff6604054d982b4e0d3f1e13b4f4093b4b`;
o driver, em `/tmp/dartforge-named-slots-final-driver`, SHA-256
`864c36f6b100a9faf227955c2320434e639f1cd230108d06c53858d75e809fab`.
Esse release manteve **450/450 controles completos de workspace/symbol**,
incluindo aliases, parts e a variante com 320 arquivos extras nos dois modos
de watcher. Logs e respostas em `/tmp/dartforge-named-slots-global-control-*`;
o comando de todos os controles terminou com código zero.

Na rodada ampliada, os três projetos mantiveram **18/18 respostas completas de
consultas e 6/6 capturas de diagnósticos por projeto**, além do estado final,
antes/depois. Os diagnósticos dos arquivos abertos coincidiram com o Dart,
omitindo somente `source`. Os payloads de hover, references e workspace/symbol
também coincidiram. A resposta completa de documentSymbol continua diferente:
o Dart envia `deprecated: false`, ausente nas duas versões do DartForge. No
limitless, o mapa completo de diagnósticos tem publicações distintas em
subprojetos; no core e frontend, coincidiu integralmente. São falhas anteriores
preservadas, não paridade geral aprovada. Nenhuma consulta devolveu erro e nenhum
diagnóstico esperado faltou. As capturas estão
em `/tmp/dartforge-named-slots-global-correctness/` e a comparação completa,
em `/tmp/dartforge-named-slots-global-correctness-verification.json`.

**Uma substituição do mesmo tamanho revelou uma dependência indireta esquecida.**
Retirar uma classe não precisa reduzir a arena: outra classe pode ocupar seu
slot. Com `typedef RetiredAlias = Retired` e um dependente que estende esse
alias, a assinatura mantida não contém o nome `Retired`. O guard anterior
aceitava a recarga e deixava o supertipo antigo apontando para a classe nova.
Uma atribuição de `AliasUser()` a uma variável local `Replacement` perdia o
diagnóstico `invalid_assignment`, presente na análise do zero. O ensaio externo
reprovado está em `/tmp/dartforge-named-slots-alias-replacement2.log`; a regressão
no repositório, em `/tmp/dartforge-named-slots-alias-replacement-red.log`.

O reparo propaga os nomes alterados por aliases não SDK, incluindo os mantidos,
e pelos cabeçalhos das classes recarregadas, até um fixpoint. Propaga também a
mudança dos membros de uma classe para seus aliases e herdeiros, de modo que
o mesmo guard de assinaturas possa reconhecer a dependência indireta. A
regressão foi ampliada para duas camadas de alias e uma superclasse intermediária;
verifica o diagnóstico completo antes de exigir a recarga pelo corte de reserva.
Passaram a regressão e os **52 testes unitários de parity**, além de clippy.
Logs: `/tmp/dartforge-named-slots-alias-{replacement-green,units,clippy}.log`.
O mesmo ensaio externo originalmente reprovado também passou, em
`/tmp/dartforge-named-slots-alias-replacement-fixed.log`.

O novo release está em `/tmp/dartforge-named-slots-alias-release`, SHA-256
`8980b29edae6ccd1a514a085b62a6f594d35519a8ea67cb3816da43bae608886`.
Passaram novamente **450/450 controles completos**, em
`/tmp/dartforge-named-slots-alias-control-*`. A repetição ampliada de consultas
em `/tmp/dartforge-named-slots-alias-correctness/` manteve **54/54 payloads completos
de consultas e 18/18 capturas de diagnósticos** antes/depois nos três projetos,
além do estado final. Nenhum pedido devolveu erro ou diagnóstico esperado faltou.
Os diagnósticos dos arquivos abertos coincidiram com o Dart, omitindo somente
`source`; as diferenças anteriores de documentSymbol e dos subprojetos do
limitless permaneceram. A comparação está em
`/tmp/dartforge-named-slots-alias-correctness-verification.json`.
Os três manifestos
`/tmp/dartforge-named-slots-{limitless,core,frontend}-inputs-recheck.json` ficaram
idênticos aos anteriores: 9.761 / 6.640 / 10.894 arquivos, sem raízes ausentes.
A suíte ampla desta versão terminou com código zero: **3.443 testes do workspace
e 117 do CLI native/jit**, sem ignorados. Passaram também 11 testes Python,
formatação, clippy de todos os alvos do workspace e documentação estrita.
Resultados em `/tmp/dartforge-named-slots-alias-{workspace,native,workspace-clippy,doc,python,fmt}.log`
e contagens em `/tmp/dartforge-named-slots-alias-gate.json`.
O snapshot de 1.962 fontes e dos três
binários está em `/tmp/dartforge-named-slots-alias-source.json`; nenhuma fonte
mudou durante os checks. Esse gate antecede o reparo de cabeçalho descrito abaixo.
O teste completo de builders pelo executor nativo já passou nessa execução.
Também passaram os três testes de `typed_incremental` no código atual, incluindo
as publicações locais e do residente após as inserções e reversões;
log em `/tmp/dartforge-named-slots-alias-publications.log`.
Os rastros do release atual, sem consultas adicionais, confirmaram quatro
recargas com dependentes preservados no frontend e quatro no core. No core,
a carga ainda expande o conjunto de bibliotecas durante a sessão, e uma
tentativa recusou os slots; não são quatro recargas consecutivas após aquecimento.
O cenário do limitless não entrou no ramo de reparo: seus cortes voltaram ao
prefixo congelado durante as edições de API. Nenhum diagnóstico esperado faltou.
Artefatos em `/tmp/dartforge-named-slots-alias-trace-{limitless,core,frontend}.{json,log}`.
São ensaios de mecanismo durante a suíte, excluídos da comparação de tempos;
o benefício no limitless continua sem demonstração.

Um ensaio externo adicional manteve os dependentes com uma variável de tipo
inferido por uma função cujo retorno usa o alias alterado. Os diagnósticos
continuaram iguais aos da análise do zero, sem uma recusa desnecessária;
log em `/tmp/dartforge-named-slots-inferred-dependency.log`. Ele não acrescenta
testes ao total da suíte do repositório.

**A dívida de documentSymbol foi delimitada, ainda sem correção.** Um projeto
pequeno, sem erros de análise no Dart, consulta os formatos plano e hierárquico,
com e sem anúncio de suporte a tags: **0/4 respostas completas iguais**.
No plano falta `deprecated`; no hierárquico faltam também `detail` de funções,
há arrays `children` vazios em lugar de campos ausentes e diferenças de spans,
inclusive junto a annotations. O Dart envia `deprecated` tanto falso como
verdadeiro nos quatro modos. Uma classe depreciada não torna seus membros
não anotados depreciados. O ensaio cobre campos, acessores, construtores,
funções locais, aliases, constantes de enum e annotation prefixada.
O controle com import exclusivamente prefixado deixa as annotations não
qualificadas sem resolução: elas recebem falso, enquanto `@sdk.deprecated`
recebe verdadeiro. Buscar apenas o texto do nome não reproduz essa resolução.
Artefatos em `/tmp/dartforge-document-symbol-deprecation-probe.{py,jsonl,log}`
e `/tmp/dartforge-document-symbol-deprecation-unresolved-before.{jsonl,log}`.
Esses casos são uma reprovação funcional registrada, não parte dos 450 controles
de workspace/symbol, nem uma mudança no código em validação.

**Terceira regressão: bounds depois de uma annotation com mapa.** O recorte do
cabeçalho usava a primeira `{`, que podia pertencer à annotation. Mudar
`B<T extends num>` para `B<T extends String>` era então confundido com uma mudança
só de membros. Uma função mantida, `String use(B value) => value.item`, conservava
`B<num>`: publicava um erro de bound e um de retorno, enquanto a análise do zero
não publicava nenhum. O primeiro ensaio, só de método herdado, manteve os mesmos
diagnósticos; não foi tratado como falha. O caso de bound reprovou o ensaio externo
e a regressão no repositório antes do reparo.

O recorte agora usa o fim dos nós de tipo, parâmetros, representação e nome da
declaração na AST. Não precisa procurar chaves nem reler os corpos. Um teste cobre
seis formas, incluindo bounds com records, annotations com mapas e um construtor
primário que possui uma parte `this` no corpo. Alterar o cabeçalho muda o descritor;
alterar só o corpo o mantém. Passaram **54 testes unitários de parity e 338 do
LSP**, clippy do workspace e documentação estrita na versão corrigida.
Artefatos: `/tmp/dartforge-named-slots-{metadata-header,metadata-bound}.log`,
`/tmp/dartforge-named-slots-header-{red,units,lsp,clippy,doc}.log`.
O novo release está em `/tmp/dartforge-named-slots-header-release`, SHA-256
`b9c037d0bb8b9943899b084f9d90ec71c3e762e3aeb7f20a826dda35aa899f95`.
O snapshot atual está em `/tmp/dartforge-named-slots-header-source.json`; desde
o gate completo anterior, só mudaram `analysis.rs` e `in_place.rs` em parity.
Os 450 controles e as capturas dos três projetos foram repetidos no novo release:
**450/450 respostas completas de símbolos, 54/54 respostas de consultas e 18/18
capturas de diagnósticos antes/depois**, além do estado final. Não houve pedidos
com erro ou diagnósticos esperados ausentes. As diferenças anteriores contra o
Dart permaneceram registradas. Resultados em
`/tmp/dartforge-named-slots-header-controls.log` e
`/tmp/dartforge-named-slots-header-correctness/`, com comparação completa em
`/tmp/dartforge-named-slots-header-verify.py`.
A medição isolada veio antes da nova suíte ampla, para resolver eventuais
regressões de desempenho antes de repetir os gates longos. Ainda não há commit
desta etapa.

**Primeira matriz isolada após o reparo de cabeçalho (05/11).** Foram 27 sessões:
três projetos, três servidores e três rotações da ordem. O driver fez oito
inserções/reversões da função pública por sessão nos mesmos offsets, sem
`--api-queries`, com `--positions 0`, sem rastros e sem builds/testes concorrentes.
Cada mediana de edição/CPU reúne 24 amostras por projeto e servidor; partida e
memória são medianas de três sessões. As publicações dos arquivos abertos foram
conferidas após cada edição e reversão, fora do limite cronometrado.
As colunas de edição medem as inserções; as reversões têm capturas de correção,
mas não uma coluna própria de latência ou CPU neste ensaio.

| Projeto | Medida de edição de API (ms) | Antes | Depois | Dart 3.6.2 |
|---|---|---:|---:|---:|
| limitless | Primeiro diagnóstico | 25,1 | 25,0 | 32,6 |
| limitless | Até estabilizar | 25,1 | 25,0 | 59,0 |
| limitless | CPU da árvore | 25,4 | 25,3 | 65,4 |
| core | Primeiro diagnóstico | 27,3 | 14,0 | 6,2 |
| core | Até estabilizar | 27,3 | 14,0 | 24,5 |
| core | CPU da árvore | 36,8 | 14,3 | 24,0 |
| frontend | Primeiro diagnóstico | 171,4 | 23,0 | 20,2 |
| frontend | Até estabilizar | 171,4 | 23,0 | 42,9 |
| frontend | CPU da árvore | 281,9 | 23,1 | 47,7 |

Até estabilizar, a queda foi de **48,8% no core e 86,6% no frontend**; a CPU caiu
**61,1% e 91,8%**. O limitless não mostrou ganho consistente, como o rastro sem
entrada no ramo de reparo já sugeria. As edições de corpo ficaram próximas do
baseline. O primeiro diagnóstico ainda chega antes no Dart no core e frontend:
ganhar no status final e na CPU não fecha essa lacuna.

| Projeto | Footprint da árvore (MiB) | Antes | Depois | Dart |
|---|---|---:|---:|---:|
| limitless | Pico amostrado | 332,0 | 331,5 | 581,2 |
| limitless | Em repouso ao final | 332,0 | 331,5 | 460,6 |
| core | Pico amostrado | 294,2 | 292,3 | 497,9 |
| core | Em repouso ao final | 292,1 | 292,3 | 488,1 |
| frontend | Pico amostrado | 558,5 | 567,9 | 596,2 |
| frontend | Em repouso ao final | 554,0 | 423,9 | 430,2 |

Não há ganho uniforme de memória. O pico do frontend subiu na mediana, com faixas
**555,9–566,1 antes e 555,6–569,0 MiB depois**; o pico de RSS foi de **568,8 para
577,8 MiB**. O repouso do frontend variou de **419,4 a 557,5 MiB** depois, contra
**551,1–555,8 antes e 427,7–515,2 no Dart**. No core, o repouso depois variou de
291,0 a 296,4 MiB. A mediana menor do frontend não prova uma vitória estável em
repouso. O footprint final é medido após dois segundos sem edições; os picos
são amostrados a cada 25 ms e não são o máximo simultâneo exato. O ensaio não
mede sessão longa, frio, primeira consulta ou Windows.

As 27 sessões terminaram com código zero e mantiveram **144/144 capturas completas
de diagnósticos antes/depois**, além de **9/9 estados finais**. Os arquivos abertos
coincidiram com o Dart em **144/144 capturas**, omitindo somente `source`; os mapas
completos do core e frontend também coincidiram. O mapa global do limitless
continuou distinto nos subprojetos nas duas versões do DartForge. Os manifestos
de entrada ficaram idênticos antes/depois, e nenhuma fonte do snapshot mudou.

Artefatos em `/tmp/dartforge-named-slots-api-matrix/`, relatório
`/tmp/dartforge-named-slots-api-matrix.log`, conferência completa
`/tmp/dartforge-named-slots-api-verification.{json,log}` e manifestos
`/tmp/dartforge-named-slots-{limitless,core,frontend}-matrix-inputs-{before,after}.json`.
Os custos remanescentes e a variação de memória precisam ser investigados antes
de fechar a etapa; a nova suíte ampla ainda não foi iniciada.

**Perfis posteriores, fora da matriz.** Um primeiro cliente exploratório usou
substituições completas e capacidades diferentes das do driver; seu perfil não
foi usado para explicar os tempos da matriz. A amostragem foi repetida com o
driver real, 40 repetições, mesmas capacidades e edições incrementais. Depois
do ensaio de corpo, `sample` acompanhou o residente por dez segundos durante
as edições de API. O driver e o amostrador terminaram com código zero.
Entre os frames de trabalho aparecem reconstrução de namespaces, rehash das
tabelas e `in_place::slots::Elements::capture`; há também muitas amostras de
threads esperando, que não devem ser interpretadas como CPU da recarga.
Arquivos em `/tmp/dartforge-named-slots-driver-profile{.log,.json,.sample.txt,.demangled.txt}`,
`/tmp/dartforge-named-slots-driver-profile-control.log` e
`/tmp/dartforge-named-slots-driver-profile-sample.log`.

**Reutilização de tabelas de escopo avaliada e retirada.** O experimento
reaproveitou as tabelas dos imports mantidos, limpando os bindings antigos e
reduzindo a capacidade quando sobrava pelo menos metade. Uma nova regressão
substitui e desfaz nomes importados, com e sem prefixo, comparando os diagnósticos
com uma análise do zero. Ela passou antes do experimento e foi mantida depois
da retirada. Passaram 55 testes unitários de parity, 338 do LSP, clippy e
documentação estrita; os controles conservaram 450/450 respostas completas,
e as consultas dos projetos mantiveram 54/54 respostas e 18/18 diagnósticos
antes/depois.

Na matriz isolada de 27 sessões contra a versão de cabeçalhos AST, as 144
capturas de edição/reversão e os nove estados finais coincidiram antes/depois.
No frontend, a mediana até estabilizar caiu de 22,85 para 21,15 ms e a CPU de
22,95 para 21,30 ms. O pico de footprint não melhorou (564,3 → 564,8 MiB),
e o repouso ficou maior nas três sessões experimentais: mediana de
420,0 → 559,3 MiB. Esses resultados não provam a causa da retenção, mas
não atendem à meta de memória. A reutilização foi retirada; o `outline.rs`
voltou ao hash da versão AST medida. Não se reivindica seu ganho de CPU.
Artefatos preservados em `/tmp/dartforge-named-slots-scope-matrix/`,
`/tmp/dartforge-named-slots-scope-verification.{json,log}` e
`/tmp/dartforge-scope-reuse-outline.rs`.

**Chaves compactas avaliadas e retiradas.** O layout original no target ARM64 ocupa
**32 bytes por chave e 40 por par `(id, chave)`**. A representação integrada
no experimento usou tags de um byte para as espécies, preservando nome, dono e papel de
acessores. Uma compilação dessas definições, com o `SymbolId` real,
confirmou **16 e 20 bytes**, respectivamente. A redução da estrutura ainda
não demonstra queda de RSS ou footprint do servidor. Fontes e resultado em
`/tmp/dartforge-named-slots-compact-layout.{rs,log}`; o primeiro protótipo
permanece em `/tmp/dartforge-named-slots-key-layout.{rs,log}`.
Passaram 55 testes unitários de parity, 338 do LSP, clippy de todos os alvos e
documentação estrita. O release compacto, SHA-256
`60e98f089cce171c97fc6715266b40e14852b02b4610da08d1dc146ae6cf878b`,
manteve 450/450 respostas completas nos 16 controles de símbolos contra o Dart.
Logs em `/tmp/dartforge-named-slots-compact-{units,lsp,clippy,doc,controls}.log`.
O snapshot `/tmp/dartforge-named-slots-compact-source.json` registra 1.962 fontes
e os três binários preservados. As nove sessões de correção nos projetos reais
mantiveram 54/54 respostas de consultas e 18/18 mapas completos de diagnósticos,
além dos três estados finais, antes/depois. Não houve erro de pedido nem
diagnóstico esperado ausente. Contra o Dart, as 18 capturas dos arquivos abertos
coincidiram ao omitir somente `source`; permanecem as diferenças de documentSymbol
e das publicações globais do limitless. Artefatos em
`/tmp/dartforge-named-slots-compact-correctness/` e
`/tmp/dartforge-named-slots-compact-correctness-verification.{json,log}`.
A matriz isolada de 27 sessões terminou com 144/144 capturas completas de
edição/reversão e nove estados finais iguais antes/depois. Os diagnósticos dos
arquivos abertos também coincidiram nas 144 capturas contra o Dart, omitindo
somente `source`; as diferenças globais do limitless foram preservadas.
Nenhuma publicação esperada faltou. Os três manifestos antes/depois ficaram
idênticos, com 9.761 / 6.640 / 10.894 arquivos e nenhuma raiz ausente.

| Mediana da matriz compacta | Anterior AST | Chaves compactas | Dart |
| --- | ---: | ---: | ---: |
| limitless: API até estabilizar / CPU (ms) | 25,30 / 25,70 | 25,15 / 25,45 | 58,35 / 62,50 |
| core: API até estabilizar / CPU (ms) | 14,10 / 14,50 | 14,70 / 15,00 | 24,60 / 24,05 |
| frontend: API até estabilizar / CPU (ms) | 23,15 / 23,30 | 23,15 / 23,25 | 42,70 / 46,05 |
| limitless: pico / repouso de footprint (MiB) | 332,53 / 332,53 | 330,74 / 330,74 | 606,02 / 505,31 |
| core: pico / repouso de footprint (MiB) | 293,55 / 293,55 | 293,39 / 293,39 | 490,42 / 490,17 |
| frontend: pico / repouso de footprint (MiB) | 556,81 / 425,22 | 565,35 / 424,63 | 591,83 / 435,11 |

São 24 inserções por servidor/projeto, com reversões verificadas, três sessões
por célula e as mesmas opções da matriz AST. Não houve ganho geral de memória:
no frontend, as faixas do pico se sobrepõem (552,2–562,4 contra 550,6–569,0 MiB),
mas a mediana subiu. O core ficou 4,3% mais lento até estabilizar e usou 3,4%
mais CPU; por sessão, as medianas de API foram 14,15/14,50, 14,10/15,35 e
14,10/14,00 ms (anterior/compacta). Uma repetição previamente definida de três
rodadas somente no core usou os mesmos parâmetros, ambos os binários e o Dart,
alternando a ordem. Ela manteve 48/48 capturas completas e três estados finais
antes/depois, além dos diagnósticos contra o Dart. Até estabilizar, a mediana
foi 14,15/14,25 ms, com CPU de 14,50/14,60 ms. Juntando as seis sessões por
binário, ficaram 14,10/14,40 ms e 14,50/14,80 ms. A primeira piora de 4,3% não
se repetiu nessa magnitude, mas tampouco houve benefício de tempo/CPU no core.
A mediana de footprint caiu só cerca de 0,3 MiB na repetição, com faixas
sobrepostas. Sem ganho geral no processo, a compactação foi retirada; o arquivo
de slots voltou ao hash da versão AST antes do experimento seguinte. A redução
do tamanho da chave não é apresentada como melhoria de memória do servidor.
As duas matrizes e a fonte experimental foram preservadas, sem substituir os
resultados originais: `/tmp/dartforge-named-slots-compact-core-repeat/`,
`/tmp/dartforge-named-slots-compact-core-repeat-verification.{json,log}` e
`/tmp/dartforge-named-slots-compact-slots.rs`.
Artefatos em `/tmp/dartforge-named-slots-compact-matrix/`,
`/tmp/dartforge-named-slots-compact-verification.{json,log}` e manifestos
`/tmp/dartforge-named-slots-{limitless,core,frontend}-compact-inputs-{before,after}.json`.
Ainda falta uma nova validação ampla. O problema do pico permanece aberto.

**Captura restrita ao sufixo, em validação.** As três capturas de identidade
(antes da carga, antes e depois da permutação) percorriam as arenas completas
para cada biblioteca estável, incluindo o prefixo mantido que não pode conter
seus elementos. Elas agora recebem o checkpoint já calculado pelo residente
e começam no limite de cada arena. A enumeração antecede o salto para conservar
os IDs absolutos; bibliotecas com slots descontínuos continuam filtradas por
dono. Não há cache entre recargas nem mudança na representação das chaves.
Passaram 55 testes unitários de parity, 338 do LSP, clippy de todos os alvos e
documentação estrita, com logs em
`/tmp/dartforge-named-slots-suffix-{units,lsp,clippy,doc}.log`. Os controles,
no release SHA-256
`fe0629108b1a9a7c8346de4925da390229737f2f912024c0f158142f83399857`,
mantiveram 450/450 respostas completas iguais às do Dart nas 16 suítes.
As nove sessões de consultas nos projetos reais mantiveram 54/54 respostas
completas e 18/18 mapas de diagnósticos antes/depois, além dos três estados
finais. Não houve pedidos com erro nem diagnósticos esperados ausentes.
Os diagnósticos dos arquivos abertos coincidiram com o Dart ao omitir somente
`source`; permanecem as diferenças de documentSymbol e das publicações globais
do limitless. Artefatos em `/tmp/dartforge-named-slots-suffix-correctness/` e
`/tmp/dartforge-named-slots-suffix-correctness-verification.{json,log}`.
A matriz isolada de 27 sessões manteve 144/144 mapas completos de diagnósticos
de edição/reversão e nove estados finais antes/depois, sem publicações esperadas
ausentes. Os 144 mapas dos arquivos abertos também coincidiram com o Dart,
omitindo somente `source`; as diferenças globais do limitless foram preservadas.
Os três manifestos antes/depois ficaram idênticos, sem raízes ausentes.

| Mediana da captura restrita ao sufixo | Anterior AST | Captura do sufixo | Dart |
| --- | ---: | ---: | ---: |
| limitless: API até diagnóstico / estabilizar / CPU (ms) | 25,05 / 25,05 / 25,45 | 25,15 / 25,15 / 25,55 | 33,00 / 58,50 / 64,00 |
| core: API até diagnóstico / estabilizar / CPU (ms) | 14,20 / 14,20 / 14,55 | 13,95 / 13,95 / 14,35 | 6,05 / 24,70 / 23,70 |
| frontend: API até diagnóstico / estabilizar / CPU (ms) | 23,00 / 23,00 / 23,15 | 22,20 / 22,20 / 22,35 | 20,40 / 42,95 / 47,20 |
| limitless: pico / repouso de footprint (MiB) | 332,28 / 332,28 | 330,14 / 330,14 | 582,61 / 456,50 |
| core: pico / repouso de footprint (MiB) | 292,94 / 292,94 | 292,31 / 292,31 | 498,47 / 497,25 |
| frontend: pico / repouso de footprint (MiB) | 561,03 / 557,97 | 561,05 / 555,50 | 595,55 / 441,95 |

São 24 inserções por servidor/projeto e três sessões por célula, com a mesma
rotação, opções e ausência de consultas da matriz anterior. As reversões foram
verificadas, sem coluna própria de tempo/CPU. No core, a latência de API caiu
1,8% e a CPU 1,4%; no frontend, ambas caíram cerca de 3,5%. A mediana por sessão
também caiu nas três rodadas desses dois projetos. No limitless, houve +0,1 ms
em latência e CPU; não há ganho geral nos três projetos. As edições de corpo
ficaram próximas do baseline: 35,40/35,50, 4,30/4,30 e 12,35/12,40 ms.

A alteração foi mantida por eliminar a varredura de elementos que pertencem ao
prefixo e pelo ganho pequeno e repetido no core/frontend, sem criar cache.
Não se reivindica melhoria geral de memória: o pico do frontend ficou
praticamente igual, com faixas de 558,0–562,3 contra 556,1–566,4 MiB. Seu repouso
variou de 424,4–558,0 para 412,5–556,1 MiB, e a mediana atual continua maior que
a do Dart. O primeiro diagnóstico também continua perdendo para o Dart no core
e frontend. Sessões longas, Windows, parts e identidade geral permanecem abertos.

Artefatos em `/tmp/dartforge-named-slots-suffix-matrix/`,
`/tmp/dartforge-named-slots-suffix-verification.{json,log}` e manifestos
`/tmp/dartforge-named-slots-{limitless,core,frontend}-suffix-inputs-{before,after}.json`.
O snapshot de 1.962 fontes e três binários está em
`/tmp/dartforge-named-slots-suffix-source.json`.

**Nova validação ampla interrompida após uma regressão adicional.** Depois de
encerrar a matriz, foi iniciado um gate sobre essas fontes estáveis. Ele constrói
um CLI `native,jit` novo, preserva-o fora de `target/debug`, usa-o nos dois
seletores dos builders, verifica os pré-requisitos e executa workspace e CLI
com `--no-fail-fast -- --include-ignored`, seguidos de clippy, documentação
estrita, testes Python e formatação. Os hashes das fontes, dos binários de
medição e do compiler preservado são conferidos entre as etapas. Logs em
`/tmp/dartforge-named-slots-suffix-full-gate.log` e
`/tmp/dartforge-named-slots-suffix-full-gate-status.json`; este último só registra
etapas terminadas. O CLI novo e os pré-requisitos passaram; o workspace foi
interrompido com código 143 durante o executor nativo, antes de concluir.
O motivo foi um teste externo adicional que encontrou mensagens de ciclo de
herança diferentes entre o residente e uma análise do zero: inserir
`Added extends B` antes de `B extends Added` produzia `Added, B` no residente
e `B, Added` na análise nova. O reparo mantinha os dependentes, mas a travessia
dos componentes usava a ordem dos IDs da arena, agora independente da fonte.
O repro está em `/tmp/dartforge-named-slots-cycle-order.{rs,log}`; a compilação,
em `/tmp/dartforge-named-slots-cycle-order-build.log`; os processos encerrados
e o motivo foram registrados em
`/tmp/dartforge-named-slots-suffix-full-gate-cancel.json`.
Esse gate não é verde. A regressão no repositório primeiro reprovou, com o
diagnóstico completo registrado em
`/tmp/dartforge-named-slots-cycle-order-red.log`. Ela agora passa após inserções
e reversões de ciclos de duas e três classes e também de funções e variáveis
com erros. A comparação inclui a lista inteira de diagnósticos, códigos,
argumentos, severidade e contextos, sem ordenar ou remover campos; o contador
confirma que os dependentes continuam mantidos.

**Ordem de declarações separada da identidade, em validação.** `ElementOrder`
registra as posições de criação nas faixas permutadas, sem guardar ASTs antigas.
Faixas de identidade são implícitas e a truncagem elimina o sufixo removido;
o armazenamento depende das arenas atuais. Os índices de classes, funções e
variáveis conservam essa ordem de travessia, mesmo após sua reconstrução, e a
busca de componentes de herança usa o índice de classes. A truncagem das listas
também deixou de presumir que o último ID seja o maior. Um teste de composição,
truncagem e reversões repetidas verifica o ciclo de vida desse registro.
Os testes direcionados passaram, incluindo os diagnósticos completos ampliados
em `/tmp/dartforge-named-slots-order-expanded.log`. Os oráculos oficiais locais
3.6.2 e 3.13.5 confirmaram suas respectivas mensagens de ciclo em
`/tmp/dartforge-named-slots-order-oracle/{floor,modern}.stdout.json`; a versão
moderna usa `B, Added`, independentemente dos IDs do residente.
Passaram 619 testes nas suítes de elements, analysis, parity e LSP, sem falhas
ou ignorados, além de clippy de todos os alvos e documentação estrita. O teste
de diagnósticos completos foi então ampliado com edições de corpo, passou
novamente e confirmou pelos contadores a reutilização da AST, sem nova carga.
Logs em `/tmp/dartforge-named-slots-order-{focused,clippy,doc,body-path}.log`.
Um teste externo de ciclo entre typedefs também manteve os diagnósticos inteiros
da análise do zero, em `/tmp/dartforge-named-slots-alias-cycle-order.log`.
O novo release, SHA-256
`c71e0c1c9e1bfe08f56761f118f2b72795822b67e0469aaeeb6d0cd997601f5a`,
manteve 450/450 respostas completas iguais às do Dart nos 16 controles de
símbolos, com logs em `/tmp/dartforge-named-slots-order-controls.log`.
As nove sessões de consultas dos projetos mantiveram 54/54 respostas completas
e 18/18 mapas de diagnósticos antes/depois, além dos três estados finais, sem
pedidos com erro nem publicações esperadas ausentes. Os 18 mapas dos arquivos
abertos também coincidiram com o Dart ao omitir somente `source`; permanecem
as diferenças conhecidas de documentSymbol e dos diagnósticos globais do limitless.
Artefatos em `/tmp/dartforge-named-slots-order-correctness/` e
`/tmp/dartforge-named-slots-order-correctness-verification.{json,log}`.
A matriz isolada contra a versão anterior de captura do sufixo está em execução.
O snapshot
`/tmp/dartforge-named-slots-order-source.json` inclui 1.963 fontes, inclusive
o módulo novo `order.rs`, e os três binários preservados. Os três manifestos
de entrada continuam idênticos aos da matriz anterior, sem raízes ausentes.
Os números de desempenho acima ainda pertencem à versão anterior a este
reparo; o custo atual precisa ser medido antes da nova validação ampla.
O gate amplo antigo de 3.443/117 testes continua restrito à versão anterior
ao reparo AST.

**Comparação LSP dos erros com os dois SDKs.** O ensaio
`/tmp/dartforge-named-slots-order-diagnostics.py` abre os mesmos arquivos nos
servidores anterior, corrigido e oficial, com SDK 3.6.2 nos três da primeira
sessão e SDK 3.13.5 nos três da segunda. São 12 capturas por versão: aquecimento,
ciclos de duas/três classes, inserções de funções/variáveis, edições de corpo e
reversões. A comparação conserva a ordem e todos os campos dos diagnósticos
dos dois arquivos abertos, omitindo somente `source`.

| SDK da sessão | Anterior igual ao Dart | Ordem corrigida igual ao Dart |
| --- | ---: | ---: |
| 3.6.2 | 6/12 | 12/12 |
| 3.13.5 | 6/12 | 12/12 |

O controle negativo reproduziu também as listas de funções e variáveis fora
de ordem, além das mensagens de ciclo. Os 24 resultados inteiros e os textos
de cada edição ficaram em `/tmp/dartforge-named-slots-order-diagnostics/`;
o resumo, em `summary.json`, e o log em
`/tmp/dartforge-named-slots-order-diagnostics.log`.

**Matriz isolada do registro de ordem concluída.** As 27 sessões conservaram
144/144 mapas de edição/reversão e nove estados finais antes/depois. Os 144 mapas
dos arquivos abertos também coincidiram com o Dart, omitindo somente `source`,
sem publicações esperadas ausentes. Os três manifestos antes/depois ficaram
idênticos e sem raízes ausentes. Os parâmetros e a rotação são os mesmos da
matriz anterior: oito inserções por sessão, com reversões verificadas, três
sessões por servidor/projeto e nenhuma consulta intercalada.

| Mediana do registro de ordem | Captura do sufixo | Ordem corrigida | Dart |
| --- | ---: | ---: | ---: |
| limitless: API até diagnóstico / estabilizar / CPU (ms) | 25,10 / 25,10 / 25,45 | 24,80 / 24,80 / 25,15 | 33,30 / 58,20 / 65,10 |
| core: API até diagnóstico / estabilizar / CPU (ms) | 14,30 / 14,30 / 14,60 | 14,10 / 14,10 / 14,45 | 6,20 / 23,85 / 24,60 |
| frontend: API até diagnóstico / estabilizar / CPU (ms) | 22,45 / 22,45 / 22,60 | 22,65 / 22,65 / 22,75 | 21,05 / 42,45 / 47,40 |
| limitless: pico / repouso de footprint (MiB) | 331,88 / 331,88 | 332,13 / 332,13 | 581,38 / 466,30 |
| core: pico / repouso de footprint (MiB) | 291,24 / 286,94 | 293,89 / 293,13 | 501,53 / 497,63 |
| frontend: pico / repouso de footprint (MiB) | 558,25 / 555,77 | 562,64 / 554,97 | 597,55 / 449,14 |

A correção de diagnóstico é necessária, mas seu custo não fica escondido:
no frontend, +0,20 ms até estabilizar e +0,15 ms de CPU; no core, o pico mediano
subiu 2,66 MiB e o repouso 6,19 MiB. As faixas de pico do frontend se sobrepõem
(557,7–558,7 contra 555,0–563,3 MiB), e o repouso segue variável. A edição de
corpo ficou em 35,50/35,65, 4,40/4,40 e 12,40/12,35 ms, respectivamente.
Não há ganho geral de memória nem vantagem no primeiro diagnóstico do core/frontend.
Artefatos em `/tmp/dartforge-named-slots-order-matrix/`,
`/tmp/dartforge-named-slots-order-verification.{json,log}` e
`/tmp/dartforge-named-slots-{limitless,core,frontend}-order-inputs-{before,after}.json`.
A fonte medida foi preservada em `/tmp/dartforge-named-slots-order-source.patch`.

As sessões funcionais com `--api-queries` também expõem um custo aberto:
no frontend, a sessão corrigida levou 131,3 s e amostrou pico total de footprint
de 1.834,6 MiB e RSS de 1.918,6 MiB; o Dart levou 17,8 s, com 740,8 e 856,5 MiB.
A versão anterior ficou próxima da corrigida (132,3 s e 1.828,6 MiB de footprint).
É uma sessão por versão, em ordem fixa e com consultas de conteúdo intercaladas;
esses valores não são as medianas da matriz isolada e não medem estabilidade
de sessão longa. As consultas após API precisam de uma avaliação própria.

A auditoria das nove sessões funcionais da versão de poda confirmou os 18
pedidos esperados por sessão, sem respostas de erro; os JSONs não registram
tempos individuais dessas consultas. Nessa execução, em ordem fixa e uma
sessão por servidor/projeto, a versão atual levou 20,07 / 18,58 / 134,24 s
no limitless/core/frontend, contra 17,11 / 14,98 / 17,89 s do Dart. Os picos
totais amostrados foram 1.465,85 / 1.333,35 / 1.840,77 MiB, contra
731,61 / 626,16 / 754,91 MiB. Esses totais incluem toda a sessão funcional,
com espera inicial e edições; não isolam o custo de cada consulta nem são
as medianas da matriz de edições sem consultas. A auditoria, incluindo hashes,
está em `/tmp/dartforge-named-slots-prune-api-query-audit.json`.

O próximo ensaio precisa registrar tempo e CPU por método depois da edição
e da reversão. Um rascunho externo em
`/tmp/dartforge-api-query-metrics.patch` acrescenta essas medidas ao lado das
respostas completas, fora do intervalo cronometrado de edição; ainda não foi
aplicado nem compilado. A revisão do cache confirmou que uma geração nova
invalida os índices das consultas, mas não demonstrou que isso causa o custo
total observado. Tampouco é seguro presumir que `referencedNames` depende
só do texto: `Context::new` constrói a árvore usando a resolução dos corpos
para distinguir construtores e prefixos. Qualquer reaproveitamento entre
gerações precisa manter essa informação válida, além do texto e dos IDs.

**Poda dos índices sem varrer o prefixo.** Para aceitar listas
em ordem de criação, o primeiro reparo filtrava todos os IDs de todas as listas
mantidas. Agora a ordem fornece um limite inferior para a posição de criação
de qualquer ID retirado. Uma lista ordenada que termina antes desse limite
pode ser mantida sem varredura. A ordem ainda completa é usada para filtrar
os índices; só depois seu registro é truncado. A regressão inclui um ID alto
no meio da lista cujo último ID já está abaixo do corte, caso que o antigo
atalho por ID final não detectaria. Passaram 620 testes nas quatro suítes,
sem falhas ou ignorados, além de clippy de todos os alvos e documentação estrita,
com logs em `/tmp/dartforge-named-slots-prune-{focused,clippy,doc}.log`.
O release de poda, SHA-256
`ee403fe303eae14e731d5e158cd2c7b12be639cfc1aeea9ecd9cac39909ebbbd`,
manteve 450/450 respostas completas nos controles de símbolos. As nove sessões
de consultas também mantiveram 54/54 respostas completas e 18/18 mapas de
diagnósticos antes/depois, além dos três estados finais, sem pedidos com erro
nem publicações esperadas ausentes. Os diagnósticos dos arquivos abertos
coincidiram com o Dart ao omitir somente `source`; permanecem as diferenças
conhecidas de documentSymbol e das publicações globais do limitless.
Artefatos em `/tmp/dartforge-named-slots-prune-correctness/` e
`/tmp/dartforge-named-slots-prune-correctness-verification.{json,log}`.
A matriz contra a versão com registro de ordem foi concluída; resultados abaixo. O
snapshot de 1.963 fontes está em `/tmp/dartforge-named-slots-prune-source.json`;
os manifestos anteriores dos três projetos continuam iguais aos da matriz
anterior.

**Resultado da poda e nova validação ampla.** A matriz de 27 sessões conservou
144/144 mapas de edição/reversão e nove estados finais antes/depois, além dos
144 mapas dos arquivos abertos contra o Dart ao omitir somente `source`.
Não houve publicação esperada ausente, e os três manifestos antes/depois
ficaram idênticos, sem raízes ausentes. A repetição LSP com os SDKs correspondentes
também manteve 12/12 capturas completas em cada versão (3.6.2 e 3.13.5),
com controle negativo de 6/12 em ambas na versão anterior ao registro de ordem.
Os binários desse ensaio estão identificados por SHA-256 em
`/tmp/dartforge-named-slots-prune-diagnostics/binaries.json`.

| Mediana da poda dos índices | Registro de ordem | Poda por limite de criação | Dart |
| --- | ---: | ---: | ---: |
| limitless: API até diagnóstico / estabilizar / CPU (ms) | 25,05 / 25,05 / 25,40 | 25,50 / 25,50 / 25,90 | 33,35 / 59,95 / 66,30 |
| core: API até diagnóstico / estabilizar / CPU (ms) | 14,25 / 14,25 / 14,60 | 14,50 / 14,50 / 14,85 | 6,35 / 24,65 / 25,15 |
| frontend: API até diagnóstico / estabilizar / CPU (ms) | 22,60 / 22,60 / 22,75 | 22,45 / 22,45 / 22,60 | 20,30 / 42,95 / 46,10 |
| limitless: pico / repouso de footprint (MiB) | 332,35 / 332,35 | 329,58 / 329,58 | 575,99 / 457,02 |
| core: pico / repouso de footprint (MiB) | 293,56 / 292,64 | 292,36 / 289,38 | 494,69 / 490,33 |
| frontend: pico / repouso de footprint (MiB) | 559,44 / 554,47 | 559,69 / 424,70 | 585,41 / 443,33 |

O repouso mediano caiu nos três projetos; no frontend, as três sessões novas
ficaram entre 422,0 e 427,8 MiB, enquanto as anteriores variaram entre 419,1 e
559,4 MiB. Essa variação impede tratar a diferença das medianas como redução
garantida em toda sessão. Seu pico ficou próximo do anterior, com faixas
sobrepostas (554,5–560,1 contra 554,5–564,8 MiB).

O custo de tempo/CPU também permanece explícito: no limitless, +0,45/+0,50 ms;
no core, +0,25/+0,25 ms; no frontend, -0,15/-0,15 ms. A edição de corpo ficou
em 35,70/35,60, 4,25/4,30 e 12,30/12,40 ms. Não há ganho uniforme de CPU e
latência; o critério de superar o Dart em todos os custos continua aberto.
A poda foi mantida para evitar a varredura do prefixo e pelos resultados de
repouso, preservando a correção; os pequenos aumentos de CPU do core/limitless
continuam como pendência. Os três projetos ainda ganham do Dart até estabilizar,
mas o primeiro diagnóstico continua perdendo no core e frontend.

Artefatos em `/tmp/dartforge-named-slots-prune-matrix/`,
`/tmp/dartforge-named-slots-prune-verification.{json,log}`,
`/tmp/dartforge-named-slots-prune-diagnostics/` e manifestos
`/tmp/dartforge-named-slots-{limitless,core,frontend}-prune-inputs-{before,after}.json`.

Depois de encerrar todos os ensaios, começou uma nova validação ampla com
fontes estáveis. O script preserva um CLI `native,jit` novo em
`/tmp/dartforge-named-slots-prune-final-native-current`, verifica os pré-requisitos
e executa os dois comandos completos de testes, incluindo ignorados e
`--no-fail-fast`, seguidos de clippy, documentação estrita, testes Python e fmt.
Fontes, binários de medição e compiler preservado são conferidos por hash
entre as etapas. Logs em `/tmp/dartforge-named-slots-prune-final-full-gate.log`
e estado das etapas terminadas em
`/tmp/dartforge-named-slots-prune-final-full-gate-status.json`.
Essa execução terminou com todas as oito etapas aprovadas e fontes estáveis.
Os gates anteriores interrompidos permanecem incompletos e o antigo verde
de 3.443/117 continua restrito à sua versão de fontes.

Na nova execução, o corpus completo de builders pelo executor nativo passou
em 870,64 s, sem falhas ou ignorados, usando o CLI preservado desta versão.
Não havia `DARTFORGE_CASES` nem troca de raiz do corpus no ambiente. Os quatro
testes de transparência Angular também passaram (588,92 s no executável).
Essas durações pertencem à validação funcional, separada das matrizes de
desempenho do LSP.

O workspace completo dessa mesma execução terminou com saída zero em
3.292,65 s: **3.452 testes aprovados, zero falhas e zero ignorados em
226 alvos**, incluindo doctests. A suíte do CLI com `native,jit` também
terminou com saída zero: **117 testes em 16 alvos, sem falhas ou ignorados**,
em 668,17 s. Passaram clippy do workspace com todos os alvos e `-D warnings`,
documentação inclusive de itens privados com `RUSTDOCFLAGS=-D warnings`,
os 11 testes Python do oráculo e fmt. A auditoria final confirmou **1.963
fontes e quatro binários preservados sem alterações**; o gate inteiro levou
4.022,51 s. SHA-256 do JSON final de estado:
`1283640d7eba6820b9562604d598c290da215f49492fad3d8c7e5edbd7c72b4b`.
Esse verde cobre a versão de poda medida acima. Não fecha as pendências de
desempenho, memória, consultas após API, sessão longa ou Windows, nem apaga
os resultados negativos das versões anteriores.

**Auditoria adicional do medidor durante o gate.** Dois testes de CLI
reproduziram uma falha anterior de `editing_compare.py`: um driver que sai
com erro, ou sai com zero sem escrever JSON novo, podia deixar a ferramenta
terminar com zero e reler o JSON de uma execução anterior. As duas reprovações
estão em `/tmp/dartforge-editing-compare-baseline-red.log`.
Os ensaios desta etapa tiveram verificação externa de completude e conteúdo;
uma auditoria adicional conferiu também os códigos de saída do driver nos
logs principais: 171 sessões das sete matrizes, incluindo a repetição do core,
e nove sessões de correção da poda, todas com saída zero e identificadores
únicos. As contagens e hashes dos logs estão em
`/tmp/dartforge-named-slots-driver-exits-audit.json`.

Uma correção isolada em `/tmp/dartforge-editing-compare-draft.py` produz um
arquivo temporário exclusivo por execução, arquiva o JSON/log anterior ao
repetir uma sessão, publica o novo JSON somente após validar a saída e devolve
falha quando o driver falha ou não produz um resultado válido. Passaram seis
testes de CLI com processos reais, sem SDK, em
`/tmp/test_dartforge_editing_compare_draft.py`, com log em
`/tmp/dartforge-editing-compare-draft-tests.log`. Essa versão ainda está fora
do repositório: sua integração fica para depois do gate atual, preservando
a janela de fontes estáveis da implementação Rust.

A preparação da integração ampliou o rascunho em
`/tmp/dartforge-editing-compare-portable/`, ainda fora do repositório.
Nove testes agora chamam o fluxo principal e executam o driver de teste em
processos reais pelo interpretador Python, sem shebang ou permissões de
execução específicas do Unix. As leituras de carga e uptime são substituídas
nos testes; isso não constitui execução nem validação do medidor no Windows.
Dois casos adicionais primeiro reprovaram: falhar ao iniciar o driver
interrompia as sessões restantes, e `--report-only` aceitava um conjunto
incompleto com saída zero. O rascunho registra a falha de lançamento e
continua os demais servidores; o relatório exige todas as rodadas dos
servidores solicitados, conservando a seleção parcial explícita com `--only`.
Os nove casos passaram após o reparo, incluindo o arquivamento dos resultados
anteriores e a preservação de JSON inválido para inspeção. Evidências em
`/tmp/dartforge-editing-compare-portable-{red,green}.log`.
O `--report-only` do rascunho também foi executado sobre os dois conjuntos
reais completos já coletados (nove sessões funcionais e 27 da matriz de
edições). Os relatórios ficaram idênticos, byte a byte, aos da ferramenta
atual, com saída zero e sem stderr; não foram iniciados servidores nem novas
medições. Hashes e contagens em
`/tmp/dartforge-editing-compare-real-reports.json`.

Uma auditoria independente do limite de poda compilou o módulo atual de
ordem, sem alterar o repositório, e enumerou 874 permutações, suas composições
com uma permutação de ordem reversa e 4.796.368 combinações de subconjuntos/listas e cortes.
Inclui IDs de identidade antes/depois da faixa permutada. Em todos os casos,
uma lista preservada sem varredura não continha ID retirado pelo corte.
Fonte e resultado em `/tmp/dartforge-order-pruning-exhaustive.{rs,log}`.
Esses números contam combinações da auditoria, não testes adicionais do workspace.

A versão anterior usada no primeiro ensaio está em `/tmp/dartforge-named-slots-after`,
SHA-256 `f8be4deb146e85ac39c651e587afe2b2440061e2928b3546e47a2dbc8a6869bd`.
Ela não contém o reparo global. O binário de debug com esse reparo está em
`/tmp/dartforge-named-slots-global-debug` e não serve à comparação de tempos.
Seu rastro está em `/tmp/dartforge-named-slots-global-trace.{json,log}`;
a comparação ampliada, em `/tmp/dartforge-named-slots-requests-{before,after,dart}.{json,log}`.
Artefatos anteriores em
`/tmp/dartforge-named-slots-correctness/frontend.{before,after,dart}.0.{json,log}`,
`/tmp/dartforge-named-slots-trace-{before,after}.{json,log}` e
`/tmp/dartforge-named-slots-debug-trace{,2}.{json,log}`.
Os caminhos e pontos de inserção dos três projetos foram registrados em
`/tmp/dartforge-named-slots-projects.json`; os manifestos anteriores à medição
conservam 9.761 / 6.640 / 10.894 arquivos e nenhuma raiz ausente.
Esta etapa ainda está sem commit. A medição isolada acima não fecha a meta geral.

## Capacidades

`textDocument/documentSymbol` lista declarações de topo e membros a partir da
AST do documento aberto. Usa `DocumentSymbol[]` hierárquico quando o cliente
anuncia `hierarchicalDocumentSymbolSupport`; caso contrário devolve
`SymbolInformation[]` plano, como exige o contrato do LSP. Os intervalos são
UTF-16. A árvore é descartada depois de cada pedido, mantendo o platô de
memória por edição. Edições `didChange` com versão antiga são ignoradas sem
republicar diagnósticos. Testes direcionados: `cargo test -p dartforge-lsp
--test simbolos --locked`.

`workspace/symbol` busca nos documentos abertos **e** nos arquivos `.dart`
dos projetos do workspace: as raízes do `initialize` (`workspaceFolders`,
senão `rootUri`/`rootPath`) e o projeto (diretório com `pubspec.yaml`) de
cada documento aberto, sem diretórios ocultos ou `build`. Os arquivos de pacotes
aninhados participam conforme as fronteiras dos contextos descritas acima. Com
`onlyAnalyzeProjectsWithOpenFiles`, as pastas anunciadas não fixam raízes permanentes;
os contextos acompanham os projetos abertos. O texto do editor vale mais que o disco.
As árvores do parser saem depois de extrair as declarações; o índice sintático conserva
essas declarações com suas impressões digitais e invalida o que mudou.

O casamento usa `crates/lsp/src/fuzzy_matcher.rs` (`FuzzyMatcher`, estilo `Text`), sem
reordenar os resultados pela pontuação. O `SymbolInformation[]` mantém a ordem de
descoberta e das declarações, para em 500 e usa o intervalo de código do elemento.
Consulta vazia retorna lista vazia. A paridade desses campos e da ordem continua sujeita
aos limites de evidência da seção de 04/11, especialmente a variação entre caches do
Dart e as divergências ainda abertas nos projetos grandes.

### Definição, hover e referências pela identidade da declaração

Com SDK, `textDocument/definition`, `textDocument/hover` e
`textDocument/references` saem do mesmo modelo do renomear
(`crates/lsp/src/projeto.rs`): o que a posição denota vem das tabelas da
inferência comum (`get_resolved`, `declaracao_local`, `tipos_de_locais`) e
dos namespaces de `crates/elements` (`lookup`, `lookup_prefixed`), nunca de
uma resolução paralela por nome. Formas cobertas (matriz em
`tests/navegacao_semantica.rs`):

| Forma | Definição | Hover |
| --- | --- | --- |
| local, parâmetro (promovido) | a declaração | `num n` + `Type: int` (tipo da referência, com promoção) |
| função local | a declaração | `int dobro(int v)` |
| membro por instância, `this` implícito, `super`, cascata | o membro resolvido para o receptor (a sobrescrita, não a família) | assinatura completa (`void met(int x, [int y = 0])`; com três parâmetros ou mais, um por linha, como o `multiline` do analyzer); numa chamada, `Type:` com o tipo da invocação (`void Function(int, [int])`); campo e getter com `Type:` já substituído (`T valor` + `Type: int`) |
| construtor nomeado (`A.nome()`, `new A.nome()`) e sem nome escrito (`A()`) | o construtor; sem construtor escrito, a classe | `Caixa<T> Caixa.vazia(T v)` |
| classe, mixin, enum, extension type, extensão, typedef | a declaração | `abstract class B<T> extends A with M implements I` (modificadores, parâmetros de tipo, supertipos como no `ElementDisplayStringBuilder`) |
| variável, função, getter de topo; importados com ou sem prefixo | a declaração (outro arquivo, aberto ou no disco, ou o SDK) | `int x` + `Type: int`; `int soma([int a = 0])`; `int get g` + `Type: int` |
| constante de enum, membro de extensão | a declaração | `Cor azul` + `Type: Cor` |
| prefixo de import | o `as p` da diretiva | — (o Dart também não mostra) |
| parâmetro de tipo | o parâmetro mais interno com o nome | `T extends Bound` |
| referência `[nome]`, `[A.b]` num comentário de documentação | resolvida no escopo da declaração documentada (parâmetros, parâmetros de tipo, membros da classe, biblioteca) | como a declaração |

O hover inclui a documentação (`///` ou `/** */`, limpa, depois de `---` em
Markdown); um membro sobrescrito sem documentação própria mostra a do
primeiro membro sobrescrito que a tem, como o analyzer. Em Markdown, o
texto segue o `toHover` do Dart 3.6.2 (`lsp/handlers/handler_hover.dart`):
o bloco `dart` com a descrição (`(deprecated) ` antes, se a declaração tem
`@deprecated`/`@Deprecated`; `(new) ` antes do construtor de uma criação
sem `new`/`const`), `Type: \`T\``, a biblioteca do elemento não local em
itálico (`*package:x/y.dart*`, `*dart:core*`, ou o caminho relativo à raiz
para um arquivo fora de `lib/`, como o `_libraryInfo` do analyzer) e, depois
de `---`, a documentação. Um parâmetro mostra os delimitadores do tipo dele
(`[int b = 0]`, `{required int c}`); a declaração de uma constante de enum
não tem hover; os nomes de `show`/`hide` resolvem para o elemento exportado
(hover, definição, referências e renomear). Em texto puro, sem a linha da
biblioteca. Partes entram pela
biblioteca dona e a definição cruza parte ↔ dona; o texto aberto vale mais
que o disco (inclusive para o dono da declaração). Definição e hover
carregam só a biblioteca do documento (e o que ela importa) e inferem só os
corpos dela; o literal de URI de diretiva continua pela regra sintática
(arquivo existente). Prefixo que colide com declaração de topo
(`prefix_collides_with_top_level_member`) não é resolvido. Sem SDK, ou se a
posição não resolve, valem as respostas sintáticas conservadoras descritas
acima. Testes: `cargo test -p dartforge-lsp --test navegacao_semantica
--test semantica --locked`.

`textDocument/references` carrega o projeto inteiro (como o renomear) e
devolve a declaração (se `context.includeDeclaration`) e os usos do mesmo
elemento em **todas** as bibliotecas do projeto, abertas ou só no disco,
ordenados por (URI, offset); intervalos de arquivo fechado vêm do texto do
disco. A identidade é a da declaração: homônimos de outras bibliotecas,
locais que sombreiam, e parâmetros ou membros de mesmo nome não entram; um
membro de instância abrange a família ligada por sobrescrita (como o
`getHierarchyMembers` do analyzer), e as declarações dos outros membros da
família não contam como usos. Entram também `show`/`hide`, metadados,
rótulos `nome:` de parâmetros nomeados e referências `[nome]` de
documentação. Política de dependências: um elemento do SDK ou de pacote tem
a declaração devolvida (no arquivo do SDK/pacote), mas os usos só são
procurados no projeto. Sem SDK, vale a regra conservadora dos documentos
abertos descrita acima.

### Consultas semânticas por requisição (completar, renomear, ações)

As três capacidades abaixo leem tipos e resoluções da **inferência comum**
de `crates/types` (`BodyInferrer`, tabelas `BodyTypes`: `get_type`,
`get_resolved`), nunca de uma inferência paralela no LSP. Cada requisição
usa uma `Consulta` (`crates/lsp/src/consulta.rs`): programa carregado com os
textos vigentes dos documentos abertos (os demais arquivos vêm do disco),
outline, tabela de tipos e corpos inferidos só das bibliotecas necessárias.
A do completar (texto com sentinela) é descartada ao responder; a de
definição, hover, referências, renomear e ações fica na sessão limitada
descrita abaixo. Um arquivo `part of` entra
pela biblioteca dona (URI escrita, ou o arquivo do projeto que declara o
`part` na forma `part of nome;`), o que vale também para `definition` e
`hover` semânticos; parte que a dona não declara entra sozinha. Dois ganchos da inferência,
desligados no compilador, existem para o LSP: `sonda_escopo` captura o
escopo léxico (locais, parâmetros, parâmetros de tipo, classe/extensão
envolvente, `this`) no identificador pedido, e `registrar_locais` guarda,
para cada expressão que lê ou escreve um local, o offset da declaração.
Sem SDK, completar responde `null`, renomear recusa e as ações ficam só nas
sintáticas. Custo medido (binário release, projeto pequeno com o SDK 3.6.2
real, processo novo por medida): completar ~70–100 ms, renomear ~70–100 ms,
ação de importar ~135 ms na primeira vez (inclui o índice de nomes do SDK);
o tempo é dominado pela carga das bibliotecas `dart:`.

### Sessão semântica limitada

`crates/lsp/src/session.rs` guarda **um** programa carregado (com outline,
tipos e corpos) e o reaproveita nas consultas seguintes enquanto nada do
que ele leu mudou: `hover`, `definition`, `prepareRename`, a identificação do
nome de `references`/`implementation` e as ações (escopo da biblioteca),
`rename` (projeto inteiro, que também serve a uma consulta de biblioteca do
mesmo projeto). Desde 2026-10-31 a carga de uma biblioteca lê as bibliotecas
**fora do projeto** (SDK e pacotes) sem os corpos das declarações
(`parse_lexed_outline`, `load_lenient_outline`; ver "DartForge × servidor
oficial" abaixo). A política de memória é explícita:

* no máximo uma entrada (não há cache por arquivo nem por versão);
* `didOpen`, `didChange` e `didClose` descartam a entrada na hora (o `drop`
  acontece no despacho da notificação) — nenhuma árvore de versão velha
  fica presa;
* antes de reaproveitar, a chave é conferida: versão e tamanho de cada
  documento aberto, data e tamanho de cada arquivo lido do disco (fora o SDK
  e o *pub cache*, que nada reescreve no lugar e o servidor do Dart também
  não vigia) e a lista de `.dart` do projeto (arquivo novo pode satisfazer um
  import); disco alterado por outra ferramenta recarrega;
* só retém programa cuja fonte somada cabe no orçamento
  `DARTFORGE_LSP_SESSION_MIB` (MiB de fonte; padrão 32 desde 2026-10-31, era
  8; `0` desliga). Acima dele, o programa vale só para a consulta e cai com a
  resposta.

Medida (2026-09-27, Linux, release, SDK 3.6.2 real; `cargo run --release -p
dartforge-lsp --example sessao_semantica -- <arquivo> 20`: 20 posições de
identificador com `hover` + `definition`, 3 `references`, uma edição e a
primeira consulta depois dela; latência e memória viva do alocador
contador, juntas):

| Arquivo (pub-cache) | Modo | hover (mediana; máx) | definition | references | 1ª após edição | fonte retida | vivo retido | vivo após edição | cargas / reusos |
|---|---|---|---|---|---|---|---|---|---|
| `args-2.7.0/lib/src/arg_parser.dart` (15 KB) | sem sessão | 81,6; 102,6 ms | 72,6 ms | 73,0 ms | 124,1 ms | 0 | 0 | 0 | 44 / 0 |
| | com sessão | 0,3; 97,9 ms | 0,5 ms | 0,4 ms | 82,2 ms | 2,5 MiB | 21,1 MiB | 0,0 MiB | 3 / 41 |
| `collection-1.19.1/lib/src/iterable_extensions.dart` (32 KB) | sem sessão | 58,1; 80,3 ms | 57,5 ms | 120,1 ms | 69,2 ms | 0 | 0 | 0 | 44 / 0 |
| | com sessão | 0,9; 87,8 ms | 1,5 ms | 0,7 ms | 112,0 ms | 2,7 MiB | 25,9 MiB | 0,0 MiB | 3 / 41 |
| `analyzer-7.7.1/lib/src/dart/element/display_string_builder.dart` (18 KB; projeto de 1.607 arquivos), 10 posições | sem sessão | 59,9; 103,8 ms | 58,6 ms | 1.358,7 ms | 71,4 ms | 0 | 0 | 0 | 24 / 0 |
| | com sessão | 6,8; 84,8 ms | 7,6 ms | 1.174,8 ms | 70,0 ms | 0 (projeto acima do orçamento) | 19,4 MiB | 0,0 MiB | 5 / 19 |

No `analyzer` (projeto grande), a biblioteca cabe no orçamento e é
reaproveitada (a mediana de ~7 ms é quase toda a conferência da chave: a
lista de 1.607 arquivos e as datas dos lidos); o projeto inteiro passa de 8
MiB de fonte, então cada `references` recarrega (~1,2 s) e nada dele fica
retido — o orçamento funcionando como limite, não como cache.

Leitura: a sessão troca ~60–120 ms por consulta por ~1 ms enquanto o texto
não muda, ao custo de ~8–10× a fonte carregada em memória viva **enquanto**
a versão vale (o orçamento padrão de 8 MiB de fonte limita isso a algumas
dezenas de MiB); a primeira consulta depois de uma edição paga a carga
inteira de novo, e o vivo volta à base (0,0 MiB) a cada edição. Os três
"máximos" altos são as três cargas (biblioteca, projeto, após a edição).
Testes: `cargo test -p dartforge-lsp --test sessao --test sessao_memoria
--locked` (reuso, invalidação por edição, por arquivo alterado e por
arquivo novo, orçamento zero; retenção e volta à base em 20 edições).

`textDocument/completion` (`triggerCharacters: ["."]`). O ponto de
digitação quase nunca analisa (`a.` sem nome, `a.ca` sem `;`), e o parser
recupera por comando: o comando incompleto some da árvore. Antes da
análise, o nome sob o cursor vira um identificador sentinela e, se preciso,
recebe um fecho curto (`;`, `)`, `);`, `));`, `]`, `});` …); vale a
variante que põe o sentinela numa expressão com o menor número de
diagnósticos. Então:

* `alvo.▮` — membros de instância pelo tipo estático do alvo
  (`get_type`), pela busca de membros de `crates/types`
  (`MemberResolver::lookup_member`), com genéricos substituídos
  (`List<String>.first` mostra `String`), herdados, de mixins, interfaces
  e de extensões aplicáveis; parâmetro de tipo usa o limite; `dynamic` usa
  `Object`; registros mostram `$1`… e os nomeados. Nome de classe
  (`A.▮`): estáticos e construtores nomeados. Prefixo de import
  (`p.▮`): o espaço de nomes do prefixo.
* nome simples — locais e parâmetros visíveis (os declarados adiante e os
  de blocos fechados não aparecem), parâmetros de tipo, membros da classe
  envolvente (de instância só fora de contexto estático), declarações de
  topo, importados sem prefixo, os prefixos e palavras-chave pelo contexto
  (comando, expressão, membro de classe, topo). Numa lista de argumentos,
  os parâmetros nomeados ainda não passados (`nome: `) vêm primeiro.
* posição de tipo (`Str▮ x`, `List<▮>`, `void f(▮ a)`, `p.▮ x`) — só
  tipos: classes, mixins, enums, extension types e typedefs do escopo (ou
  do prefixo), parâmetros de tipo em escopo, prefixos, `dynamic`/`void`, e
  as palavras-chave de declaração quando o tipo abre uma declaração ou um
  comando; nunca valores (locais, variáveis, funções, `null`…);
* nomes públicos de bibliotecas **ainda não importadas** que casam com o
  digitado (não com a lista vazia): do SDK (índice das bibliotecas públicas,
  `dart:core` fora) e do projeto (índice incremental por arquivo em
  `crates/lsp/src/indice.rs`, um projeto por vez, só nomes e offsets), com
  `detail` `Auto import from 'dart:math'` e `additionalTextEdits` com a
  diretiva no lugar certo (a mesma regra da ação de importar: `dart:`,
  `package:`, relativas; relativo ou `package:` conforme `lib/`). Em posição
  de tipo, só tipos. Numa parte não há (o `import` iria para a dona). Acima
  de 200 desses itens, a lista sai com `isIncomplete: true`;
* privado de outra biblioteca nunca aparece; nada em comentário, texto de
  string ou número (interpolação é código).

Os itens seguem o formato do servidor do Dart: `label` `met(…)`/`met()`
para funções, `detail` `(int x, {String? nome}) → void` ou o tipo, `kind`
(2 método, 3 função, 4 construtor, 5 campo, 6 variável, 7 classe, 9
prefixo, 10 getter, 13 enum, 14 palavra-chave, 20 constante de enum, 25
parâmetro de tipo), `textEdit` sobre o prefixo digitado. O filtro é aproximado
(`crates/lsp/src/aproximado.rs`): prefixo, contém, iniciais de palavras
(`vt` acha `valorTotal`) e subsequência que abre como o nome. A ordem é por
relevância e estável, e `sortText` a repete: o que começa com o digitado
(sem diferenciar maiúsculas) antes do que só casa por aproximação; depois o
grupo (nomeados, locais, membros — os herdados de `Object` por último —,
biblioteca, importados, prefixos, não importados, palavras-chave) e o nome.

`completionItem/resolve` (`resolveProvider: true`): cada item com declaração
conhecida leva `data` (`arquivo`, `inicio`); o resolve lê o comentário de
documentação da declaração (texto aberto ou disco; `///` ou `/** */`) e o
devolve em `documentation`, em Markdown se o cliente anuncia
`documentationFormat: ["markdown"]`. Sem `data`, o item volta como veio.

Snippets: quando o cliente anuncia `completionItem.snippetSupport` (e não
passa `completeFunctionCalls: false` nas `initializationOptions`), funções,
métodos e construtores entram com os parênteses e os parâmetros
obrigatórios como marcadores (`met(${1:x})$0`, nomeados obrigatórios como
`req(n: ${1:n})$0`, sem parâmetros `nada()$0`, parâmetros desconhecidos de
um nome não importado `f($0)`), com `insertTextFormat: 2`; se já há `(`
depois do nome, só o nome. Campos, getters e variáveis nunca levam
parênteses. Teste: `cargo test -p dartforge-lsp --test completar
--locked`.

`textDocument/prepareRename` e `textDocument/rename` (`renameProvider:
{prepareProvider: true}` quando o cliente anuncia `prepareSupport`). O
projeto é o diretório mais próximo com `pubspec.yaml` (sem ele, o do
arquivo); todos os `.dart` dele (sem ocultos, `build` nem subpacotes), menos
as partes, entram como bibliotecas de entrada de uma só carga
(`load_lenient_entradas` em `crates/elements`), para que as bibliotecas que
*importam* a declaração também sejam vistas; os corpos do projeto são
inferidos com `registrar_locais`. O que o cursor denota e as ocorrências vêm
do modelo comum (`crates/lsp/src/projeto.rs`, o mesmo das referências):

* local, parâmetro ou função local — a declaração e as expressões que a
  referem (inclusive em interpolação e como alvo de atribuição); num
  parâmetro nomeado, os rótulos `nome:` nas chamadas e, num método de
  instância, o parâmetro homônimo de todas as sobrescritas (com os usos e
  rótulos delas);
* membro de classe — a família ligada por sobrescrita na hierarquia (sobe e
  desce por `extends`, `with`, `implements` e `on` até fechar), getter e
  setter juntos, cada uso resolvido para um membro da família (por
  instância, `this` implícito, `super`, cascata), parâmetros `this.x`,
  inicializadores `x = e` e os rótulos dos `this.x` nomeados; membro de
  extensão e estático são só os do dono; constantes de enum inclusas;
* declaração de topo — a declaração (getter e setter homônimos juntos), os
  usos resolvidos com ou sem prefixo, as anotações de tipo (resolvidas pelo
  escopo da biblioteca, respeitando parâmetros de tipo homônimos), os
  construtores escritos com o nome da classe, `show`/`hide` e metadados
  `@nome`;
* construtor nomeado — a declaração, `A.nome()`, `new A.nome()`,
  `this.nome()`, `super.nome()` (pela superclasse), `factory … = A.nome`,
  constantes de enum `a.nome()` e metadados `@A.nome()`;
* prefixo de import — o `as p` de cada import com ele na biblioteca e cada
  `p.` escrito nela (expressões, tipos, metadados);
* parâmetro de tipo — a declaração e os usos cujo parâmetro mais interno
  com o nome é ele (um `<T>` de método sombreia o da classe);
* em todos os casos, as referências `[nome]`/`[A.nome]` dos comentários de
  documentação que resolvem para o elemento.

Com `renameFilesWithClasses: "always"` nas `initializationOptions` e o
cliente aceitando `documentChanges` com a operação `rename`, renomear uma
classe cujo arquivo segue o nome dela (`MinhaClasse` em `minha_classe.dart`)
renomeia o arquivo (operação depois das edições de texto) e corrige as
diretivas `import`/`export`/`part`/`part of` do projeto que o citam
(relativas ou `package:`); destino existente deixa o arquivo como está.

Recusa com o código `-32010` (o `RenameNotValid` do servidor do Dart):
identificador malformado, palavra reservada, identificador embutido como
nome de tipo ou prefixo, elemento do SDK ou de pacote fora da raiz
(inclusive a sobrescrita de um membro deles, como `toString`), e conflitos
por escopo léxico (bloco, `for`, `catch`, função; o bloco do corpo é o
escopo dos parâmetros): local duplicado no mesmo escopo, referência que
ficaria no escopo de outro local aninhado com o nome novo, uso de `novo`
(identificador ou tipo) no escopo que passaria a denotar o local, uso solto
de um topo ou membro que cairia no escopo de um local `novo`, uso de topo
dentro de uma classe com membro `novo`, `novo` solto no corpo da família que
passaria a denotar o membro, membro já existente na família, nome já
declarado na biblioteca (ou numa que usa o elemento), construtor ou estático
homônimo, prefixo que colidiria com nome visível, parâmetro de tipo irmão
ou tipo usado no escopo, e nome público que viraria privado com usos em
outra biblioteca. `prepareRename` em espaço, palavra-chave ou literal
devolve `null`. As edições saem como `WorkspaceEdit.documentChanges` (cada
documento com a versão vigente; fechado, `version: null`) quando o cliente
anuncia `workspace.workspaceEdit.documentChanges`, senão como `changes`,
convertidas com as linhas do texto aberto ou, para arquivo fechado, do
arquivo no disco. Uso dentro de um comando que não analisa não é visto (a
recuperação do parser o descarta). Teste: `cargo test -p dartforge-lsp
--test renomear --locked`.

`textDocument/codeAction` (`codeActionKinds: ["quickfix", "refactor"]`,
honra `context.only`). As correções respondem aos diagnósticos **publicados**
da versão vigente: os imediatos (recalculados no pedido) e os tipados que o
fluxo contínuo publicou para essa versão — o servidor guarda a última
publicação tipada de cada documento aberto (substituída a cada publicação,
removida no `didClose`) e não a usa se a versão mudou. As edições saem em
`documentChanges` com a versão do documento quando o cliente anuncia
`workspace.workspaceEdit.documentChanges`, senão em `changes`; cada ação de
correção leva o diagnóstico que corrige.

* `Insert ';'` (`quickfix.insertSemicolon`) para o `expected_token`
  "Expected to find ';'." publicado, inserindo no fim do intervalo do
  diagnóstico, com o diagnóstico na ação — como o `dart.fix.insertSemicolon`.
* `Import library '…'` para um nome indefinido no intervalo: identificador
  sem resolução na inferência e fora do escopo da biblioteca, ou nome de
  tipo não encontrado (sem parâmetro de tipo homônimo). Candidatas: as
  bibliotecas públicas do SDK que o declaram (`quickfix.import.librarySdk`;
  índice de nomes de topo das bibliotecas sem `_` e das suas partes, montado
  na primeira vez e de tamanho fixo) e as do projeto
  (`quickfix.import.libraryProject1`): import relativo, ou `package:` quando
  o arquivo está fora de `lib/` e o alvo dentro. A diretiva entra na ordem
  (`dart:`, `package:`, relativas), depois de `library`, ou no topo.

* Correções dos códigos semânticos publicados, com os títulos e espécies do
  servidor do Dart 3.6.2 (títulos e ids conferidos no
  `analysis_server.dart.snapshot` do SDK):

  | Código publicado | Ação | Espécie |
  | --- | --- | --- |
  | `unused_local_variable` | `Remove unused local variable` (a declaração, ou só a variável numa lista, e os comandos que só atribuem a ela) | `quickfix.remove.unusedLocalVariable` |
  | `unused_element` (função local, declaração de topo, membro de classe) | `Remove unused element` (as linhas da declaração, com o `///` de cima) | `quickfix.remove.unusedElement` |
  | `unnecessary_cast` | `Remove unnecessary cast` (e o parêntese que sobraria em volta de uma primária) | `quickfix.remove.unnecessaryCast` |
  | `unnecessary_non_null_assertion` | `Remove the '!'` | `quickfix.remove.nonNullAssertion` |
  | `invalid_null_aware_operator` | `Replace with '.'` / `Replace with '['` | `quickfix.replace.withNotNullAware` |
  | `instance_access_to_static_member` | `Change access to static using 'C'` (`p.C` se a classe é vista por prefixo) | `quickfix.change.toStaticAccess` |
  | `record_literal_one_positional_no_trailing_comma` | `Add trailing comma` | `quickfix.add.trailingComma` |
  | `assignment_to_final` (campo) | `Make field 'x' not final` (tira o `final` da declaração, ou troca por `var` sem tipo, no arquivo dela) | `quickfix.makeFieldNotFinal` |
  | `abstract_field_initializer` | `Remove initializer` e `Remove the 'abstract' keyword` | `quickfix.remove.initializer`, `quickfix.remove.abstract` |
  | `non_bool_condition` | `Add != null` (como o `AddNeNull` do Dart, depois da condição) | `quickfix.add.neNull` |
  | `uri_does_not_exist` (URI relativa `.dart`) | `Create file 'x.dart'`: operação `create` no `WorkspaceEdit`, vazio ou com o `part of` para uma parte; só ao cliente que anuncia `documentChanges` e `resourceOperations: create` | `quickfix.create.file` |

  Cada edição é conferida contra a árvore do texto vigente: diagnóstico que
  não corresponde ao nó esperado não gera ação.
* Assistência `Add type annotation` (`refactor.add.typeAnnotation`) num
  local `var x = e;` ou `final x = e;` (cursor na palavra-chave ou no nome),
  com o tipo da inferência comum; não quando o tipo é `dynamic` nem quando
  algum nome do tipo não é visível na biblioteca (a anotação não
  compilaria).

Dos códigos semânticos publicados (`crates/analysis/verified.txt`), os
três de enum (`enum_constant_same_name_as_enclosing`, `enum_with_name_values`,
`values_declaration_in_enum`) não têm correção rápida no `dart
language-server` 3.6.2 (conferido), e este servidor também não. Teste:
`cargo test -p dartforge-lsp --test acoes --locked` (inclui os negativos:
fora do intervalo, variável usada, publicação tipada de versão velha,
`dynamic`, cursor no inicializador, `context.only`).

Implementadas: `initialize` (com `serverInfo`), `initialized`, `shutdown`,
`exit` (0 após `shutdown`, 1 sem), `$/cancelRequest`,
`textDocument/didOpen`/`didChange` (incremental e integral)/`didClose`,
`textDocument/publishDiagnostics` (`severity`, `source: "dartforge"`,
`version`), `textDocument/documentSymbol`, `workspace/symbol`,
`textDocument/definition`, `textDocument/references`, `textDocument/hover`,
`textDocument/completion`, `textDocument/prepareRename`,
`textDocument/rename`, `textDocument/codeAction`, `completionItem/resolve`,
`textDocument/diagnostic` (só com o cliente que anuncia
`textDocument.diagnostic`), `textDocument/signatureHelp`,
`textDocument/documentHighlight`, `textDocument/implementation`,
`textDocument/typeDefinition`, `textDocument/foldingRange`,
`textDocument/selectionRange`, `textDocument/inlayHint`,
`textDocument/semanticTokens/full` e `/range`,
`textDocument/prepareTypeHierarchy`, `typeHierarchy/supertypes`,
`typeHierarchy/subtypes`, `textDocument/prepareCallHierarchy`,
`callHierarchy/incomingCalls`, `callHierarchy/outgoingCalls` (regras em
"Paridade com o servidor do Dart (3.6.2)"), `dartforge/dormir`
(gancho de teste do cancelamento em execução; clientes reais nunca enviam).

Pendentes nas ações: das assistências do Dart (`refactor.*`: extrair
método ou variável, embutir, converter corpo, mover para arquivo…), só a
anotação de tipo de local existe; das ações de fonte, só `Organize Imports`
(sem remover imports não usados: `unused_import` não é publicado ainda);
`Sort Members` e `Fix All` não existem.

**Diagnósticos puxados (LSP 3.17).** O cliente que anuncia
`textDocument.diagnostic` recebe o `diagnosticProvider`
(`interFileDependencies: true`, `workspaceDiagnostics: false`) e o servidor
deixa de empurrar `publishDiagnostics` para ele. `textDocument/diagnostic`
devolve o resultado tipado da versão vigente, senão o imediato; o
`resultId` é a versão (`"3"`, ou `"3.t"` quando já é o tipado), e um
`previousResultId` igual volta `unchanged` sem recalcular. Quando chega um
resultado tipado, o servidor pede `workspace/diagnostic/refresh` (se o
cliente anunciou `workspace.diagnostics.refreshSupport`) e o cliente puxa de
novo; a resposta do cliente é aceita em silêncio. Documento fechado não tem
diagnóstico. Cliente sem a capacidade continua no fluxo empurrado.
Teste: `tipados::diagnosticos_puxados_no_lugar_dos_empurrados`.

**Formatação: decisão (L08).** `documentFormattingProvider` não é
anunciado. O formatador de referência é o `dart_style` (`dart format`), com
regras que mudam por versão de linguagem ("tall style" a partir da 3.7); um
formatador que não seja byte a byte o do SDK reescreveria arquivos de forma
diferente do `dart format` do projeto e do CI dele. A capacidade só entra com
um porte do `dart_style` conferido contra ele num oráculo (como os demais
componentes); até lá, a formatação fica com o `dart format`. Os
diagnósticos semânticos publicados (os códigos de `verified.txt`,
inclusive os que dependem de tipos) chegam pelo fluxo tipado descrito em
"Diagnósticos tipados"; os não verificados continuam fora do editor.

## Paridade com o servidor do Dart (3.6.2)

O alvo é o `dart language-server` do SDK 3.6.2 (o mesmo do oráculo dos
diagnósticos). As regras abaixo saem da fonte do `analysis_server` na tag
`3.6.2` do checkout de referência (`git show 3.6.2:pkg/analysis_server/…`,
caminhos relativos a `pkg/analysis_server/lib/src/`) e foram conferidas no
oráculo; onde o servidor atual (pós-3.6) mudou o comportamento e a mudança
é melhor para o editor, a escolha está dita.

### Oráculo de paridade

`crates/lsp/oracle/oracle.py` abre os mesmos arquivos nos dois servidores
por stdio (capacidades de cliente como as do VS Code: Markdown, snippets,
`documentChanges`, `prepareSupport`, `lineFoldingOnly`,
`hierarchicalDocumentSymbolSupport`), espera a análise inicial do Dart e pede,
em posições amostradas de identificadores (fora de comentários e strings, 12
por arquivo, espalhadas): `hover`, `definition`, `typeDefinition`,
`implementation`, `references`, `documentHighlight`, `prepareRename`,
`rename`, `codeAction` (assistências no cursor), `selectionRange`,
`prepareCallHierarchy`, `prepareTypeHierarchy` e `completion` (no meio do
nome, com até dois caracteres digitados, ou logo depois do `.` de um acesso a
membro); `signatureHelp` logo depois do `(` de chamadas (disparo automático)
e depois da primeira vírgula (invocado); por arquivo, `documentSymbol`,
`foldingRange`, `semanticTokens/full`, `inlayHint`, `documentLink` e
`formatting`; e `codeAction` no intervalo de cada diagnóstico que o Dart
publica. `crates/lsp/oracle/summary.py` normaliza (URIs sem
percent-encoding nem caixa, intervalos, conjuntos) e imprime a tabela.

Uso (o binário do DartForge com `DARTFORGE_SDK_LIB` apontando o SDK 3.6.2):

```text
python crates/lsp/oracle/oracle.py saida.jsonl target/release/dartforge-lsp <projeto> …
python crates/lsp/oracle/summary.py saida.jsonl [--examples N] [--method M]
```

O SDK do Dart 3.6.2 vem de `ORACLE_DART` (o executável) e `ORACLE_SDK_LIB` (a pasta `lib`); sem elas,
os caminhos do Windows (`C:\tools\dartsdk-3.6.2`). `ORACLE_SNIPPETS=0` tira do cliente o
`experimental.snippetTextEdit` (ligado por padrão, como no Dart-Code) e `ORACLE_METHODS=codeAction,references`
pede só esses métodos (atenção: o estado do servidor do Dart depende dos pedidos anteriores — o conjunto
de arquivos que ele conhece cresce com eles —, então uma rodada parcial pode divergir da completa). Num sistema de arquivos que não distingue caixa, passe os projetos com a caixa do disco.
O `resumo.py` compara também a edição de cada ação de título comum (`TextEdit`/`SnippetTextEdit`).

Projetos medidos: cópias de `args-2.7.0`, `path-1.9.1` e
`string_scanner-1.4.1` do pub-cache (só `lib/`, `dev_dependencies` fora,
`dart pub get --offline`) e um projeto `app` com erros comuns (método,
getter e função indefinidos, argumento obrigatório faltando, `switch` não
exaustivo, membros abstratos sem implementação, variável não usada, cast
desnecessário).

Resultado (4 projetos, 36 arquivos, 410 posições; "igual" é o resultado
normalizado idêntico ao do Dart; antes = o servidor no início deste
trabalho, depois = o atual):

| Recurso | Antes | Depois | Observação |
|---|---|---|---|
| documentSymbol | 53% | 97% | espécies e nomes (`A.nome`) como o Dart |
| foldingRange | 0% | 100% | |
| semanticTokens (por token) | 0% | 100% | |
| inlayHint (por arquivo) | 17% | 50% | falta `InvalidType` e argumentos de tipo de chamada genérica |
| codeAction: correções p/ diag. do Dart | 17% | 75% | faltam `Add missing switch cases` e `Add required argument` |
| codeAction: ações de fonte | 1% | 0% | `Organize Imports` igual; `Fix All` e `Sort Members` não existem |
| hover (texto inteiro) | 17% | 86% | |
| hover (assinatura) | 86% | 97% | |
| definition | 95% | 96% | |
| typeDefinition | 28% | 95% | |
| implementation | 71% | 77% | o Dart procura no SDK inteiro |
| references | 56% | 58% | o Dart procura no SDK inteiro (aqui, o projeto carregado) |
| documentHighlight | 0% | 95% | |
| prepareRename | 95% | 96% | |
| rename | 88% | 89% | |
| codeAction: assistências no cursor | 8% | 26% | faltam Extract/Inline Method, Move to file, Convert to… |
| selectionRange | 0% | 48% | nós intermediários da árvore do analyzer |
| prepareCallHierarchy | 68% | 86% | |
| prepareTypeHierarchy | 22% | 76% | `range` do Dart é o nome; aqui a declaração |
| completion: alvo presente | 95% | 98% | |
| completion: mesmo top-1 | 46% | 63% | |
| completion: top-5 ≥ 60% comum | 48% | 52% | |
| signatureHelp (rótulo) | 53% | 99% | |
| workspace/symbol | 0% | 0% | o Dart inclui o SDK inteiro |

Latência (mediana no oráculo, ms, Dart / DartForge): hover 1,4 / 3,5 (era
3,5), completion 5,0 / 69,5 (era 67,6), signatureHelp 1,1 / 3,3,
documentHighlight 1,4 / 4,1, semanticTokens 2,1 / 4,0, inlayHint 1,7 / 3,4,
foldingRange 1,4 / 0,4. Com `examples/latencia_recursos.rs` (release,
Windows, 20 pedidos em `args/lib/src/arg_parser.dart`,
`args/lib/command_runner.dart` e `path/lib/src/context.dart`): hover parado
2,5–3,5 ms, completion parado 67–82 ms, signatureHelp 2,6–2,8 ms,
documentHighlight 3,2–4,1 ms, foldingRange 0,5–1,6 ms, hover logo após
edição 63–73 ms, completion logo após edição 74–88 ms. O hover e o
completar não pioraram; o completar continua dominado pela reinferência da
biblioteca a cada pedido.

#### Reprise de 2026-10-13 (macOS arm64)

Mesmos quatro projetos e mesmo cliente (com `experimental.snippetTextEdit`), Dart 3.6.2. Posições
divergentes (as que o `summary.py --examples 50` lista) antes → depois dos commits `fc496a7e` … `7c146b44`:

| recurso | antes | depois | posições |
|---|---:|---:|---|
| references | 96% | 100% | 16 → 0 |
| implementation | 96% | 100% | 18 → 1 (`int`: o Dart ainda não conhece `js_number.dart` naquele momento) |
| completion: alvo presente / top-1 / top-5 | 100 / 98 / 97% | 100 / 99 / 100% | 1/10/13 → 0/3/1 |
| definition | 98% | 100% | 10 → 0 |
| hover (texto inteiro) / typeDefinition | 99 / 100% | 100 / 100% | 3/1 → 1/0 |
| prepareCallHierarchy | 99% | 100% | 5 → 0 |
| demais | iguais | iguais | — |

Medianas (ms, Dart / DartForge, rodada final): references 2,6 / 2,1 (era 2,5), implementation 1,0 / 1,6
(era 2,1), completion 4,5 / 7,2 (era 8,4), workspace/symbol 8,1 / 11,6 (era 16,7), hover 1,0 / 1,1.

O que mudou, pela regra do Dart que faltava:

* **Busca pelo índice** (`SearchEngineImpl.appendAllSubtypes`, `Search.subTypes`): os subtipos vêm das
  relações que o índice grava, não do modelo de elementos. Um supertipo escrito por `typedef` (`with
  MapMixin<K, V>`, `ListMixin`, `SetMixin`) fica gravado no alias e não faz subtipo; um *class alias*
  (`class A = B with M;`) é subtipo. `implementation` e a família de `references` usam o mesmo mapa
  (`Project::written_subtypes`, guardado até uma unidade mudar).
* **Referências de documentação** contam só quando o `[x]` resolve para um dos elementos procurados
  (`_getRefElements`), não quando a família dele alcança o alvo; um `[x]` na documentação de um construtor
  com `this.x` cita o parâmetro (`ConstructorInitializerScope`).
* **Parâmetros `this.x`** (e os `super.x` que apontam para eles): só os do campo pivô
  (`includeParametersForFields` compara `parameter.field == member`); o *rename* continua renomeando os de
  toda a família (`RenameClassMemberRefactoringImpl.fillChange`).
* **Membros de `Object` num receptor `dynamic`** (`_resolveReceiverDynamicBounded`): `d.toString()`,
  `d.noSuchMethod(i)` (argumentos que casam), `d.hashCode` resolvem para `Object`.
* **Completar**: alvo classe de uma chamada sem tipo estático (sem membros de `Type`); `A.n(…)` com
  construtor `n` é criação (`AstRewriter`), e o nome lista só construtores; o passo dos não importados roda
  quando o *matcher* tem prefixo (nome no meio de uma declaração); variáveis de bibliotecas não inferidas
  ganham tipo sob demanda (`Query::ensure_variable_type`); um par getter/setter explícito são dois membros
  (a chave do mapa distingue o setter; os elementos dão o nome nu aos dois); membros herdados privados de
  outra biblioteca entram (`_addInheritedMembers` não filtra).
* **Definição** (`DartNavigationComputer` só sobre o nó do cursor): o nome de classe de uma declaração de
  construtor e o nome de uma anotação vão para a classe; `super.x` vai para o parâmetro do construtor
  super; a variável de um `for-in` não tem destino.
* **Hover/typeDefinition**: setter escrito mostra o `writeType`; rótulo de argumento nomeado de
  construtor do SDK usa o tipo do esboço; nome indefinido mostra só ``Type: `InvalidType` ``.
* **Hierarquia de chamadas**: o `==` é o da interface do receptor (`num.==`, `String.==`); anotação
  prepara o construtor.
* **`Create function/method`** escreve os nomes declarados dos parâmetros do tipo função (anotação
  `Function`, parâmetro antigo `void f(int x)` ou `typedef`); só os sem nome viram `p1`…; argumento nomeado
  (`k: onE`) também é atendido.

* **Pastas do workspace**: a enumeração dos pacotes das raízes (I7) saiu do laço principal para o
  trabalhador tipado, sem a trava: com uma raiz grande (`/tmp`), um pedido de 1,5 s levava 16 s; agora
  2 s. Os testes de protocolo abrem sem pasta de workspace (de ~10–20 s para 3 s; o `cancellation` não
  encosta mais no limite de 20 s).

Restam: 1 hover (o Dart reescreve a cerca ```` ```dart template:expression ```` como ```` ```dart:expression ````),
1 implementation (estado do Dart), 3 top-1 do completar (relevância: tipo de contexto de `match = …`
quando a variável é parâmetro, `commands` antes de `command`, `SourceFile` × construtor), 1 top-5
(construtores de classe não importada no lugar da classe), 1 documentHighlight, 1 selectionRange, 1
inlayHint e as 7 assistências/ações de fonte em imports com prefixo (`-32001` do próprio Dart).

### Regras por recurso

**Completar** (`lsp/handlers/handler_completion.dart`,
`services/completion/dart/*`). Além do descrito em "Consultas semânticas
por requisição":

* *relevância* — a do `RelevanceComputer`/`FeatureComputer` do Dart
  (`crates/lsp/src/relevancia.rs`): média ponderada das características
  tipo de contexto (o tipo que a posição espera — parâmetro do argumento,
  alvo da atribuição, tipo escrito da variável, retorno da função, `bool`
  da condição — contra o tipo do item: igual 1,0, subtipo 0,40, supertipo
  0,02, sem relação 0,13), espécie do elemento no local do completar (as
  tabelas estatísticas `relevance_tables.g.dart` do 3.6.2, geradas em
  `relevancia_tabelas.rs` por `crates/lsp/oraculo/gerar_tabelas.py`; o local
  é o papel da posição no nó pai: `Block_statement`,
  `ArgumentList_method_unnamed`, `PropertyAccess_propertyName`,
  `ReturnStatement_expression`, `VariableDeclaration_initializer`…; locais
  pela proximidade e membros pela distância de herança, `0,9^d`),
  palavra-chave (o alto da faixa no local), não importado (−1), nome com `$`
  e `noSuchMethod`; `⌊((média + 1) / 2) × 1000⌋`, e os argumentos nomeados
  com 900 (950 obrigatório). A ordem é a relevância (o digitado como
  prefixo antes do casamento aproximado; entre iguais, o grupo e o nome);
* nada no nome de uma declaração (variável com `var`/`final`/tipo,
  parâmetro com tipo, função, classe…), como o Dart;
* `Cor.▮` lista as constantes do enum; `show`/`hide` lista o que a
  biblioteca da diretiva exporta; `this.▮` num construtor lista os campos
  ainda não inicializados e `super.▮` os parâmetros do construtor da
  superclasse ainda não repassados;
* com `completionItem.labelDetailsSupport`, o rótulo é só o nome e
  `labelDetails` traz a assinatura curta (`(…) → int`, ` String`) e a
  biblioteca a importar, como o Dart faz com esse cliente.

Divergências conhecidas do completar: o Dart sugere também o que o SDK
inteiro declara sem import (`InternetAddress`… com `isNotImported`); aqui só
as bibliotecas `dart:` importáveis que o índice conhece. A latência
(~70 ms contra ~5 ms) vem da inferência da biblioteca refeita a cada pedido.

**Ajuda de assinatura** (`computer/computer_signature.dart`,
`lsp/handlers/handler_signature_help.dart`, `lsp/mapping.dart`
`toSignatureHelp`). `signatureHelpProvider` com `triggerCharacters: ["("]`
e `retriggerCharacters: [","]` (`lsp/constants.dart`). A lista é a
`ArgumentList` mais interna que contém o cursor (do `(` exclusive ao `)`
inclusive); uma `FunctionExpression` entre o cursor e a lista encerra a
busca (cursor no corpo de um *closure* argumento não mostra a chamada de
fora). Só chamadas de método ou função (`MethodInvocation`), criações de
instância (`InstanceCreationExpression`, com ou sem `new`) e invocações de
expressão cujo alvo é um identificador; getter ou campo que devolve função
não tem assinatura. Rótulo `nome(p1, [p2], {p3})`, com `nome` o nome do
método, ou o nome qualificado do tipo criado (`p.A.nome`), e cada
parâmetro `[required ]tipo nome[ = padrão]`, tipo como o elemento o vê (num
receptor genérico, substituído: `List<int>.add` → `add(int value)`; numa
criação, pelos argumentos do tipo criado) e o padrão como escrito. Um só
`SignatureInformation`, `activeSignature: 0`; documentação do elemento (a do
hover, Markdown se o cliente aceita). Disparo automático (`triggerKind: 2`,
`isRetrigger: false`) só responde quando o cursor está logo depois do `(`
que abre a lista. Escolha: o 3.6.2 manda `activeParameter: -1` (nenhum
parâmetro destacado); o servidor atual calcula o parâmetro ativo (o do
argumento sob o cursor, nomeado pelo nome, posicional pela posição; entre
argumentos, o próximo posicional) e manda o tamanho da lista quando não há
(`null` ao cliente com `noActiveParameterSupport`). Seguimos o atual: no VS
Code, o parâmetro sendo digitado fica destacado. Assinatura de argumentos de
tipo (`List<▮>`) não existe aqui.

**Destaques no documento** (`lsp/handlers/handler_document_highlights.dart`,
`DartUnitOccurrencesComputer`): as ocorrências do elemento sob o cursor no
próprio arquivo, a declaração inclusive, sem `kind`. Aqui, pela identidade
do `references` (`Projeto::ocorrencias`) restrita à unidade; num membro de
instância a família inteira (o Dart separa o membro sobrescrito do que
sobrescreve).

**Implementações** (`lsp/handlers/handler_implementation.dart`,
`search/type_hierarchy.dart` `TypeHierarchyComputerHelper`): o elemento sob
o cursor; se é classe (ou construtor dela), os subtipos transitivos; se é
membro de instância de classe, a declaração do membro em cada subtipo que o
declara (ou o recebe de um mixin; não os intermediários sem declaração);
local, topo que não é classe e membro de extension type não têm
implementação (lista vazia). Os locais são o nome de cada declaração. A
busca cobre o projeto carregado (as bibliotecas do projeto e o que elas
importam) e segue as relações do índice: supertipo escrito por `typedef` não
conta, *class alias* conta, `Object` implícito só nos arquivos que citam o nome.

**Definição do tipo** (`lsp/handlers/handler_type_definition.dart`): o nó
sob o cursor dá o tipo — nome de tipo, a própria classe; declaração de
variável, de parâmetro ou de `for-in`, o tipo declarado; expressão, o tipo
estático (com promoção); método ou função, nada (é tipo função) — e só tipo
de interface (classe, enum, mixin, extension type; `int?` vai para `int`)
tem destino: o nome da declaração do tipo. Sem destino, lista vazia. Sem
`LocationLink` (o cliente do oráculo não anuncia `linkSupport`).

**Dobras** (`computer/computer_folding.dart`,
`lsp/handlers/handler_folding.dart`): as regiões em ordem de visita da
árvore — anotações (do fim do nome da primeira ao fim da última), corpo de
classe e de mixin (do fim do nome ao `}`), de enum, extensão e extension
type (entre as chaves), construtor, método e função (do fim do nome ao
fim), listas de parâmetros e de argumentos (entre os parênteses; se a linha
do `(` já tem região, a partir do primeiro parâmetro ou argumento), literais
de lista, conjunto, mapa e registro, blocos de `if`/`else`/`while`/`do` (do
`{` ao fim do último comando, ou do último comentário antes do `}`), `for`
e corpo de *closure* (do `{` ao `}`), `switch` e cada caso (do `:` ao fim),
`switch` de expressão e cada caso (do `=>` ao fim), `assert` e strings de
várias linhas —, depois as diretivas (da palavra-chave da primeira ao fim da
última, `imports`) e os comentários (`/* */` do fim da primeira linha;
`//`/`///` consecutivos do mesmo tipo e sem linha em branco, do fim do
primeiro ao fim do último; `comment`). Só regiões de mais de uma linha;
duas não começam na mesma linha. Ordenadas pelo início; com
`lineFoldingOnly`, a que termina na linha em que a seguinte começa (sem
contê-la) termina na linha anterior, e some se ficar com uma linha.

**Faixas de seleção** (`computer/computer_selection_ranges.dart`): os nós
que contêm o cursor, do mais interno ao mais externo, sem repetir intervalo;
a declaração inclui o comentário de documentação. Aqui, dos nós da árvore do
parser (nomes, expressões, comandos, tipos, padrões, parâmetros, listas de
argumentos, `nome: valor`, `x = e`, declarações, diretivas); a árvore do
analyzer tem nós intermediários que esta não tem (`ExpressionStatement` com
o `;`, `VariableDeclarationList`, `FormalParameterList`), então alguns
degraus diferem.

**Dicas embutidas** (`computer/computer_inlay_hint.dart`): o arquivo inteiro
(o intervalo pedido é ignorado, como no Dart); tipo antes do nome de
variável sem tipo escrito (local, campo, topo, `for-in`, padrão), antes do
nome de parâmetro sem tipo (de *closure* inclusive; não `this.x`/`super.x`)
e de função ou método sem tipo de retorno (antes do `get` num getter; não
em setter); `nome:` antes de cada argumento posicional de chamada
resolvida; argumentos de tipo inferidos (`<int>`) antes do `[`/`{` de
literal sem argumentos escritos e depois do nome da classe numa criação sem
argumentos escritos. `label` em partes (sem `location`), `paddingRight`
como o Dart. Falta: argumentos de tipo inferidos de chamada genérica
(`map<String>(…)`), que pedem os argumentos inferidos da inferência comum.

**Tokens semânticos** (`computer/computer_highlights.dart`,
`lsp/semantic_tokens/{mapping,legend,encoder}.dart`): a legenda é a do
3.6.2, na mesma ordem (tipos `annotation, keyword, class, comment, method,
variable, parameter, enum, enumMember, type, source, property, namespace,
boolean, number, string, function, typeParameter`; modificadores
`documentation, constructor, declaration, importPrefix, instance, static,
escape, annotation, control, label, interpolation, void, wildcard`). Palavras
reservadas e embutidas viram `keyword` (`control` nas de fluxo), `true`/
`false` `boolean`, `void` `keyword`+`void`; números, strings (cada trecho
de uma interpolada; `${…}`/`$x` como `source`+`interpolation`; escapes como
`string`+`escape`) e comentários (`documentation` em `///`/`/**`); cada nome
pelo elemento que denota — declarações com `declaration` (classe,
extension type, método, getter/setter, campo e variável), referências a
getter, campo e variável de topo como `property`, campo declarado como
`variable` (com `instance` ou `static`), métodos com `instance`/`static`,
construtores (`class`/`method` + `constructor`), locais, parâmetros
(`label` nos rótulos `nome:`), parâmetros de tipo, constantes de enum,
prefixos (`importPrefix`), e `source` para o não resolvido. Anotações:
`annotation` do `@` ao `(` (ou ao fim) e no `)`, com `annotation` nos
nomes. Sobreposições divididas pelo de cima (`splitOverlappingTokens`);
sem `multilineTokenSupport`, um token por linha.

**Hierarquia de tipos** (`lsp/handlers/handler_type_hierarchy.dart`,
`computer/computer_lazy_type_hierarchy.dart`): `prepareTypeHierarchy` no
nome de um tipo (ou dentro de uma classe, mixin, enum ou extension type)
devolve o item da classe (`kind: 5`, nome com os parâmetros de tipo,
`range` a declaração, `selectionRange` o nome); `supertypes`: superclasse
(`Object` quando não há `extends`), restrições `on`, interfaces e mixins, com
os argumentos de tipo escritos (`Base<int>`); `subtypes`: as classes do
projeto que citam o tipo em `extends`, `implements`, `on` ou `with`.

**Ações rápidas**: além das descritas em `textDocument/codeAction` acima,
`crates/lsp/src/correcoes.rs` traz as dos códigos publicados que ainda não
tinham correção, com título, espécie e edição dos produtores do 3.6.2
(`services/correction/dart/*.dart`, ligados em `fix_internal.dart`):
`non_abstract_class_inherits_abstract_member` (`Create N missing
override(s)` — os membros sem implementação concreta na cadeia da classe,
ordenados pelo nome, getter antes de setter, par getter/setter como campo,
tipos substituídos pelos argumentos com que a classe vê o supertipo,
corpo `// TODO: implement x` + `throw UnimplementedError();`, depois do
último membro —, `Create 'noSuchMethod' method`, `Make class 'C'
abstract`), `concrete_class_with_abstract_member` (as duas últimas),
`unused_field`, `unused_catch_clause`, `unused_catch_stack`,
`assignment_to_final_local`, `missing_default_value_for_parameter` (`Add
'required' keyword` num nomeado, `Make 'x' nullable`),
`await_in_wrong_context` (`Add 'async' modifier`, com `Future<…>` no
retorno escrito), `nullable_type_in_*_clause` (`Remove the '?'`),
`const_instance_field` (`Add 'static' modifier`), `non_final_field_in_enum`
(`Make final`), `extension(_type)_declares_member_of_object` (`Remove method
declaration`) e `assert_in_redirecting_constructor` (`Remove the
assertion`). As correções valem para os diagnósticos das linhas pedidas (o Dart filtra
por linha), as assistências para o cursor; `Ignore 'x' for this line`/`for
the whole file` vêm por último para os diagnósticos que não são erro.
Assistências de reescrita (`crates/lsp/src/assistencias.rs`): `Convert to
async function body`, `Convert to block body`, `Convert to expression
body`, `Remove type annotation`, `Split variable declaration`, `Use curly
braces`, `Assign value to new local variable`, `Inline Local Variable`,
`Extract Local Variable`; criação de membros (`crates/lsp/src/criar.rs`):
`Change to 'x'` (distância de edição < 3), `Create method/function/class/
mixin/getter/field/local variable`, `Create extension method/getter`.
`source.organizeImports` segue o `ImportOrganizer` (dart:, package:,
relativos; exports depois; duplicados removidos). Faltam: `Fix All`, `Sort
Members`, `Add missing switch cases`, `Add required argument`, `Extract
Method`, `Inline Method`, `Move to file`.

**Hierarquia de chamadas** (`computer/computer_call_hierarchy.dart`,
`lsp/handlers/handler_call_hierarchy.dart`): `prepareCallHierarchy` num
executável escrito (função, método, operador, getter/setter escritos,
construtor; o sem nome implícito fica na classe), com nome exibido (`get
x`, `set x`, `A.nome`), espécie (12 função, 6 método, 9 construtor, 7
propriedade), `detail` o contêiner (classe ou arquivo), `range` a
declaração e `selectionRange` o nome; `incomingCalls`: as referências (a
família de sobrescrita inclusa) agrupadas pelo executável que as contém
(ou pela classe, num inicializador de campo, ou pelo arquivo);
`outgoingCalls`: chamadas, criações e leituras de getter escrito no corpo,
agrupadas pelo chamado. As recebidas procuram no projeto carregado.

**Fora (e por quê)**: formatação (decisão L08 acima); `documentLink` (no
3.6.2 só liga `** See code in examples/api/…` de comentários do Flutter);
`codeLens`, `colorProvider` e `inlineValue` (desligados no 3.6.2 sem
configuração do cliente).

## Testes

`crates/lsp/tests/protocolo.rs` (processo filho, bytes no fio):
`sessao_completa` (2 erros → correção incremental → 1 → fechar → 0 →
código 0), `utf16_correto` (emoji 4 bytes/2 unidades, CRLF, escape de
surrogate solto), `stdout_limpo` (com `RUST_LOG=debug`, tudo decodifica
quadro a quadro), `cancelamento` (resposta -32800).
`tests/editor_session.rs` (processo filho): uma sessão de editor num projeto
de dois pacotes com SDK mínimo — relatório do SDK, `$/setTrace`, hover,
definição entre pacotes, referências, digitação caractere a caractere com
completar/hover/tokens no meio de erros de sintaxe, renomear, formatar,
mudança de `package_config.json` anunciada, rodadas de edição com a memória
residente medida, `didClose`, `shutdown`/`exit` —, sem erro interno, sem
pânico e sem notificação ignorada no `stderr`; também SDK ausente (um único
aviso), `sdkPath` nas opções, código profundamente aninhado e a mesma pressão
num projeto real (`DARTFORGE_LSP_EDITOR_PROJECT`, até
`DARTFORGE_LSP_EDITOR_FILES` arquivos; pulado sem a variável).
`tests/plato_protocolo.rs` (`plato_pelo_protocolo`, K=24 × K=24 pelo
transporte com arquivos reais): binário separado de propósito — o alocador
contador é global e os testes de um binário dividem o processo, então a
medição precisa do processo só para ela.
`tests/completar.rs`, `tests/renomear.rs` e `tests/acoes.rs` falam
JSON-RPC com o `Servidor` semântico sobre um projeto temporário
(`pubspec.yaml`, `package_config.json`) e um SDK mínimo em disco
(`tests/comum`), sem depender do Dart instalado; cobrem código incompleto
(`a.` solto, `a.ca` sem `;`, chamada sem `)`, comando quebrado no corpo).
`tests/tipados.rs` usa o mesmo apoio para o fluxo tipado: código publicado
que depende de tipos (`non_bool_condition`) só na publicação tipada, só
códigos publicados e `// ignore:` respeitado, resultado de versão velha
descartado, rajada de edições coalescida numa análise, arquivo importado
editado republicando quem o importa, e `didClose` descartando resultado
pendente.

## Medição D.2 — mesmo projeto, mesma sequência

Comando: `scripts/measure-lsp.ps1` (sem flag: DartForge; com `-Dart`: o LSP
do Dart). Sequência idêntica: `initialize`, 1.258 `didOpen`, 200 edições
incrementais distribuídas, `shutdown` + `exit`; RSS via `Get-Process` a
cada 50 mensagens. Projeto `C:/MyDartProjects/new_sali` (ngdart + angel3,
1.258 arquivos, 8,52 MiB). Máquina e data da rodada: Windows x64,
2026-09-22, binário release.

| | DartForge (`dartforge-lsp`) | Dart (`dart language-server --protocol=lsp`) |
| --- | --- | --- |
| Mensagens enviadas | 1.462 | 1.462 |
| Quadros recebidos | 1.460 (diagnóstico por abertura e edição) | 1 (só o `initialize`; nada publicado em 2 min) |
| Tempo total | 2,5 s | 131,9 s (com 120 s de espera final) |
| Pico RSS | 21,3 MiB | 640,5 MiB |
| Platô RSS (média das 10 últimas) | 19,0 MiB | 633,7 MiB |
| Saída | 0 | 0 |

In-process (alocador contador, `crates/lsp/examples/memoria_lsp.rs`, tudo retido como num editor
com o projeto aberto): 8,52 MiB de fonte → 19,94 MiB vivos (2,34×),
pico 20,43 MiB, 893 ms, 1,37 M alocações; após fechar tudo, 1.024 bytes
acima da base (o `Vec` de caminhos do próprio exemplo ainda vivo na hora
da impressão; o teste de platô, sem esse resíduo, volta ao nível exato).

Leitura honesta: o número de 6 GB do PLANO.md é o relato de campo do
proprietário em edição interativa, não reproduzido aqui — o Dart, nesta
sequência em lote, estabilizou em ~634 MiB sem publicar diagnósticos na
janela (possível causa: sem `package_config` no projeto, a resolução de
pacotes fica prejudicada, ou a análise priorizada nunca alcança 1.258
arquivos abertos de uma vez). O que a comparação prova, sem extrapolar:
na mesma sequência, o DartForge publica diagnóstico para cada abertura e
edição retendo ~20 MiB, e N edições estabilizam em platô (afirmado pelo
teste) em vez de crescer com N.

Prova de ponta a ponta com arquivo real (binário release, mesma invocação
da extensão: `dartforge-lsp --stdio`; `local_signer_server.dart` do
`new_sali` + `void quebrado( { }` inserido): 1 diagnóstico, linha 127
coluna 0, `source: "dartforge"`, mensagem `esperava ')', encontrou o fim
do arquivo`. No VS Code isso aparece como sublinhado vermelho na linha 128
com a mensagem no hover e no painel Problemas; o canal *Output ›
DartForge* mostra o ciclo de vida. (Janela de desenvolvimento via F5 com a
extensão compilada — passos em `editors/vscode/README.md`; sem captura de
tela neste ambiente.)

## Memória com a pasta do workspace aberta (2026-10-13)

Regressão achada na reprise do oráculo: com a raiz do workspace aberta (I7),
o trabalhador tipado analisa **todos** os arquivos de cada pacote, e o
`new_sali` (858 arquivos: `core` e `frontend`) deixava o servidor com pico de
~430–920 MiB e RSS em repouso de 99–640 MiB, variando entre rodadas — contra
21,3 / 19,0 MiB medidos antes do I7.

### Diagnóstico: alocador, não retenção

Instrumentação (macOS arm64, release, SDK 3.6.2; cópia do `new_sali` no
scratchpad com `package_config.json` regravado):

* `DARTFORGE_LSP_MEMORY=1` registra no `stderr` o heap em uso
  (`malloc_zone_statistics`) antes e depois de cada análise e nos pontos de
  cancelamento do motor (`crates/lsp/src/memory.rs`).
* `vmmap --summary` e `heap -s` no processo em repouso.
* `cargo run --release -p dartforge-lsp --example workspace_memory --
  <pacote>`: alocador contador sobre a mesma carga do trabalhador, com o
  quanto cada estrutura libera ao cair.

| | antes |
|---|---|
| heap em uso em repouso (858 abertos, 200 edições) | 17–27 MiB |
| `vmmap`: alocado / fragmentado nas zonas `malloc` | 17,1 MiB / 494–618 MiB (97–98%) |
| RSS em repouso | 472–917 MiB (cai a 4–60 MiB quando o sistema comprime as páginas) |
| *physical footprint* em repouso | 240–477 MiB |
| pico vivo da análise do `frontend` (dentro do processo) | 590–658 MiB |

O que fica vivo depois de uma análise é pouco (documentos e diagnósticos
publicados); o resto do RSS são páginas que o alocador do sistema guardou da
análise que acabou. `malloc_zone_pressure_relief` e uma zona `malloc` própria
para a análise não as devolveram (num teste sintético no macOS 27, liberar
tudo leva o *footprint* de 2.080 a 673 MiB, e nem `pressure_relief` nem
`malloc_destroy_zone` descem disso); o
RSS também não serve de régua no macOS — páginas marcadas como reutilizáveis
contam no RSS até o kernel pegá-las, e sob pressão o RSS cai a poucos MiB
com o *footprint* intacto. As medidas abaixo usam o *physical footprint*
(`proc_pid_rusage`), que é o que o Monitor de Atividade e o *jetsam* usam.

Para onde vai o pico da análise do `frontend` (3.316 bibliotecas, 37 MiB de
fonte; 326 arquivos do pacote, o resto dependências): carga 383 MiB —
expressões 141, comandos 41, membros 32, tipos 22, funções 18 dos nós; escopos
das bibliotecas 56 (672 mil entradas de `scope` materializadas, 312 mil de
prefixos); fontes 37; elementos 31; *interner* 7 —, *outline* +24, a passada
dos inicializadores +36 (temporária) e o índice de faixas do motor ~100 MiB.

### Correção

1. **Cada análise tipada num processo filho** (`crates/lsp/src/typed.rs`,
   `run_in_child`): o binário chama a si mesmo com `--typed-analysis`, manda
   a tarefa (pacote, textos abertos, configuração) em JSON pelo `stdin`, lê os
   resultados pelo `stdout` e o filho termina — o programa nunca entra no
   servidor. Cancelar mata o filho; pânico ou estouro de pilha na análise
   derruba só o filho. Publicação idêntica nos dois modos
   (`typed::child_process_publishes_like_the_thread`).
2. **Índice de faixas compacto no motor** (`crates/parity/src/analysis.rs`,
   `Index`): as unidades fora das bibliotecas do lote (as dependências) iam
   num `HashMap` com um `Vec` por nó; agora num vetor ordenado de
   `(início, fim, unidade)`, 12 bytes por nó, construído só quando um
   diagnóstico sem unidade fora dos inicializadores pede (a maioria das
   análises não tem nenhum). Os textos analisados também não ficam mais em três
   cópias. Pico vivo do `frontend` 590,6 → 515,2 → 492,1 MiB (*footprint* do
   processo 786 → 634 → 612 MiB); diagnósticos publicados idênticos em nove
   projetos (`limitless_ui`, `especificacao`, `new_sali/core`, `args`…).
3. Textos abertos compartilhados com as tarefas (sem duas cópias por
   análise); arquivo fechado sem diagnóstico volta sem o texto; o trabalhador
   não parece ocioso enquanto percorre as pastas (o `wait_idle` voltava antes
   da primeira análise — corrida que o teste novo pegou).

### Resultado (`new_sali`, 858 `didOpen` + 200 edições + 20 s, macOS arm64)

| | antes (análise na thread) | depois | Dart 3.6.2 |
|---|---|---|---|
| servidor: pico de *footprint* | 602–640 MiB | 27,1–30,6 MiB | — |
| servidor: platô de *footprint* | 240–477 MiB | 27,1–30,6 MiB (estável entre rodadas) | — |
| filho da análise (só durante ela) | — | 639 MiB (`frontend`, com os 858 textos abertos), 310 MiB (`core`) | — |
| pico total (servidor + filho) | 602–640 MiB | 664 MiB | 737–1.402 MiB |
| em repouso | 240–477 MiB | 27 MiB | 736–1.286 MiB |

O Dart, aqui com `package_config.json` válido, analisa de fato (na medição
D.2 no Windows ele não tinha `package_config` e ficou em 640 MiB sem
publicar); o *footprint* dele variou de 737 a 1.402 MiB entre rodadas. O
pico do DartForge é o do filho que analisa o maior pacote, e volta inteiro ao
sistema quando ele termina; o que segue para baixar o pico está na lista
acima (escopos materializados, tamanho dos nós de expressão).

Custo do processo filho na latência da publicação tipada (binário release,
`args/lib/src/arg_parser.dart` aberto sem pasta de workspace, 10 edições
esperando cada publicação): mediana 40 ms com o filho, 35 ms na thread
(mín. 29 ms nos dois).

`scripts/measure-lsp.ps1` soma o RSS dos processos filhos.
Teste: `tests/plateau_workspace.rs` — o mesmo pacote de workspace analisado
na thread e em processos filhos (o pico do servidor com filhos é uma fração
do da thread: 0,05 contra 0,99 MiB) e rodadas de abrir, editar e fechar
terminando no mesmo nível vivo.

Oráculo depois (mesmos quatro projetos e cliente da reprise): todas as
categorias idênticas à rodada final da reprise. Medianas (ms, Dart /
DartForge) com a máquina sob carga (média de carga ~45): references 3,1 /
2,5, implementation 1,4 / 2,2, completion 6,0 / 8,8, hover 1,1 / 1,2 — as
razões DartForge/Dart ficaram iguais ou menores que na reprise (references
0,81, implementation 1,6 → 1,57, completion 1,6 → 1,47).

