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
cada documento aberto, sem diretórios ocultos, `build` nem subpacotes. O
texto aberto vale mais que o disco; cada árvore é descartada antes da
próxima (nada fica indexado entre pedidos). O casamento é aproximado
(`crates/lsp/src/aproximado.rs`, no espírito do `FuzzyMatcher` do Dart):
prefixo, contém e iniciais de palavras (`cR` acha `CarregadorRemoto`); os
que contêm a consulta vêm antes das subsequências, e entre iguais a ordem é
(URI, posição). O resultado é `SymbolInformation[]`, no intervalo do nome.

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

