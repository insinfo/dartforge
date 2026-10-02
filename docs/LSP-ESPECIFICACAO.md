# LSP: especificação do servidor do Dart 3.6.2 e paridade do DartForge

Este documento descreve, requisição por requisição, o que o `dart
language-server` do SDK 3.6.2 faz — entrada, algoritmo com `arquivo:linha` da
fonte oficial, formato e ordem exatos da resposta, casos de borda — e, para
cada uma, o que o servidor do DartForge (`crates/lsp`) faz hoje e onde
diverge, medido no oráculo de paridade (`crates/lsp/oraculo/`, descrito em
`docs/LSP.md`, "Paridade com o servidor do Dart (3.6.2)").

**Fonte.** `E:\references\dart-sdk-3.6.2\pkg\analysis_server\lib` e
`...\pkg\analyzer\lib` (tag 3.6.2). Prefixos: `AS:` =
`pkg/analysis_server/lib/`, `AN:` = `pkg/analyzer/lib/`, `AP:` =
`pkg/analyzer_plugin/lib/`, `PLUGIN:` = `pkg/analysis_server_plugin/lib/`
(os dois últimos lidos da mesma tag no clone `E:\references\dart-sdk`, porque
não estão na cópia 3.6.2).

**Medida.** O oráculo abre os mesmos arquivos nos dois servidores e pede cada
recurso em 410 posições (12 identificadores por arquivo em 36 arquivos de 4
projetos: `args-2.7.0`, `path-1.9.1`, `string_scanner-1.4.1` e um `app` com
erros comuns), mais os pedidos por arquivo e por diagnóstico. "Igual" é a
resposta normalizada idêntica à do Dart. Os números abaixo são os da última
rodada (`E:\dftemp\lsp\depois.jsonl`), antes das mudanças desta rodada.

## Resumo

| Requisição | Igual ao Dart | Amostras | Seção |
|---|---|---|---|
| `initialize` (capacidades) | parcial | — | 2 |
| `publishDiagnostics` | ver paridade do analisador | — | 3 |
| `textDocument/documentSymbol` | 97% | 36 | 4 |
| `workspace/symbol` | 0% | 24 | 5 |
| `textDocument/foldingRange` | 100% | 36 | 6 |
| `textDocument/selectionRange` | 48% | 410 | 7 |
| `textDocument/formatting` | 0% (não implementado) | 36 | 8.1 |
| `textDocument/documentLink` | 100% | 36 | 8.4 |
| `documentColor`, `codeLens`, `willRenameFiles`, `dart/*` | não implementados | — | 8 |
| `textDocument/hover` (texto / assinatura) | 86% / 97% | 410 | 9 |
| `textDocument/definition` | 96% | 410 | 10 |
| `textDocument/typeDefinition` | 95% | 410 | 10 |
| `textDocument/implementation` | 77% | 410 | 10 |
| `textDocument/references` | 58% | 410 | 11 |
| `textDocument/documentHighlight` | 95% | 410 | 11 |
| `textDocument/prepareRename` / `rename` | 96% / 89% | 410 | 12 |
| `textDocument/prepareCallHierarchy` | 86% | 410 | 12 |
| `textDocument/prepareTypeHierarchy` | 76% | 410 | 12 |
| `textDocument/codeAction` — quick fixes p/ diagnósticos | 75% | 12 | 13 |
| `textDocument/codeAction` — assists no cursor | 26% | 410 | 13 |
| `textDocument/codeAction` — ações de fonte | 0% | 422 | 13 |
| `workspace/executeCommand` | não implementado | — | 13 |
| `textDocument/completion` — alvo / top-1 / top-5 | 98% / 63% / 52% | 410 | 14 |
| `textDocument/signatureHelp` (rótulo) | 99% | 149 | 15.1 |
| `textDocument/semanticTokens/full` (por token) | 100% (7 de 13 877) | 36 | 15.2 |
| `textDocument/inlayHint` (por arquivo) | 50% | 36 | 15.3 |

Latência (mediana, ms, Dart / DartForge): hover 1,4 / 3,5; completion 5,0 /
69,5; definition 1,2 / 3,4; references 2,2 / 4,3; semanticTokens 2,1 / 4,0;
inlayHint 1,7 / 3,4; foldingRange 1,4 / 0,4.

## 1. Convenções comuns a todos os handlers

Referências: `AS:` = `pkg/analysis_server/lib/`, `AN:` = `pkg/analyzer/lib/`,
`AP:` = `pkg/analyzer_plugin/lib/` e `PLUGIN:` = `pkg/analysis_server_plugin/lib/`.
Os dois últimos não estão em `E:\references\dart-sdk-3.6.2\pkg`; foram lidos
com `git -C E:\references\dart-sdk show 3.6.2:<caminho>` (mesma tag).

**Posições.** `toRange(lineInfo, offset, length)` (`AS:src/lsp/mapping.dart:1325-1335`)
usa `LineInfo.getLocation` (1-based) menos 1 (`toPosition`, `:1319-1323`);
offsets e colunas em unidades UTF-16 (o servidor não anuncia
`positionEncoding`). `toOffset` (`mapping.dart:1288-1307`) só valida a
**linha** (`line >= lineCount` → `InvalidFileLineCol (-32004)` `"Invalid line
number"`, ou `ClientServerInconsistentState (-32099)` quando
`failureIsCritical`); a coluna não é validada (`getOffsetOfLine(line) +
character` transborda para a linha seguinte).

**URI → caminho.** `pathOfUri` (`AS:src/lsp/handlers/handlers.dart:107-171`), erros
`InvalidFilePath (-32003)`: URI nula `"Document URI was not supplied"`; sem
esquema `"URI is not a valid file:// URI"`; esquema não suportado `"URI scheme
'x' is not supported. Allowed schemes are 'file', ..."`; no Windows, caminho
começando com `\` `"... (missing drive letter)"`; exceção `"URI does not
contain a valid file path"`. "É Dart?" é `uri.path.endsWith('.dart')`
(`mapping.dart:519-521`).

**Unidades** (`handlers.dart:179-254`): `requireResolvedUnit` /
`requireUnresolvedUnit` tentam de novo depois de esperar a inicialização e o
rebuild de contextos; falhando, `FileAnalysisFailed (-32013) 'Analysis failed
for file'` (arquivo analisado) ou `FileNotAnalyzed (-32007) 'File is not being
analyzed'`; `!exists` → `-32003 'File does not exist'`. O driver escolhido
(`AS:src/analysis_server.dart:585-603`) é o do contextRoot que analisa o
arquivo, senão um que já o conhece, senão **o primeiro**: arquivos fora das
raízes ainda são resolvidos.

**Erros genéricos.**

| Situação | Resposta | Fonte |
|---|---|---|
| params inválidos | `InvalidParams (-32602)` `"Invalid params for <método>:\n<1º erro>"` | `handlers.dart:356-372` |
| `InconsistentAnalysisException` | `ContentModified (-32801)` `'Document was modified before operation completed'` | `AS:src/lsp/lsp_analysis_server.dart:528-534` |
| exceção não tratada | `UnhandledError (-32001)` `'An error occurred while handling <método> request'` + `window/logMessage` | `lsp_analysis_server.dart:535-549` |
| erro numa notificação | `window/showMessage` (Error) | `lsp_analysis_server.dart:767-779` |
| `ClientServerInconsistentState` | o servidor passa a recusar tudo (`'An unrecoverable error occurred and the server cannot process messages'`) e chama `shutdown()` | `lsp_analysis_server.dart:781-792`; `handler_states.dart:62-70` |
| método desconhecido | notificação `$/…` ignorada; outra mensagem `MethodNotFound (-32601) 'Unknown method <m>'` | `handler_states.dart`, `handlers.dart:465-492` |

**Estado no DartForge.** Posições e intervalos em UTF-16
(`crates/lsp/src/utf16.rs`, `DocumentStore`); a coluna além do fim da linha é
recortada para o fim da linha (não transborda). Erros: `-32601` para método
desconhecido, `-32602` para parâmetros inválidos, `-32603` para pânico numa
consulta (o Dart usa `-32001 UnhandledError`), `-32800` para cancelamento,
`-32010` (`RenameNotValid`) no renomear. Um arquivo fora do projeto é
carregado como biblioteca avulsa (como o "primeiro driver" do Dart).
**Diverge:** o código do erro interno (`-32603` × `-32001`) e a mensagem; o
transbordo de coluna. **Pendente:** usar `-32001` com a mensagem do Dart.

## 2. Ciclo de vida e configuração

### 2.1 `initialize`

**Algoritmo** (`AS:src/lsp/handlers/handler_initialize.dart:24-82`):
1. `handleClientConnection` (`lsp_analysis_server.dart:408-446`) guarda as
   capacidades (`AS:src/lsp/client_capabilities.dart:147-255`) e lê
   `initializationOptions` (`lsp_analysis_server.dart:1187-1208`): `appHost`,
   `remoteName`; `onlyAnalyzeProjectsWithOpenFiles`, `closingLabels`,
   `outline`, `flutterOutline`, `allowOpenUri`, `useInEditorDartFixPrompt`
   (verdadeiros só com `== true`); `suggestFromUnimportedLibraries` (padrão
   true); `completionBudgetMilliseconds`.
2. Sem `onlyAnalyzeProjectsWithOpenFiles`, as raízes são os
   `workspaceFolders` `file:` mais `rootUri` (ou `rootPath`) (`:41-59`).
3. Resposta `InitializeResult{capabilities, serverInfo:{name:'Dart SDK LSP
   Analysis Server', version}}` (`:70-81`).

**Capacidades estáticas** (`AS:src/lsp/server_capabilities_computer.dart:163-227`).
Uma opção estática só vai quando o cliente **não** declarou
`dynamicRegistration` para a feature (`registration/feature_registration.dart:246-247`);
senão é registrada depois com `client/registerCapability`.

| Campo | Valor (cliente sem registro dinâmico) | Fonte |
|---|---|---|
| `textDocumentSync` | `{openClose:true, change:2, willSave:false, willSaveWaitUntil:false}` (sem `save`) | `handler_text_document_changes.dart:162-167` |
| `completionProvider` | `{triggerCharacters:[".","=","(","$","\"","'","{","/",":"], resolveProvider:true, completionItem:{labelDetailsSupport:true}}` (`allCommitCharacters:["("]` só com `dart.previewCommitCharacters`) | `handler_completion.dart:881-888`; `constants.dart:23-37` |
| `hoverProvider` | `true` | `handler_hover.dart:141` |
| `signatureHelpProvider` | `{triggerCharacters:["("], retriggerCharacters:[","]}` | `handler_signature_help.dart:154-157`; `constants.dart:40-43` |
| `definitionProvider`, `typeDefinitionProvider`, `implementationProvider`, `referencesProvider`, `documentHighlightProvider`, `documentSymbolProvider`, `workspaceSymbolProvider`, `foldingRangeProvider`, `selectionRangeProvider`, `callHierarchyProvider`, `typeHierarchyProvider` | `true` | handlers respectivos |
| `codeActionProvider` | com `codeActionLiteralSupport`: `{codeActionKinds:["source","source.organizeImports","source.fixAll","source.sortMembers","quickfix","refactor"]}`; senão `true` | `handler_code_actions.dart:268-276`; `constants.dart:254-262` |
| `codeLensProvider` | `{}` | `handler_code_lens.dart:73-74` |
| `documentLinkProvider` | `{resolveProvider:false}` | `handler_document_link.dart:70-71` |
| `colorProvider` | `{documentSelector:[{language:"dart",scheme:"file"}, …]}` | `handler_document_color.dart:75-82` |
| `documentFormattingProvider`, `documentRangeFormattingProvider` | `true` (com `dart.enableSdkFormatter`, padrão true) | `handler_formatting.dart:78-84`; `handler_format_range.dart:79-85` |
| `documentOnTypeFormattingProvider` | `{firstTriggerCharacter:"}", moreTriggerCharacter:[";"]}` | `handler_format_on_type.dart:84-88` |
| `renameProvider` | `{prepareProvider:true}` com `rename.prepareSupport`; senão `true` | `handler_rename.dart:318-320` |
| `inlayHintProvider` | `{resolveProvider:false}` | `handler_inlay_hint.dart:79-81` |
| `semanticTokensProvider` | `{legend, full:{delta:false}, range:true}` | `handler_semantic_tokens.dart:173-179` |
| `executeCommandProvider` | `{commands:[…13], workDoneProgress:true}` | `handler_execute_command.dart:108-114` |
| `workspace.workspaceFolders` | `{supported:true, changeNotifications:true}` | `server_capabilities_computer.dart:198-201` |
| `workspace.fileOperations` | `{willRename:{filters:[{scheme:"file",pattern:{glob:"**/*.dart",matches:"file"}},{scheme:"file",pattern:{glob:"**/",matches:"folder"}}]}}` | `constants.dart:57-74`; `handler_will_rename_files.dart:117-127` |
| `experimental` | `{"textDocument":{"super":{},"augmented":{},"augmentation":{}}}` | `server_capabilities_computer.dart:208-225` |
| `diagnosticProvider` (pull), `declarationProvider`, `linkedEditingRangeProvider`, `positionEncoding` | ausentes | `:169-226` |

Comandos (`constants.dart:99-111`; `refactoring_processor.dart:20-28`), na
ordem: `dart.edit.sortMembers`, `dart.edit.organizeImports`,
`dart.edit.fixAll`, `dart.edit.fixAllInWorkspace.preview`,
`dart.edit.fixAllInWorkspace`, `dart.edit.sendWorkspaceEdit`,
`refactor.perform`, `refactor.validate`, `dart.logAction`,
`dart.refactor.convert_all_formal_parameters_to_named`,
`dart.refactor.convert_selected_formal_parameters_to_named`,
`dart.refactor.move_selected_formal_parameters_left`,
`dart.refactor.move_top_level_to_file`.

Legenda dos tokens semânticos (`AS:src/lsp/semantic_tokens/legend.dart:34-50`,
`semantic_tokens/mapping.dart:10-179`, `constants.dart:217-247`): tipos
`annotation, keyword, class, comment, method, variable, parameter, enum,
enumMember, type, source, property, namespace, boolean, number, string,
function, typeParameter`; modificadores `documentation, constructor,
declaration, importPrefix, instance, static, escape, annotation, control,
label, interpolation, void, wildcard`.

**Estado no DartForge** (`crates/lsp/src/servidor.rs`, `initialize`): anuncia
`textDocumentSync` incremental, completion (`triggerCharacters` `.` e outros
que o servidor trata, `resolveProvider:true`), hover, signatureHelp (`(`/`,`),
definition, typeDefinition, implementation, references, documentHighlight,
documentSymbol, workspaceSymbol, codeAction (os seis kinds do Dart),
rename com `prepareProvider`, foldingRange, selectionRange, call/type
hierarchy, inlayHint, semanticTokens (legenda idêntica à do 3.6.2, `full` e
`range`) e, se o cliente aceita, `diagnosticProvider` (pull, LSP 3.17, que o
Dart não tem). **Não anuncia:** formatting/rangeFormatting/onTypeFormatting
(decisão L08), codeLens, documentLink, colorProvider, executeCommand,
`workspace.fileOperations`, `experimental`. Capacidades sempre estáticas (sem
`client/registerCapability`). **Diverge** nesses itens ausentes e no
`diagnosticProvider` a mais.

### 2.2 `initialized`, configuração, pastas, encerramento e cancelamento

* `initialized` (`handler_initialized.dart:25-47`): passa ao estado
  inicializado (`initialize` repetido → `ServerAlreadyInitialized (-32002)`);
  busca a configuração (`workspace/configuration`) e faz o registro dinâmico.
  **Antes dele, requisições recebem `ServerNotInitialized (-32002)` e
  notificações (inclusive `didOpen`) são descartadas** (`handler_states.dart:175-187`).
* Raízes (`lsp_analysis_server.dart:1101-1137`): as pastas do workspace; sem
  pastas, a raiz do pacote de cada arquivo aberto. `dart.analysisExcludedFolders`
  relativo é juntado a cada pasta.
* `workspace/configuration` (`lsp_analysis_server.dart:317-377`): pede
  `[{scopeUri: pasta_i, section:'dart'}…, {section:'dart'}]`; só aceita lista
  com `pastas+1` itens. Chaves lidas (`client_configuration.dart`): globais
  `analysisExcludedFolders`, `codeLens`, `completeFunctionCalls` (false),
  `experimentalRefactors` (false), `includeDependenciesInWorkspaceSymbols`
  (true), `documentation` (`none`/`summary`/`full`), `previewCommitCharacters`
  (false), `showTodos`; por recurso `enableSdkFormatter` (true),
  `enableSnippets` (true), `lineLength`, `maxCompletionItems` (2000),
  `renameFilesWithClasses` (`never`), `updateImportsOnRename` (true).
  `didChangeConfiguration` ignora `settings` e busca de novo
  (`handler_workspace_configuration.dart:22-31`).
* `didChangeWorkspaceFolders` (`handler_change_workspace_folders.dart:28-53`).
* `shutdown` responde `null` e passa a recusar requisições com `InvalidRequest
  (-32600)`; `exit` sai com 0 depois de `shutdown`, senão 1
  (`handler_shutdown.dart:20-31`, `handler_exit.dart:27-40`).
* `$/cancelRequest` (`handler_cancel_request.dart:9-45`): cancela o token; a
  resposta vira `RequestCancelled (-32800) 'Request was cancelled'`.
* Notificações do servidor: `$/analyzerStatus {isAnalyzing}` ou `$/progress`
  `ANALYZING` (`lsp_analysis_server.dart:836-871`);
  `dart/textDocument/publishClosingLabels`, `publishOutline`,
  `publishFlutterOutline` só com as opções de inicialização correspondentes.

**Estado no DartForge:** `shutdown`/`exit` com os mesmos códigos de saída;
`$/cancelRequest` com `-32800` (também para a requisição já na fila);
requisição antes de `initialize` recusada. **Diverge:** não lê
`workspace/configuration` (nenhuma chave `dart.*` tem efeito), não trata
`didChangeWorkspaceFolders` (o projeto é o pacote do arquivo aberto, com ou
sem pastas), não publica `$/analyzerStatus`/`$/progress`, `closingLabels` e
`outline`; aceita `didOpen` antes de `initialized`.

## 3. Sincronização e diagnósticos

* `didOpen` (`handler_text_document_changes.dart:110-134`): grava a versão e o
  overlay; conteúdo igual ao do disco não reanalisa.
* `didChange` (`:27-61`, `AS:src/lsp/source_edits.dart:30-79`): aplica as
  mudanças em sequência; documento não aberto → `-32099`, que derruba o
  servidor (seção 1).
* `didClose` (`:76-95`): remove o overlay; **os diagnósticos não são limpos**
  (o arquivo continua analisado se estiver nas raízes).
* `didSave`: sem handler (`textDocumentSync` não anuncia `save`); a
  notificação gera `MethodNotFound` → `window/showMessage`.
* `publishDiagnostics` (`AS:src/analysis_server.dart:1097-1129`,
  `AS:src/protocol_server.dart:30-33`, `:98-122`, `:127-176`;
  `lsp_analysis_server.dart:1268-1291`, `:709-729`): para **todos** os arquivos
  analisados das raízes (não só os abertos), filtrados pelo `ErrorProcessor`
  das `analysis_options` e pela regra de TODO (`dart.showTodos`); erros do
  servidor e de plugins concatenados; sem lista vazia repetida; sem `version`.
  Cada `Diagnostic` (`mapping.dart:602-649`): `range`; `severity` ERROR→1,
  WARNING→2, INFO→3 (nunca `Hint`); `code` = nome do código em minúsculas;
  `codeDescription.href` = `https://dart.dev/diagnostics/<código>` (só com
  `codeDescriptionSupport` e `hasPublishedDocs`); `source:"dart"`; `message` =
  mensagem + `"\n" + correction`; `tags` `[1]` (Unnecessary) para `dead_code`
  e `[2]` (Deprecated) para os `deprecated_member_use*` (filtradas pelo
  `tagSupport`); `relatedInformation` com cada `contextMessage`.

**Estado no DartForge** (`docs/LSP.md`, "Diagnósticos tipados"): publica para
os documentos **abertos** (o Dart publica para todos os arquivos das raízes) o
sintático imediatamente e o tipado em seguida, só com os códigos de
`crates/analise/verificados.txt` (os de paridade conferida no corpus); no
`didClose` publica lista vazia (o Dart mantém). `code`, `severity`,
`message`/`correction`, `tags` e `codeDescription` seguem o Dart para os
códigos publicados. **Diverge:** conjunto de arquivos (abertos × raízes), a
lista vazia no fechamento, os códigos não publicados (paridade do
analisador: trabalho dos outros agentes) e o `diagnosticProvider` (pull) que
o Dart não tem.

## 4. `textDocument/documentSymbol`

**Algoritmo** (`AS:src/lsp/handlers/handler_document_symbols.dart:32-146`):
unidade **resolvida**; `DartUnitOutlineComputer(unit).compute()`
(`AS:src/computer/computer_outline.dart:22-71`). Com
`hierarchicalDocumentSymbolSupport`, `DocumentSymbol[]` (unidade sem
declarações → **`null`**, `:107-110`); senão `SymbolInformation[]` em
pré-ordem (`:121-144`).

**Outline.** Unidade (`computer_outline.dart:25-68`), em ordem de fonte:
classe → CLASS; mixin → MIXIN; enum → ENUM com **todas as constantes antes
dos membros**; extensão → EXTENSION (sem nome: location no `extendedType`);
extension type; uma TOP_LEVEL_VARIABLE **por variável**; função → FUNCTION,
GETTER ou SETTER; `ClassTypeAlias` → CLASS_TYPE_ALIAS; `FunctionTypeAlias`
→ FUNCTION_TYPE_ALIAS; `GenericTypeAlias` → FUNCTION_TYPE_ALIAS se o alias é
`GenericFunctionType`, senão TYPE_ALIAS. Membros (`:407-430`): construtor
(nome `A` ou `A.named`, `:131-156`); um FIELD por variável; método → METHOD,
GETTER ou SETTER. Corpos (`:450-548`) contribuem as **funções locais**
(recursivo, inclusive em closures) e `group(...)`/`test(...)` do `package:test`
(`'group("descrição")'`).

**Campos.** `name` (`toElementName`, `mapping.dart:1211-1217`; nome vazio →
`<unnamed extension>`/`<unnamed>`); `detail` = `parameters.toSource()`
(função/construtor: `"(int a, {String? b})"`; getter top-level `""`; getter de
classe, campos e classes: null); `kind` por `elementKindToSymbolKind`
(`mapping.dart:340-414`):

| ElementKind | SymbolKind (sem `valueSet` do cliente) |
|---|---|
| CLASS, CLASS_TYPE_ALIAS, MIXIN, FUNCTION_TYPE_ALIAS | 5 |
| ENUM | 10 |
| ENUM_CONSTANT | 22, ou **10** sem `valueSet` |
| EXTENSION, EXTENSION_TYPE, LIBRARY | 3 |
| FIELD | 8 |
| CONSTRUCTOR | 9 |
| METHOD, UNIT_TEST_* | 6 |
| GETTER, SETTER | 7 |
| FUNCTION | 12 |
| TOP_LEVEL_VARIABLE, LOCAL_VARIABLE, PARAMETER, PREFIX | 13 |
| TYPE_PARAMETER | 26, ou 13 sem `valueSet` |
| TYPE_ALIAS (`typedef A = int;`) | **19 (Obj)**: ausente do `switch` |

`deprecated` = `hasDeprecated`; `range` = *code range* (`codeOffset =
firstTokenAfterCommentAndMetadata`, **sem** doc comment e anotações; para
variáveis e campos só `nome = inicializador`, `:396-404`); `selectionRange` =
nome; `children` (vazio → null). Plano: `location.range` = **nome**;
`containerName` = nome cru do pai.

**Estado no DartForge** (`crates/lsp/src/simbolos.rs`): **97%** (34 arquivos,
1 diferente). Mesma árvore, nomes `A.named`, espécies (extensão 3, getters 7,
variáveis 13, campos 8, `EnumMember`→`Enum` e `TypeParameter`→`Variable` sem
`valueSet`). **Diverge:** funções **locais** não entram (o Dart as lista como
filhos da função: `isWhitespace` em `args/lib/src/utils.dart`), testes
`group`/`test` não viram símbolos, `typedef A = int;` sai como 5 e não 19.

## 5. `workspace/symbol`

**Algoritmo** (`AS:src/lsp/handlers/handler_workspace_symbols.dart:28-123`;
`AN:src/dart/analysis/search.dart:170-195`, `:1057-1135`, `:1246-1300`):
query vazia → `[]`; limite de **500** resultados; com
`includeDependenciesInWorkspaceSymbols` (padrão) chama
`discoverAvailableFiles()` (`AN:src/dart/analysis/driver.dart:654-697`), que
traz **todas as bibliotecas do SDK** e os `lib/` dos pacotes; primeiro os
arquivos analisados, depois os conhecidos. Pré-filtro `nameUnion` (letras da
query em ordem, sem caixa). Por unidade, na ordem: acessores top-level;
classes (a classe, acessores, construtores, campos, métodos); enums; mixins;
extensões (as sem nome não, os membros sim); extension types; funções;
typedefs; variáveis. Casamento `FuzzyMatcher(query, MatchStyle.TEXT).score
>= 0` (subsequência sem caixa, `AN:src/utilities/fuzzy_matcher.dart:137-159`,
`:375-484`); **sem ordenar por score**.

**Formato.** `name` = nome + `"()"` (parâmetros `()`), `"(…)"` (outros), nada
(getter); construtor nomeado aparece só como `named(…)`; `kind`
(`mapping.dart:215-266`): classe, mixin, **extensão**, extension type,
typedefs → 5; construtor 9; enum 10; constante 22 (10 sem `valueSet`); campo
8; função 12; getter/setter 7; método 6; variável 13; `location.range` =
**code range** (não o nome); `containerName` = classe ou mixin (null para
enum, extensão e topo).

**Estado no DartForge** (`crates/lsp/src/indice.rs`, `simbolos.rs`): **0%**
nas 24 consultas (`Arg`, `parse`, `Scanner`, `ctx`, `Ponto`, `join` nos 4
projetos); responde em 8 das 24. Procura só nos arquivos do projeto, por
subsequência, e devolve o intervalo do **nome**. **Diverge (todas as 24):**
o Dart inclui o SDK inteiro e os pacotes (centenas de nomes do
`html_dart2js.dart`, `_internal/…`), corta em 500, usa o code range e o
sufixo `(…)`. Fechar a diferença exige indexar as declarações de todas as
bibliotecas do SDK (inclusive `dart:html`, `_internal/js_runtime`) e das
dependências — só sintaxe, sem inferência — e imitar a ordem de coleta.

## 6. `textDocument/foldingRange`

**Algoritmo** (`AS:src/lsp/handlers/handler_folding.dart:28-135`,
`AS:src/computer/computer_folding.dart`): unidade **apenas parseada**; regiões
de `DartUnitFoldingComputer`, ordenadas por offset; `kind`: `comment`
(comentários e cabeçalho), `imports` (diretivas), demais sem kind
(`mapping.dart:1251-1265`). Uma região por linha inicial (`_addRegion`,
`:167-197`; com `fallbackStart` quando a linha já tem região), só regiões de
mais de uma linha; ordem de inserção: pré-ordem da AST, depois diretivas,
depois comentários. Regiões (início → fim): anotações (`annotations.first.name.end`
→ `annotations.last.end`); corpo de classe (`name.end` → `}`), de mixin
(`name.end` → fim), de enum/extensão/extension type (`{`+1 → `}`);
construtor/função/método (`name.end` → fim); `FunctionExpression` com bloco
(`{` → `}`); `FormalParameterList` e `ArgumentList` (`(`+1 → `)`, fallback no
primeiro parâmetro/argumento); `assert`; literais de lista/conjunto/mapa/
registro; strings; blocos de `if`/`while`/`do` (`{`+1 → fim do último
comentário antes do `}`, senão do último comando); `for` (`{` → `}`); `switch`
(`{`+1 → `}`) e casos (`:` → fim); casos de `switch` de expressão (`=>` →
fim); diretivas (`keyword.end` da primeira → fim da última, com 2+). Comentários
(`:89-161`): `/* */` do primeiro fim de linha ao fim; `//`/`///` consecutivos
do mesmo tipo e sem linha em branco, do fim do primeiro ao fim do último;
cabeçalho do arquivo como `comment`. Com `lineFoldingOnly`
(`handler_folding.dart:97-122`): sem caracteres, e a região que termina na
linha em que a seguinte começa encolhe (ou some).

**Estado no DartForge** (`crates/lsp/src/estrutura.rs`): **100%** (36/36
arquivos iguais).

## 7. `textDocument/selectionRange`

**Algoritmo** (`AS:src/lsp/handlers/handler_selection_range.dart:29-80`,
`AS:src/computer/computer_selection_ranges.dart:21-47`): unidade **apenas
parseada** (as construções que só a resolução reescreve continuam na forma do
parser: `A()` e `A.b()` são `MethodInvocation`); para cada posição,
`NodeLocator(offset)` e os ancestrais até antes da `CompilationUnit`,
registrando `(offset, length)` só se diferente do anterior. Lista vazia →
intervalo vazio na posição. Uma linha inválida falha a requisição toda.

`NodeLocator` (`AN:src/dart/ast/utilities.dart:1733-1870`): o nó mais profundo
com `offset <= pos <= end` (fim inclusivo; tokens sintéticos de comprimento 0
ignorados); entre filhos, **o primeiro que cobre** vence; com o cursor
exatamente em `name.end` de classe, construtor, função ou método, devolve a
declaração. O intervalo de uma declaração (`AnnotatedNode`) começa no doc
comment ou na primeira anotação (`AN:src/dart/ast/ast.dart:147-163`).

**O que é nó** (analyzer 6.11): nomes declarados são **tokens** (classe,
método, função, construtor, variável, parâmetro, constante de enum, parâmetro
de tipo) — o cursor no nome seleciona a declaração; `NamedType` (nome é token;
`p.` é `ImportPrefixReference`; `<…>` é `TypeArgumentList`); referências são
nós (`SimpleIdentifier`, `PrefixedIdentifier`, `PropertyAccess`,
`MethodInvocation`, `ConstructorName`, `InstanceCreationExpression`);
`ArgumentList` e `FormalParameterList` incluem os parênteses (a lista de
parâmetros inclui os `[ ]`/`{ }` de dentro); `TypeParameterList`; parâmetros
(`SimpleFormalParameter`, `FieldFormalParameter`, `SuperFormalParameter`,
`FunctionTypedFormalParameter`; opcionais/nomeados dentro de
`DefaultFormalParameter`); corpos (`BlockFunctionBody` ⊃ `Block`, com o
`async` incluído; `ExpressionFunctionBody` = `=> e;` com o `;`;
`EmptyFunctionBody`); `VariableDeclaration` ⊂ `VariableDeclarationList` ⊂
declaração/comando (com `;`); `FunctionDeclarationStatement` ⊃
`FunctionDeclaration` ⊃ `FunctionExpression`; só o **doc comment** é nó
(`Comment`, com `CommentReference` por `[ref]`); diretivas com `uri`
(`SimpleStringLiteral`), `prefix` (`SimpleIdentifier`), `Configuration`,
`ShowCombinator`/`HideCombinator` com `SimpleIdentifier`; cláusulas
`ExtendsClause`, `ImplementsClause`, `WithClause`, `MixinOnClause`,
`ExtensionOnClause`, `RepresentationDeclaration`, `EnumConstantArguments`,
`Annotation`; `ExpressionStatement` inclui o `;`; nas partes de `for`,
`ForPartsWithDeclarations`/`ForEachPartsWithDeclaration`/`DeclaredIdentifier`.

**Estado no DartForge** (`crates/lsp/src/estrutura.rs`, `selecoes`): **48%**
(410 posições). Os intervalos saem dos nós da árvore do parser do DartForge,
que não tem vários nós do analyzer. **Diverge** (213 posições), por falta de:
`TypeArgumentList`/`TypeParameterList` (`<int>`), `VariableDeclarationList` de
campos e variáveis de topo, `ExpressionFunctionBody`/`BlockFunctionBody`
(`=> …;`, `async {…}`), `FormalParameterList` com os parênteses (o
DartForge usa o grupo `{…}`), `ShowCombinator`/`HideCombinator` e os nomes
deles, a URI e o prefixo das diretivas, `Annotation` e o nome dela,
`SimpleIdentifier` do nome da classe num construtor (`returnType`), partes de
`for`. **Plano:** uma árvore no formato do analyzer construída a partir da
árvore do parser (os nós e intervalos acima), usada também pelas
assistências e refatorações que partem do `NodeLocator`.

## 8. Formatação, cores, CodeLens, links e arquivos

### 8.1 `textDocument/formatting`, `rangeFormatting`, `onTypeFormatting`

**Algoritmo** (`AS:src/lsp/handlers/handler_formatting.dart:29-61`,
`handler_format_range.dart:29-61`, `handler_format_on_type.dart:29-62`,
`AS:src/lsp/source_edits.dart:81-648`): `options` (tabSize, insertSpaces) é
**ignorado**. Não-Dart, `dart.enableSdkFormatter == false`, erro de
scan/parse (`result.errors` não vazio) ou `FormatterException` → **`null`**.
`DartFormatter(pageWidth: dart.lineLength ?? 80, languageVersion: …)` (na
unidade parseada o elemento é nulo, então vale a versão mais recente do
`dart_style` empacotado). Saída igual à entrada → `null`. Senão
`generateMinimalEdits`: re-tokeniza original e formatado (com comentários),
compara o espaço entre pares de tokens, aceita `,`/`;` adicionados ou
removidos e as trocas `>>>`/`>>`/`<<`/`[]`; edição só de `[\s,;<>]*` (salvo
comentários); fallback = uma edição do documento inteiro (em range
formatting, `[]`). `rangeFormatting` só mantém as edições dentro do
intervalo. `onTypeFormatting` (`}` e `;`) formata **o arquivo inteiro**.

**Estado no DartForge:** não implementado (decisão L08 em `docs/LSP.md`: o
formatador de referência é o `dart_style`, com regras por versão de
linguagem; só entra um porte conferido byte a byte num oráculo).
**Oráculo:** 36 arquivos, todos já formatados: o Dart responde `null` nos 36,
o DartForge responde `-32601`. **Irredutível sem o porte do `dart_style`**;
responder `null` sem formatar mentiria para arquivos não formatados.

### 8.2 `documentColor` / `colorPresentation`

**Algoritmo** (`handler_document_color.dart:38-67`,
`AS:src/computer/computer_color.dart`, `handler_document_color_presentation.dart:43-260`):
só expressões de tipo `Color` de `dart:ui` (Flutter); fora do Flutter nada.
Reconhece `Color(0xAARRGGBB)`, `Color.from(…)`, `Color.fromARGB`,
`Color.fromRGBO`, `ColorSwatch`, `MaterialAccentColor`, constantes e
`shadeNNN`; `colorPresentation` devolve 4 formas (`fromARGB`, `fromRGBO`,
`from`, `Color(0x…)`).

**Estado no DartForge:** não implementado. **Oráculo:** nenhuma amostra (os
projetos não usam Flutter). **Pendente** (só faz sentido com Flutter
carregado).

### 8.3 `textDocument/codeLens`

**Algoritmo** (`handler_code_lens.dart:32-61`,
`code_lens/augmentations.dart`): só "Go to Augmented"/"Go to Augmentation"
(augmentations, experimental), e só se o cliente lista `dart.goToLocation`
em `experimental.commands`. Caso contrário `[]`.

**Estado no DartForge:** não anuncia. Sem efeito no oráculo (o Dart devolve
`[]` sem augmentations). **Pendente** junto com augmentations.

### 8.4 `textDocument/documentLink`

**Algoritmo** (`handler_document_link.dart:26-53`; visitor em
`AP:utilities/navigation/document_links.dart`): unidade parseada; liga as URIs
de diretivas a arquivos existentes e, em comentários do Flutter, `** See code
in examples/api/…dart`. `target` sempre `file:`.

**Estado no DartForge:** não anuncia. **Oráculo:** 36 arquivos, o Dart
devolve `[]` em todos (100% igual). **Pendente:** os links das diretivas.

### 8.5 `workspace/willRenameFiles`

**Algoritmo** (`handler_will_rename_files.dart:37-102`):
`MoveFileRefactoringImpl.multi`; devolve um `WorkspaceEdit` que atualiza
`import`/`export`/`part` dos arquivos afetados; só com
`dart.updateImportsOnRename`.

**Estado no DartForge:** não implementado. **Pendente.**

### 8.6 `workspace/executeCommand` e métodos `dart/*`

Ver a seção 13 (ações). Métodos próprios: `dart/textDocument/super`,
`augmented`, `augmentation` (`AS:src/lsp/handlers/custom/abstract_go_to.dart:28-78`),
`dart/reanalyze`, `dart/diagnosticServer`, `dart/textDocumentContent`.

**Estado no DartForge:** nenhum. **Pendente:** `dart/textDocument/super`
(o "ir para o super" do Dart-Code) é o único usado pelo editor comum.

## 9. `textDocument/hover`

**Algoritmo** (`AS:src/lsp/handlers/handler_hover.dart:34-126`,
`AS:src/computer/computer_hover.dart:37-361`):
1. `node = NodeLocator(offset)`; `null` → sem hover.
2. `locationEntity` (`:186-203`), que dá o `range`: `.name` de membro de
   unidade, extensão, parâmetro, método, `DeclaredIdentifier`,
   `VariableDeclaration`, padrões; `name2` de `NamedType`; `name ?? returnType`
   de construtor; o próprio nó para `Expression`; `library` de `LibraryDirective`.
3. `_targetNode` (`:219-235`): o nome do tipo (ou o prefixo `p`) de uma
   criação e o `.named` dela viram a `InstanceCreationExpression`; o `A` de
   `A.named(...)` numa declaração vira a `ConstructorDeclaration`.
4. Só seguem `CompilationUnitMember`, `Expression`, `FormalParameter`,
   `MethodDeclaration`, `NamedType`, `ConstructorDeclaration`,
   `DeclaredIdentifier`, `VariableDeclaration`, padrões e `LibraryDirective`
   (`:49-60`); espaço ou palavra-chave dentro de uma classe dá o hover da
   classe.
5. `range` (`_hoverRange`, `:132-148`): criação → `constructorName` (`p.A.named`);
   declaração de construtor → do `returnType` ao fim do nome; senão o
   `locationEntity`.
6. `element = ElementLocator.locate(node)` (`AN:src/dart/ast/element_locator.dart:14-290`):
   anotação sem construtor → `annotation.element`; `returnType` de construtor →
   o construtor; `LibraryIdentifier` → unidade/biblioteca; criação →
   construtor; `MethodInvocation` → `methodName.staticElement`; `NamedType` →
   elemento; URI de import/export → biblioteca; `[ref]` de doc e `show`/`hide`
   são `SimpleIdentifier` (`writeOrReadElement`); rótulo `nome:` →
   `ParameterElement`.
7. Com elemento: accessor → `nonSynthetic` (getter sintético → campo; um
   `Member` vai para a **declaração**, por isso getters mostram o tipo
   declarado `T get first` e métodos o substituído `void add(int value)`),
   exceto o `values` de enum. `elementDescription =
   getDisplayString(multiline: true)`, com `'(const) '`/`'(new) '` numa criação
   **sem** palavra-chave (`:113-126`). Linha da biblioteca (`_libraryInfo`,
   `:151-180`) só para elementos **não locais** (`enclosingElement3 is!
   ExecutableElement`): `file:` → caminho relativo à raiz com `/`; outros →
   a URI (`dart:core`, `package:app/foo.dart`).
8. `staticType` (`:238-259`), em ordem: (a) nó `Expression` e elemento nulo,
   variável ou accessor → tipo da declaração/referência (`:363-393`: nome em
   declaração ou rótulo nomeado → `element.type`; alvo de atribuição →
   `writeType`; senão `staticType`); (b) `VariableElement` → `element.type`;
   (c) `methodName` de `MethodInvocation` → `staticInvokeType` (nada se
   `dynamic`); (d)/(e) padrões → `matchedValueType`; senão nada (tear-offs,
   classes e construtores não têm "Type").
9. Documentação (`:275-341`): `FieldFormalParameterElement` → o campo;
   **outro parâmetro → a função/construtor dele**; candidatos: o elemento, os
   sobrescritos (`findOverriddenElements`), o `variable2` de um accessor;
   vence o primeiro com `documentationComment`; setter sem doc usa o getter.
   `processDartdoc` (`AN:src/dartdoc/dartdoc_directive_info.dart:63-185`)
   limpa `///`/`/** */`, expande `{@macro}` e `{@youtube}`. Doc herdada de
   outro elemento ganha `"\n\nCopied from `Classe`."` (só em `full`).
   `dart.documentation`: `none`/`summary` (até a primeira linha em branco)/`full`.
10. `toHover` (`handler_hover.dart:53-112`): sem hover para **prefixo de
    import**; markdown abaixo, `trimRight`; sem `hover.contentFormat`, string
    pura; senão `MarkupContent` (`markdown`, ou `plaintext` se o cliente só
    aceita texto).

```
```dart
[(deprecated) ][(new) |(const) ]<elementDescription>
```
Type: `<staticType>`

*<biblioteca>*

---
<dartdoc limpo>
```

`ElementDisplayStringBuilder` (`AN:src/dart/element/display_string_builder.dart`):
classe `[sealed |abstract ][base |interface |final ][mixin ]class N<T extends
B> extends S with M implements I` (sem `extends Object`); enum, mixin
(`on`), extensão (`extension N<T> on T`), extension type, typedef; executável
`<retorno> <nome><T>(params)` (setter sem retorno, getter sem parênteses,
`get x`/`set x`); construtor `A A()`/`A<int> A.named(int x)`; variável `tipo
nome` (sem `final`/`const`); parâmetro isolado `int x`/`[int x = 0]`/`{int
x}`/`{required int x}`; parâmetros em **várias linhas com 3 ou mais**
(`"void f(\n  int a,\n  int b, [\n  int c = 0,\n])"`).

**Estado no DartForge** (`crates/lsp/src/descricao.rs`, `servidor.rs`):
texto inteiro **86%**, assinatura **97%** (394 posições com hover do Dart; o
DartForge responde em 392). Implementado: descrição no formato do
`ElementDisplayStringBuilder`, `(new) `/`(deprecated) `, linha `Type:` para
variáveis, getters em uso e invocações, linha da biblioteca, documentação com
herança e "Copied from", documentação da função para parâmetros.
**Diverge (57 posições):**
* parâmetro `this.x`: o DartForge omite o `Type:` (o Dart mostra o tipo do
  parâmetro) — regressão da rodada anterior;
* nome do campo num inicializador `x = …` de construtor: sem hover;
* documentação de **parâmetro de método que sobrescreve** (deve vir a doc
  herdada da função, "Copied from …"): falta;
* documentação de **campo herdado** e de getter que sobrescreve um campo: o
  DartForge herda (o Dart não, porque o accessor sintético não tem doc e
  campos não procuram sobrescritos);
* métodos e construtores do SDK cuja declaração o motor carrega do **patch**
  (`core_patch.dart`): sem documentação (`StringBuffer.toString`,
  `List.unmodifiable`, `RegExp`, `Deprecated`), e às vezes a forma do
  `external`;
* `InvalidType` (o motor diz `dynamic`) e alvo de atribuição composta
  (`i += 3` → `dynamic` em vez de `int`);
* um local num elemento de coleção (`if (…) j`) ganha a linha da biblioteca no
  Dart;
* parâmetro de tipo de método (`V` em `_create<V>`): o Dart não mostra nada.

## 10. `textDocument/definition`, `typeDefinition`, `implementation`

**definition** (`AS:src/lsp/handlers/handler_definition.dart:70-176`;
`AP:utilities/navigation/navigation_dart.dart:20-632`): `NodeLocator`; sobe
para a diretiva; `_getNavigationTargetNode` (`:51-80`) sobe enquanto o pai tem
o mesmo offset, usa o `FormalParameter` e o pai de `TypeArgumentList`. Alvo =
`element.nonSynthetic` (accessor sintético → campo; **construtor implícito →
classe**), `nameOffset/nameLength`. Casos: `ConstructorName` (prefixo →
`PrefixElement`; nome da classe → classe se o construtor é nomeado, senão o
construtor; `.nome` → construtor); `this.x` → **campo**; `super.x` →
parâmetro do super-construtor; `super(...)`/`this(...)` → construtor (ou
classe); `var` → classe do tipo inferido; URI de diretiva → biblioteca;
operadores. **O analyzer não aplica patches: o alvo no SDK é a declaração
pública em `sdk/lib/...`**. Com `linkSupport`: `LocationLink` com
`targetRange` = código sem doc/anotações (`:179-269`). `_filterResults`
(`:156-176`) tira alvos na mesma linha da posição se sobrar algum.

**typeDefinition** (`handler_type_definition.dart:44-204`): `NamedType` →
classe (`InterfaceElement`); `VariableDeclaration`/`DeclaredIdentifier`/
`FormalParameter` → tipo declarado; `Expression` → tipo estático (nome de
classe → `thisType`; nome em declaração/rótulo → `element.type`; setter →
tipo do campo); só `InterfaceType` e `TypeParameterType` têm destino. Sem
`linkSupport`: **um `Location`** (não lista).

**implementation** (`handler_implementation.dart:54-110`;
`AS:src/search/type_hierarchy.dart:156-233`;
`AS:src/services/search/search_engine_internal.dart:22-42`): classe-pivô do
elemento (ou da classe do membro); com membro, a declaração dele em cada
subtipo (DFS de subtipos em **todos os arquivos conhecidos, inclusive o SDK
inteiro e os `lib/` dos pacotes**); sem membro, os subtipos; o pivô não
entra; extension types sem membros.

**Estado no DartForge** (`crates/lsp/src/projeto.rs`, `hierarquia.rs`):
definition **96%**, typeDefinition **95%**, implementation **77%**.
**Diverge:**
* definition (16): alvos no SDK vão para o **patch** (`core_patch.dart`) em vez
  da declaração pública (o motor carrega o SDK com patches DDC); construtor
  (`A(...)` sem construtor escrito, `A.of`) vai para o construtor e não para a
  classe/nome do Dart; `azul` numa constante de enum; nomes de inicializador
  de construtor (`message`, `source` do super) apontam para o parâmetro local;
* typeDefinition (22): `@override` (o DartForge vai para `Object`; o Dart nada
  numa anotação), declaração de construtor (o Dart vai para a classe),
  campos declarados (o Dart vai ao tipo);
* implementation (94, quase todos no SDK): o Dart enumera as implementações
  nas bibliotecas internas do dart2js (`_internal/js_runtime/lib/js_number.dart`,
  `interceptors.dart`, `html_dart2js.dart`…), que o motor não carrega (usa o
  `js_dev_runtime`); campos `_line` privados de bibliotecas diferentes entram
  como família.

## 11. `textDocument/references` e `documentHighlight`

**references** (`AS:src/lsp/handlers/handler_references.dart:32-134`;
`AS:src/search/element_references.dart:19-90`;
`AS:src/services/search/hierarchy.dart:106-207`;
`AN:src/dart/analysis/search.dart:341-916`; `AN:src/dart/analysis/index.dart`):
`FieldFormalParameterElement` → campo; **todo accessor → `variable2`**.
Família: parâmetro nomeado → homônimos na hierarquia de métodos; membro de
classe (não estático, não construtor) → membros de mesmo nome nos supertipos
que o declaram e em todos os subtipos, mais `this.x` dos construtores;
**membros privados só na mesma biblioteca**. Escopo: arquivos que citam o
nome entre **todos os conhecidos (SDK e pacotes)**. Construtor: intervalos
`.nome` (com o ponto) ou **comprimento 0** depois do nome do tipo no sem
nome. Prefixo `p.` → `LibraryImportElement`. Com `includeDeclaration`, a
declaração (`nonSynthetic`) no fim. Sem dedupe.

**documentHighlight** (`handler_document_highlights.dart:28-63`;
`AS:src/domains/analysis/occurrences_dart.dart:13-205`): ocorrências do
elemento canônico (`this.x` → campo; accessor → `variable2`; `.declaration`)
no arquivo, **sem família de sobrescrita**; a declaração entra; `A()` e o
construtor sem nome destacam a **classe**.

**Estado no DartForge** (`Projeto::ocorrencias`, `familia`): references
**58%**, documentHighlight **95%**. **Diverge (references, 172):**
* 97 posições: referências no **SDK** (o Dart procura em todos os arquivos do
  SDK e dos pacotes: 8481 locais para `int`; o DartForge só no projeto
  carregado);
* 66 posições no projeto: construtor × classe (o DartForge junta as
  invocações `A()` às referências da classe; o Dart separa), rótulos
  `nome:` de argumentos nomeados de construtor em outros arquivos que faltam,
  prefixo de import (o Dart devolve o início da diretiva `import` como
  declaração e `p.` com o ponto), membros privados de bibliotecas diferentes
  entrando na família;
* documentHighlight (20): família de sobrescrita (o Dart não junta),
  parâmetro de tipo (o Dart não inclui a declaração em `T`), rótulo de campo
  num inicializador.

## 12. `prepareRename`, `rename`, call hierarchy e type hierarchy

**rename** (`AS:src/lsp/handlers/handler_rename.dart`;
`AS:src/services/refactoring/legacy/refactoring.dart:411-549`,
`rename*.dart`, `naming_conventions.dart`): `getElementToRename` escolhe o
nome por tipo de nó (operador no uso não é renomeável; `new`/`const`
renomeia a classe; rótulo `nome:` → parâmetro declarado; diretiva sem prefixo
→ intervalo vazio e placeholder `''`). `oldName`: construtor `element.name`
(**`''` no sem nome**), import o prefixo. `prepareRename`:
`checkInitialConditions` fatal → `RenameNotValid (-32010)`; resposta
`{range, placeholder}`. `rename`: `checkNewName` (FATAL/ERROR → `-32010`),
`checkFinalConditions` (ERROR/WARNING → prompt "Rename Anyway"/"Cancel" ou
`-32010` sem `showMessageRequest`), `createChange`, `documentChanges` com
versão. Elementos do SDK: `"The <kind> '<nome>' is defined in the SDK, so
cannot be renamed."`; fora do projeto: `"... is defined outside of the
project, so cannot be renamed."`; operadores `"Cannot rename operator."`.
Construtor: referências viram `.novo`; o sem nome ganha nome (`A()` →
`A.novo()`, inserindo `.novo` depois do nome da classe). Mensagens de nome
inválido (`_validateIdentifier`, `naming_conventions.dart:144-192`).

**call hierarchy** (`AS:src/computer/computer_call_hierarchy.dart`,
`handler_call_hierarchy.dart`): `_findTargetNode` (`:315-343`) — um
`SimpleIdentifier` cujo pai não é `VariableDeclaration` nem atribuição **sobe
para o pai**: cursor em `x` de `x.foo()` dá `foo`; em `a == b`, o operador
`==`; em `s.length`, `get length`; accessor sintético → nada; só executáveis.
Item: nome (`get x`, `A.named`, arquivo), kind (12/6/9/7, 5, 1/2), `detail` =
contêiner, `range` = código **com** doc, `selectionRange` = nome.
`incomingCalls`: referências da família agrupadas pelo contêiner;
`outgoingCalls`: chamadas, criações, getters explícitos.

**type hierarchy** (`AS:src/computer/computer_lazy_type_hierarchy.dart`,
`handler_type_hierarchy.dart`): alvo = o `NamedType` ancestral (com os
**argumentos e a nulabilidade escritos**: `List<String>`, `String?`) ou a
classe que contém o cursor; parâmetro de tipo/typedef → nada; nome =
`type.getDisplayString()`; `kind` sempre 5; `range` = código (com doc),
`selectionRange` = nome; **convertidos com o `LineInfo` do documento atual**
mesmo quando a classe está noutro arquivo (posições "erradas" mas
determinísticas). Supertipos: superclasse, `on`, `implements`, `with`;
subtipos diretos em todos os arquivos conhecidos.

**Estado no DartForge** (`crates/lsp/src/renomear.rs`, `chamadas.rs`,
`hierarquia.rs`): prepareRename **96%**, rename **89%**, prepareCallHierarchy
**86%**, prepareTypeHierarchy **76%**. **Diverge:**
* rename/prepareRename (45/16): construtor sem nome (o Dart transforma em
  nomeado, placeholder `''`, edição `.novo`; o DartForge renomeia a classe),
  `toString` sobrescrito (o Dart renomeia a família no projeto; o DartForge
  recusa porque a raiz é do SDK), prefixo de import (o Dart edita `p.` com o
  ponto e a diretiva), rótulos `nome:` em outros arquivos;
* call hierarchy (57): o DartForge pega o elemento do nome sob o cursor; o
  Dart sobe para a **invocação** que o contém (`codeUnitAt`, `==`,
  `get length`, `writeln`); o DartForge aceita getter e construtor
  implícito onde o Dart não aceita, e vice-versa;
* type hierarchy (98): o DartForge usa o nome declarado (`List<E>`) e o
  intervalo no arquivo da classe; o Dart usa o tipo escrito (`List<String>`,
  `String?`) e converte as posições com o `LineInfo` do arquivo atual; num
  parâmetro de tipo e em `export … show A` o Dart não responde; num método o
  Dart devolve a classe que contém o cursor.

## 13. `textDocument/codeAction` e `workspace/executeCommand`

### 13.1 Fluxo

`AS:src/lsp/handlers/handler_code_actions.dart:37-406`;
`AS:src/lsp/handlers/code_actions/dart.dart`,
`abstract_code_actions_producer.dart`.
* Arquivo não analisado ou não `file:` → `[]` (`:45-48`). Biblioteca não
  resolvida: sem o produtor Dart.
* `shouldIncludeKind(kind)` (`:64-87`): com `context.only`, `kind == w ||
  kind.startsWith('w.')`; sem `only`, os kinds anunciados pelo cliente
  (`codeActionKind.valueSet`). `shouldIncludeAnyOfKind` (`:94-111`) decide
  calcular source/quickfix/refactor. `triggerKind` só adiciona
  `autoTriggered` aos comandos de fonte.
* Literais × `Command`: fixes e assists só existem como literais; ações de
  fonte e refatorações legadas exigem `workspace.applyEdit` (`dart.dart:210-212`,
  `:318-320`) e vão como `CodeAction{title, kind, command}`.
* **Ordem da resposta** (`:213-244`): ações de fonte; quick fixes ordenados;
  assists ordenados; refatorações (as do `RefactoringProcessor`, depois
  Extract Method, Extract Local Variable, Extract Widget, Inline Local
  Variable, Inline Method, Convert Getter to Method, Convert Method to
  Getter). Ordenação (`_CodeActionSorter`, `:284-406`): **deduplicação por
  título** (vence a de diagnóstico mais perto da coluna pedida; mescla os
  diagnósticos de ações iguais), prioridade decrescente, chegada.
  `isPreferred` nunca é posto.
* **Diagnósticos por linha** (`dart.dart:160-171`): entra o erro cujas linhas
  intersectam as linhas do intervalo pedido.

### 13.2 Quick fixes

`PLUGIN:src/correction/fix_processor.dart:16-117`;
`PLUGIN:src/correction/fix_in_file_processor.dart:21-76`;
`AS:src/services/correction/fix_internal.dart:251-1902`;
`AS:src/services/correction/fix.dart`.
* Por erro: produtores do código (`lintProducers`/`nonLintProducers` e os
  multi), e, para `LintCode`/`HintCode`/`WarningCode`, os três "ignore"
  (erros de compilação e de sintaxe não). "Fix all in file" (kind *multi*,
  prioridade 40) com 2+ erros do mesmo código e produtor
  `canBeAppliedAcrossSingleFile`.
* `CodeAction{title = mensagem do FixKind, kind = toCodeActionKind(id,
  quickfix)` (`dart.fix.x` → `quickfix.x`, `mapping.dart:948-972`),
  `diagnostics:[d], command: dart.logAction, edit}`; snippets só com
  `experimental.snippetTextEdit`.
* Ignore: `Ignore '<código>' for this line` (30), `for the whole file` (29),
  ``in `analysis_options.yaml` `` (28) (`ignore_diagnostic.dart`).
* Prioridades: `Add required argument` 70; imports 55–51; `Change to`,
  `Create N missing override(s)` 51; padrão 50; `Create field/function/
  noSuchMethod` 49; em arquivo 40; extensões 30.
* Produtores dos códigos mais comuns: `unused_import` (`Remove unused
  import`), `unused_local_variable` (`Remove unused local variable`),
  `unused_element`, `unused_field`, `unused_catch_clause/stack`,
  `undefined_identifier/method/getter/class/function` (`Import library`,
  `Change to`, `Create class/field/getter/local variable/method/function/
  mixin/setter/extension method/getter`, `Create required positional
  parameter`), `missing_required_argument` (`Add required argument 'x'`),
  `non_exhaustive_switch_statement/expression` (`Add missing switch cases`),
  `non_abstract_class_inherits_abstract_member_*` (`Create N missing
  override(s)`, `Create 'noSuchMethod' method`, `Make class 'C' abstract`),
  `unnecessary_cast`, `dead_code` (`Remove dead code`), `invalid_assignment`,
  `argument_type_not_assignable`, `await_in_wrong_context` (`Add 'async'
  modifier`), `final_not_initialized`, `uri_does_not_exist` (`Create file`),
  `expected_token`, `unnecessary_non_null_assertion`, `unnecessary_question_mark`.

### 13.3 Assists

`AS:src/services/correction/assist_internal.dart:89-276`;
`AS:src/services/correction/assist.dart:44-491`. `node =
NodeLocator(offset, offset+length) ?? unit`; `getEnclosingFunctionBody()`
(`PLUGIN:edit/dart/correction_producer.dart:729-747`) = corpo da primeira
`FunctionExpression`, `FunctionDeclaration`, `ConstructorDeclaration` ou
`MethodDeclaration` ancestral (o intervalo de uma declaração inclui
anotações e doc: o cursor em `@override` está "dentro" do método). Um
produtor que também corrige um lint presente no nó não roda como assist.
`CodeAction{title, kind = refactor.<id>, diagnostics:[], command:
dart.logAction, edit}`. Prioridades: padrão 30, `Convert to async function
body` e `Remove type annotation` 31, Flutter 25–29, `Surround with` 31–38.
Regras dos comuns (arquivos em `AS:src/services/correction/dart/`):
* **Add type annotation** (`add_type_annotation.dart:43-205`): parâmetro sem
  tipo (Interface/Record), padrão declarado, literal de coleção sem `<T>`,
  lista de variáveis sem tipo com o cursor até o nome da primeira variável
  (tipo do inicializador ou LUB das atribuições), `for-in`.
* **Remove type annotation** (`remove_type_annotation.dart:39-140`):
  primeiro `DeclaredIdentifier`/`SimpleFormalParameter`/`SuperFormalParameter`/
  `VariableDeclarationList` ancestral; lista com tipo, cursor até o fim do
  nome, inicializador; não em literal de conjunto/mapa sem argumentos.
* **Convert to block body** (`convert_into_block_body.dart:47-176`): corpo
  envolvente `=>` (cursor **antes** da expressão) **ou vazio** (`;`: método
  abstrato, construtor sem corpo → `// TODO: implement <nome>` e `throw
  UnimplementedError();` se não `void`), não gerador.
* **Convert to expression body** (`convert_to_expression_function_body.dart:31-…`):
  bloco com um `return e;`/`e;`, sem comentários, cursor antes de `e`, não
  construtor gerador.
* **Convert to async function body** (`convert_into_async_body.dart:23-59`):
  sem `async`/`sync*`, cursor até o `{` (ou `=>`), não construtor nem closure.
* **Split variable declaration**, **Use curly braces**, **Assign value to new
  local variable**, **Convert to final field**, **Convert to getter**,
  **Convert to normal parameter**, **Convert to field formal parameter**,
  **Convert class to mixin**, **Convert to for-index loop**, **Invert
  conditional expression**, **Replace conditional with 'if-else'**,
  **Destructure variable assignment**, **Encapsulate field** (cursor no nome
  de campo público não final), **Add explicit 'show' combinator** (import sem
  combinador com nomes usados) — condições nos arquivos homônimos
  (`split_variable_declaration.dart:26`, `use_curly_braces.dart:43-72`,
  `assign_to_local_variable.dart:37-82`, `convert_into_final_field.dart:27`,
  `convert_into_getter.dart:25`, `convert_to_normal_parameter.dart:25-58`,
  `convert_to_field_parameter.dart:27-96`, `convert_class_to_mixin.dart:27`,
  `convert_into_for_index.dart:27`, `invert_conditional_expression.dart:24-100`,
  `replace_conditional_with_if_else.dart:25`,
  `destructure_local_variable_assignment.dart:33`, `encapsulate_field.dart:27`,
  `import_add_show.dart:30-58`).

### 13.4 Refatorações

`dart.dart:84-113`, `:217-301`;
`AS:src/services/refactoring/legacy/*.dart`;
`AS:src/services/refactoring/framework/refactoring_processor.dart:20-94`;
`AS:src/lsp/handlers/commands/{abstract_refactor,perform_refactor,refactor_command_handler}.dart`.
Formato: `CodeAction{title, kind: refactor.extract|inline|rewrite, command:
{command:'refactor.perform', arguments:[KIND, path, versão, offset,
comprimento, null]}}`; as do `RefactoringProcessor`, comando próprio e
`data.parameters`.
* **Extract Method** (`extract_method.dart:553-771`, `:985-1291`): offset > 0
  e antes do fim; seleção vazia na lista de parâmetros de uma *closure* ou no
  rótulo de um argumento-closure → a closure; senão a **menor `Expression`
  envolvente extraível**, subindo até o primeiro comando (inválidas: nome de
  declaração, nome de método/função isolado, prefixo, lado esquerdo de
  atribuição, `FunctionExpression` de declaração, tipo isolado); ou
  comandos inteiros. Execução: nome sugerido ou `newMethod`, só a seleção,
  declaração depois do membro envolvente (`=> e;` numa linha), parâmetros =
  locais referenciados.
* **Extract Local Variable** (`extract_local.dart:239-361`): `FunctionBody`
  envolvente; sobe pulando `ArgumentList`, atribuição, `NamedExpression`,
  `TypeArgumentList` e nomes de propriedade; para no primeiro não-`Expression`;
  invocação `void` é fatal; nome de função/método é pulado. Nome sugerido ou
  `newVariable`.
* **Inline Local Variable** (`inline_local.dart:55-189`): cursor num local
  (declaração ou uso) com inicializador, sem outra escrita, em bloco.
* **Inline Method** (`inline_method.dart:310-411`): declaração ou referência
  a executável não sintético, não operador, não gerador.
* **Convert Getter to Method** / **Convert Method to Getter**
  (`convert_getter_to_method.dart:82-93`, `convert_method_to_getter.dart:76-102`):
  getter escrito no projeto; função/método sem parâmetros e retorno não `void`.
* **Move '<nome>' to file** (`move_top_level_to_file.dart:173-454`): cliente
  com `documentChanges` e `resourceOperations: create`; cursor **no nome** de
  uma declaração de topo; título `Move 'X' to file`; cria
  `<dir>/<nome_snake>.dart` e ajusta imports.
* Experimentais (só com `dart.experimentalRefactors`): converter parâmetros em
  nomeados, mover parâmetros à esquerda.

### 13.5 Ações de fonte e comandos

`dart.dart:315-342`: com `workspace.applyEdit`, nesta ordem, `Sort Members`
(`source.sortMembers`, `dart.edit.sortMembers`), `Organize Imports`
(`source.organizeImports`, `dart.edit.organizeImports`), `Fix All`
(`source.fixAll`, `dart.edit.fixAll`), como `Command` com `[{path,
autoTriggered?}]`. `executeCommand` (`handler_execute_command.dart:30-111`;
`commands/simple_edit_handler.dart:23-104`) calcula e envia
`workspace/applyEdit{label, edit}` e responde `null`.
* **Sort Members** (`AS:src/services/correction/sort_members.dart`): unidade
  parseada, recusa com erro de sintaxe (`FileHasErrors -32008`); ordem `main`,
  constantes, variáveis, acessores, funções, typedefs, classes, extension
  types, extensões (públicos antes de privados); nas classes campos
  estáticos, acessores estáticos, campos, construtores, acessores, métodos,
  métodos estáticos; campos na ordem escrita, o resto pelo nome em
  minúsculas (`x getter`/`x setter`); cada membro com os comentários de cima
  e do fim da linha (`range.nodeWithComments`); diretivas organizadas sem
  remover; uma edição (prefixo/sufixo comuns).
* **Organize Imports** (`AS:src/services/correction/organize_imports.dart`):
  grupos `dart:`, `package:`, `://`, relativos (imports, depois exports),
  `part`; ordem `compareDirectiveUri`; linha em branco entre grupos;
  comentários acompanham; remove `unused_import`/`duplicate_import`/
  `unnecessary_import` (salvo com identificador não resolvido).
* **Fix All** (`commands/fix_all.dart:28-152`;
  `AS:src/services/correction/bulk_fix_processor.dart:271-800`, `:1040-1130`):
  até 4 passes; por erro, os produtores `canBeAppliedAcrossFiles`
  (`automatically` ou `acrossFiles`); edição conflitante descartada; sem
  outras edições, a fase de diretivas (remover imports não usados).

### 13.6 Estado no DartForge

`crates/lsp/src/acoes.rs`, `correcoes.rs`, `criar.rs`, `assistencias.rs`,
`ordenar.rs`, `semantica.rs`:
* **quick fixes para os diagnósticos do Dart: 75%** (12 diagnósticos; os títulos
  do Dart presentes nos do DartForge). Implementado: inserir `;`, importar
  biblioteca (SDK e projeto), as correções dos códigos publicados (seção
  correspondente em `docs/LSP.md`), criar membros, `Change to`, ignore por
  linha/arquivo, diagnósticos por linha. **Diverge:** `Add missing switch
  cases` e `Add required argument` faltam; a deduplicação por título e a
  ordem por prioridade não são imitadas; `Ignore … in analysis_options.yaml`
  falta.
* **assists no cursor: 26%** (377 posições com assist do Dart). **Diverge**
  (por título que falta): Extract Method 185, Inline Method 77, `Convert to
  block body` 46 (corpo vazio de métodos abstratos e construtores; o cursor
  em anotação/doc conta como "dentro" do método), `Move 'x' to file` 43,
  Extract Local Variable 25, `Remove type annotation` 22, `Add type
  annotation` 21, `Convert Getter to Method` 13, Inline Local Variable 13,
  `Convert to final field` 12, `Convert to async function body` 11, e
  `Convert to normal parameter`, `Convert class to a mixin`, `Convert Method
  to Getter`, `Add explicit 'show' combinator`, `Convert to for-index loop`,
  `Invert conditional expression`, `Replace conditional with 'if-else'`,
  `Destructure variable assignment`, `Encapsulate field`, `Convert to getter`
  (1–6 cada). A sobrar: `Convert to block body` 26 (cursor dentro da
  expressão), `Extract Local Variable` 6. Causa comum: o DartForge não tem o
  `NodeLocator` sobre a árvore do analyzer (seção 7) e decide pela árvore
  do parser.
* **ações de fonte: 0%** (416 posições: o Dart oferece `Sort Members` e `Fix
  All` além de `Organize Imports`; 6 posições em que o Dart responde
  `-32001` num prefixo de import). Em andamento: `Sort Members`
  (`crates/lsp/src/ordenar.rs`, o `MemberSorter`) e `Fix All` (as correções
  em lote do arquivo), como edição direta (não comando).
* `executeCommand`/`workspace/applyEdit`: não implementado (o DartForge
  devolve as edições na própria ação). **Diverge** na forma (o Dart manda
  `Command` para fonte e refatorações).

## 14. `textDocument/completion` e `completionItem/resolve`

### 14.1 Entrada e fluxo

`AS:src/lsp/handlers/handler_completion.dart:86-586`;
`AS:src/services/completion/dart/completion_manager.dart:103-539`.
* Um pedido novo cancela o anterior (`:94-98`); a unidade e o `LineInfo` vêm
  do mesmo instante (`lockRequestsWhile`, `:105-125`).
* **`triggerKind` não é lido**, só `triggerCharacter` (`:101`), validado por
  `_triggerCharacterValid` (`:731-760`): `"`/`'` só logo depois da aspa de
  abertura de uma URI de diretiva; `{` só logo depois de `${`; `/` só dentro
  da URI de diretiva; `:` exceto em `switch`; `.`, `=`, `(`, `$` sempre.
  Inválido → lista vazia, `isIncomplete:false`.
* Range de substituição (`AP:src/utilities/completion/completion_target.dart:471-533`):
  identificador/palavra sob o cursor **inteiro** (inclusive à direita); dentro
  da string de URI, o conteúdo; senão vazio no cursor. `insert` = até o
  cursor; `InsertReplaceEdit` se o cliente aceita e os dois diferem (`mapping.dart:1160-1175`);
  `itemDefaults.editRange` quando o cliente aceita (`:250-291`).
* Prefixo (`completion_manager.dart:388-445`, `:484-539`): o texto do
  identificador até o cursor; `NoPrefixMatcher` com prefixo vazio.
* Comentário comum → nada (`:157-160`); doc comment só em `[ref]`; strings
  só em diretivas; depois de número, nada.
* `InScopeCompletionPass` (`in_scope_completion_pass.dart`) num único nó
  (`_completionNode`, `:123-159`), que grava `collector.completionLocation` e
  chama `KeywordHelper`, `DeclarationHelper`, `IdentifierHelper`,
  `LabelHelper`, `OverrideHelper`, `UriHelper`.
* `NotImportedCompletionPass` (`not_imported_completion_pass.dart:111-228`):
  com `suggestFromUnimportedLibraries` (padrão) **e** `workspace.applyEdit`;
  orçamento de **100 ms** (`completionBudgetMilliseconds`); varre bibliotecas
  do SDK (exceto internas e `dart:html`, `indexed_db`, `js`, `js_util`, `svg`,
  `web_audio`, `web_gl`, `AN:src/dart/analysis/file_state_filter.dart:22-45`)
  e dos pacotes (sem `src/` de outros); orçamento estourado → `isIncomplete`.
* Filtro (`handler_completion.dart:510-524`, `:946-965`): `FuzzyMatcher(prefixo)`
  (`AN:src/utilities/fuzzy_matcher.dart`; sensível à caixa só se o prefixo
  tem maiúscula) com score > 0.
* Limite `dart.maxCompletionItems` (2000): no coletor por score e no handler
  (`_truncateResults`, `:765-827`: score, depois `sortText`, depois rótulo
  mais curto; mantém o que casa exatamente o prefixo).
* `isIncomplete` com orçamento estourado, truncamento ou erro (`:238-240`).
* **O servidor não ordena** a lista: o cliente ordena por `sortText`.

### 14.2 O que é proposto (InScopeCompletionPass)

* Depois de `.`: membros da interface do receptor (`declaration_helper.dart:344-1015`,
  filtrados por acesso, `@protected`, `@visibleForTesting`), extensões
  aplicáveis (importadas; as não importadas pelo pass de não importados);
  record: `$1…`/campos; `Function`: `call`; tipo/`Type`: membros estáticos,
  constantes de enum (sem o nome do enum), construtores nomeados; prefixo:
  declarações da biblioteca e `loadLibrary`.
* Escopo (`addLexicalDeclarations`, `:388-421`): locais e parâmetros do mais
  próximo ao mais distante (`distance` 0, 1, …), membros da declaração
  envolvente, topo da biblioteca, prefixos, importados, membros herdados.
* `Enum.valor` (`:1847-1903`) quando o tipo de contexto é o enum.
* Palavras-chave por contexto (`keyword_helper.dart`): expressão (`false`,
  `null`, `true`, `const`, `super`, `this`, `await`, `switch`), comando
  (`return`, `if`, `for`, `final`, `var`…), membro de classe, topo,
  diretivas (`import '^';`), `async`/`async*`/`sync*`, inicializadores.
* Argumentos nomeados (`:272-395`, `:1960-2014`): os ainda não usados, como
  `nome: ` (com `,` conforme `:321-360`) e closures para parâmetros-função.
* Overrides (`override_helper.dart:31-68`), `this.`/`super.` em construtor
  (`declaration_helper.dart:228-550`), `show`/`hide` (nomes exportados, sem
  parênteses), URIs (`uri_helper.dart`), labels.
* Nome de declaração: o nome do arquivo em UpperCamel (classes) ou
  combinações do nome do tipo (variáveis); senão nada.

### 14.3 Relevância

`AS:src/services/completion/dart/feature_computer.dart:48-432`,
`relevance_computer.dart:53-525`, `relevance_tables.g.dart`,
`probability_range.dart:18-25`:

```
média = Σ(v_i·w_i) / 9    (10 características; inheritanceDistance fora da média)
relevância = trunc(((média + 1) / 2) · 1000)
```

Pesos: contextType 1, elementKind 1, hasDeprecated 0,5, isConstant 1,
isNoSuchMethod 1, isNotImported 1, keyword 1, startsWithDollar 0,5,
superMatches 1, localVariableDistance 1. contextType: igual 1,0, subtipo 0,40,
supertipo 0,02, sem relação 0,13 (`:243-261`); tipo de contexto do
`_ContextTypeVisitor` (`:439-1244`; argumento, atribuição, condição `bool`,
`return`, coleções, `for-in`, inicializador declarado ou implícito pelo nome
`i/j/index/length`…). elementKind = faixa da tabela
`elementKindRelevance[local][espécie]` (meio, ou `meio + (sup-inf)·d/2` com
a distância de herança `0,9^d`). localVariableDistance `0,9^distância`.
Constantes: closure 900, `call` 200, identificador 500, label 1000,
`loadLibrary` 200, argumento nomeado **950** (obrigatório)/**900**, campo de
record 950, override 750, `super.x` 1000, URI `dart:core` 100 e demais 900.
Locais (`CompletionLocation`, `:3778-3797` e 187 atribuições):
`Block_statement`, `ArgumentList_<ctx>_<named|unnamed>`,
`PropertyAccess_propertyName`, `VariableDeclaration_initializer`,
`ReturnStatement_expression`, `IfStatement_condition`, … (sem tabela →
elementKind 0).

### 14.4 Formato do item

`AS:src/lsp/mapping.dart:974-1179`: `label` = `displayText ?? completion`
(com `labelDetailsSupport`: só o nome); `filterText` = rótulo cortado no
primeiro `(`/`=>` (só se diferente); `labelDetails` = `detail` (` T`, `(…) →
T`, `() → T`, `(…)`) e `description` = URI da biblioteca a importar;
`kind` por `ElementKind` (`:268-338`) ou por `CompletionSuggestionKind`
(`:892-938`); `detail` = assinatura completa; `documentation` (salvo quando
o item precisa de resolve); **`sortText` = `9999 − relevância`** (4
dígitos); `insertText` com snippet só com `dart.completeFunctionCalls`
(padrão falso) ou seleção no meio (`import '$0';`, `nome: $0,`);
`data = {file, importUris, ref}` para não importados e overrides que pedem
import; `deprecated`/`tags`. Deduplicação pelo texto (`suggestion_builder.dart:989-1026`).
`completionItem/resolve` (`handler_completion_resolve.dart:57-193`):
documentação e `additionalTextEdits` do import (`"Auto import from '<uri>'"`
no `detail`).

### 14.5 Estado no DartForge

`crates/lsp/src/completar.rs`, `relevancia.rs`, `relevancia_tabelas.rs`
(as tabelas do 3.6.2 geradas por `crates/lsp/oraculo/gerar_tabelas.py`):
**alvo presente 98%**, **mesmo top-1 63%**, **top-5 com ≥ 60% em comum
52%** (319 posições em que o Dart responde). Implementado: membros pelo
tipo do receptor, escopo, palavras-chave, argumentos nomeados, constantes de
enum, `show`/`hide`, `this.`/`super.`, nada em nome de declaração,
relevância pelo modelo do Dart, `labelDetails`, resolve com documentação.
**Diverge:**
* **ordem dentro da mesma relevância**: o DartForge desempata pelo nome
  (`StackOverflowError, StackTrace, StateError`); o Dart não ordena — o
  cliente desempata pela ordem de chegada, que é a do coletor
  (`matcherScore` decrescente, depois inserção: `StateError, StackOverflowError,
  StackTrace`). Reproduzir exige o score do `FuzzyMatcher` por item e a ordem
  de visita do Dart (escopo, importados, SDK);
* **não importados**: o Dart propõe o SDK e os pacotes não importados
  (`InternetAddress`, `Platform`, `ProcessInfo`…) com `isNotImported`; o
  DartForge só o que o índice conhece;
* receptor: membros de `Object` (`hashCode`) e extensões fora de ordem; o
  Dart pontua membros por distância de herança e `elementKind` de
  `PropertyAccess_propertyName` (campo ≫ método), e o DartForge às vezes
  ordena métodos e getters juntos (`codeUnitAt` × `allMatches`);
* nome de declaração: o Dart sugere o nome do arquivo/tipo (`main` em
  `import … as m^`, `Ponto` em classe), o DartForge nada;
* `isIncomplete`, `itemDefaults`, `InsertReplaceEdit`, `filterText`,
  snippets e o orçamento de 100 ms não são imitados.
* **Latência:** mediana **69,5 ms** contra **5,0 ms** do Dart: o DartForge
  refaz a inferência da biblioteca a cada pedido (o Dart reaproveita a
  unidade resolvida). Proposta: cache por corpo (seção 16).

## 15. `signatureHelp`, `semanticTokens`, `inlayHint`

### 15.1 `textDocument/signatureHelp`

**Algoritmo** (`AS:src/lsp/handlers/handler_signature_help.dart:30-137`;
`AS:src/computer/computer_signature.dart:20-118`;
`AS:src/computer/computer_type_arguments_signature.dart:20-114`;
`AS:src/lsp/mapping.dart:1337-1410`):
1. `autoTriggered` = `triggerKind == TriggerCharacter && isRetrigger == false`
   (`:48-53`).
2. Primeiro tenta a assinatura de **argumentos de tipo**: `TypeArgumentList`
   mais interna (uma `FunctionExpression` no caminho encerra a busca) cujo pai
   é `NamedType` ou `MethodInvocation` de um elemento com parâmetros de tipo;
   rótulo `element.getDisplayString()`, parâmetros = cada parâmetro de tipo
   (`T extends B`), `activeParameter: -1`; disparo automático só logo depois
   do `<`.
3. Senão `DartUnitSignatureComputer`: `NodeLocator(offset)`; sobe até a
   `ArgumentList` (uma `FunctionExpression` no caminho → nada). Pai:
   `MethodInvocation` (nome = `methodName`, elemento = `ElementLocator`),
   `InstanceCreationExpression` (nome = `type.qualifiedName` + `.nome`),
   `FunctionExpressionInvocation` com `function` identificador (tipo de
   função, ou o `call` de uma instância chamável). Sem nome, elemento ou
   parâmetros → `null`.
4. Disparo automático só com `offset == argumentList.offset + 1` (logo depois
   do `(`).
5. Rótulo (`mapping.dart:1343-1377`): `nome(obrigatórios, [opcionais],
   {nomeados})`, cada parâmetro `[required ]<tipo> <nome>[ = <padrão>]`, tipo
   `param.type.getDisplayString()` (substituído no `Member`); parâmetros como
   rótulos-string (sem offsets); documentação `cleanDartdoc` em markdown ou
   texto; `activeSignature: 0`, **`activeParameter: -1`** (`:1399-1408`).

**Estado no DartForge** (`crates/lsp/src/assinatura.rs`): **99%** pelo rótulo
(70 pedidos com resposta do Dart). **Diverge:** um rótulo do SDK com o nome do
parâmetro do patch (`write(Object? obj)` × `write(Object? object)`, mesma causa
do patch no hover); `activeParameter` calculado (escolha registrada em
`docs/LSP.md`; o 3.6.2 manda -1); a assinatura de argumentos de tipo
(`List<▮>`) não existe.

### 15.2 `textDocument/semanticTokens/full` e `/range`

**Algoritmo** (`AS:src/lsp/handlers/handler_semantic_tokens.dart:34-179`;
`AS:src/computer/computer_highlights.dart`;
`AS:src/lsp/semantic_tokens/{mapping,legend,encoder}.dart`):
`DartUnitHighlightsComputer` (unidade resolvida) emite regiões
`HighlightRegionType` (`computer_highlights.dart:64-1804`: comentários,
`_addIdentifierRegion*` por elemento — classe, construtor, local dinâmico,
extensão, campo, função, getter/setter, prefixo, palavra-chave, label, local,
método, parâmetro, typedef, parâmetro de tipo, membro não resolvido —, e um
`visit*` por nó para palavras-chave, literais, strings, interpolações,
anotações). Cada tipo de região vira tipo+modificadores
(`semantic_tokens/mapping.dart:10-179`: p.ex. `INSTANCE_FIELD_DECLARATION` →
`variable` + `declaration`,`instance`; `INSTANCE_GETTER_REFERENCE` →
`property` + `instance`; `TOP_LEVEL_VARIABLE` → `property`;
`TOP_LEVEL_FUNCTION_DECLARATION` → `function` + `declaration`,`static`;
`IDENTIFIER_DEFAULT`/`UNRESOLVED_INSTANCE_MEMBER_REFERENCE` → `source`;
`VALID_STRING_ESCAPE` → `string` + `escape`). O handler ordena
(`offsetLengthPrioritySort`), **divide sobreposições**
(`encoder.dart:114`, o de dentro vence) e **quebra tokens de várias linhas**
(`:85`) sempre (VS Code não aceita nenhum dos dois; `:86-110`), filtra pelo
intervalo no `/range` e codifica em UTF-16 relativo.

**Estado no DartForge** (`crates/lsp/src/realce.rs`): **100%** pelo tipo de
token (13 877 tokens do Dart; 7 diferentes). **Diverge:** a variável de
`for (var i = 0; …)` sai `source` em vez de `variable`+`declaration`;
funções locais declaradas depois do uso (`function` × `variable`); nomes
de campo em padrões de objeto (`MapEntry(key: …)`) e o nome de operador
(`operator []`); referência `[Context.new]` num doc comment.

### 15.3 `textDocument/inlayHint`

**Algoritmo** (`AS:src/lsp/handlers/handler_inlay_hint.dart:25-85`;
`AS:src/computer/computer_inlay_hint.dart:40-403`): unidade resolvida, o
arquivo inteiro (o `range` pedido é ignorado). O visitor emite:
* `visitArgumentList` (`:233-246`): para cada argumento **posicional** com
  `staticParameterElement`, `nome:` (kind 2, `paddingRight`) **antes** dos
  demais hints do argumento;
* `visitDeclaredIdentifier`, `visitDeclaredVariablePattern`,
  `visitVariableDeclaration` (lista sem tipo), `visitSimpleFormalParameter`
  (sem tipo; antes do nome): o tipo (kind 1, `paddingRight`);
* `visitFunctionDeclaration`/`visitMethodDeclaration` sem retorno escrito
  (não setter; antes de `get`/nome): o tipo de retorno;
* `visitInvocationExpression` (`:302-312`): sem argumentos de tipo escritos,
  **os inferidos** (`typeArgumentTypes`) antes do `(`;
* `visitListLiteral`/`visitSetOrMapLiteral`: os argumentos do tipo estático
  antes do `[`/`{`;
* `visitNamedType` (`:344-354`): tipo sem argumentos escritos cujo tipo tem
  argumentos → `<…>` **depois** do nome (`List` cru → `<dynamic>`; `Foo()` com
  inferência → `<int>`).
Rótulos em partes com `location` para cada elemento (`_appendTypePart`);
`InvalidType` aparece como tal.

**Estado no DartForge** (`crates/lsp/src/dicas.rs`): **50%** por arquivo (30
arquivos com dicas do Dart). **Diverge:** argumentos de tipo **inferidos** de
chamadas genéricas (`<int>` em `math.max`) e de tipos crus (`<dynamic>`,
`<dynamic, dynamic>`) faltam (29); variáveis de `for (var i = 0; …)` sem tipo
(30 dicas `int`); nomes de parâmetro do SDK vindos do patch (`obj:` ×
`object:`, 31) e alguns `message:` que faltam; `InvalidType` (o motor diz
`dynamic`); três argumentos de tipo a mais em criações cujo tipo já tem
argumentos implícitos.

## 16. Latência do completar: diagnóstico e proposta

**Medido** (release, Windows; oráculo e `crates/lsp/examples/latencia_recursos.rs`):
completion **67–82 ms** com o texto parado e 74–88 ms logo depois de uma
edição, contra **~5 ms** do Dart; hover 2,5–3,5 ms (Dart 1,4 ms).

**Causa** (`crates/lsp/src/completar.rs:218-262`): a cada pedido o
DartForge (1) copia o texto com um sentinela no cursor (`preparar`, para
analisar código incompleto), (2) recarrega o programa com essa cópia
(`semantica::carregar`: parse da unidade e montagem dos elementos) e (3)
infere de novo **o esboço e todos os corpos da biblioteca**
(`Consulta::inferir` → `dartforge_types::infer_bodies_das_bibliotecas*`).
O Dart reaproveita a unidade resolvida e só re-resolve o que mudou.

**Proposta, em `crates/lsp`** (sem tocar no motor):
1. Reaproveitar o `Projeto` retido da sessão quando só o corpo da função sob
   o cursor muda: o sentinela fica dentro de um corpo, logo o esboço
   (`OutlineTypes`) e os corpos das outras funções são os mesmos.
2. Cache por corpo dentro da sessão: chave = (unidade, intervalo do corpo,
   hash do texto do corpo); valor = os tipos das expressões do corpo; um
   pedido no mesmo corpo com o mesmo texto não reinfere nada (o caso
   "texto parado").

**O que falta no motor (`crates/types`, `crates/elements`) — pedido ao
coordenador:**
* **E1 — inferir um corpo isolado**: uma variante de
  `infer_bodies_das_bibliotecas` que receba `OutlineTypes` já calculado (do
  `Projeto` retido) e um conjunto de corpos (função, método, construtor ou
  inicializador de campo, por `(UnitId, FunctionId/MemberId)`), inferindo
  só esses e devolvendo um `UnitBodyTypes` parcial. É o que o
  `BodyInferrer` já faz com `apenas_bibliotecas`, só que por corpo.
* **E2 — trocar o texto de uma unidade sem recarregar o programa**: em
  `dartforge_elements::load`, reparsear uma unidade e substituir a árvore e
  os elementos **só daquele corpo** (as declarações de topo e membros não
  mudam quando a edição é interna a um corpo), devolvendo o mesmo `Program`
  com a unidade nova.
Com E1+E2 o completar vira parse de uma unidade + inferência de um corpo (a
mesma ordem de grandeza do hover, 3 ms).

## 17. Maiores divergências (por número de posições do oráculo)

| # | Requisição / recurso | Atual | Posições diferentes | Causa principal | Onde se resolve |
|---|---|---|---|---|---|
| 1 | codeAction: ações de fonte | 0% | 416 | faltam `Sort Members` e `Fix All` | `crates/lsp` (em andamento) |
| 2 | codeAction: assists | 26% | ~280 | sem `NodeLocator` na árvore do analyzer; faltam Extract Method (185), Inline Method (77), Move to file (43) e as regras exatas de Convert to block body, Remove/Add type annotation, Extract/Inline Local | `crates/lsp` |
| 3 | selectionRange | 48% | 213 | nós do analyzer ausentes na árvore do parser | `crates/lsp` (árvore no formato do analyzer) |
| 4 | references | 58% | 172 | 97 no SDK inteiro (indexar o SDK); construtor × classe, rótulos nomeados, prefixos, privados | `crates/lsp` (+ índice do SDK) |
| 5 | completion top-1 / top-5 | 63% / 52% | ~150 / ~190 | ordem de empate (score fuzzy + ordem do coletor), não importados, nomes de declaração | `crates/lsp` |
| 6 | prepareTypeHierarchy | 76% | 98 | tipo escrito (`List<String>`, `String?`) e posições com o `LineInfo` do arquivo atual | `crates/lsp` |
| 7 | implementation | 77% | 94 | implementações nas bibliotecas internas do dart2js | motor (carregar o SDK como o analyzer: sem patches, com `_internal/js_runtime`) |
| 8 | hover (texto) | 86% | 57 | `this.x` sem `Type`, docs do patch, `InvalidType`, docs herdadas de campo | `crates/lsp` + motor (patch, `InvalidType`) |
| 9 | prepareCallHierarchy | 86% | 57 | alvo = a invocação que contém o nome | `crates/lsp` |
| 10 | rename / prepareRename | 89% / 96% | 45 / 16 | construtor sem nome → nomeado, família com raiz no SDK, prefixos | `crates/lsp` |
| 11 | workspace/symbol | 0% | 24 | o Dart inclui SDK e pacotes | `crates/lsp` (índice sintático do SDK) |
| 12 | typeDefinition | 95% | 22 | anotação, construtor, campo | `crates/lsp` |
| 13 | documentHighlight | 95% | 20 | sem família de sobrescrita no Dart | `crates/lsp` |
| 14 | inlayHint | 50% (arquivos) | 18 arquivos | argumentos de tipo inferidos, `for`, nomes do patch | motor (argumentos de tipo inferidos de invocações; nomes públicos do SDK) + `crates/lsp` |
| 15 | definition | 96% | 16 | patch do SDK; construtor implícito → classe | motor (SDK sem patches) + `crates/lsp` |
| 16 | formatting | 0% | 36 | sem formatador (L08) | porte do `dart_style` (fora do escopo) |
| 17 | semanticTokens | 100% | 7 tokens | `for`, função local, operador | `crates/lsp` |
| 18 | quick fixes | 75% | 3 | `Add missing switch cases`, `Add required argument` | `crates/lsp` |
| 19 | signatureHelp | 99% | 1 | nome do parâmetro do patch | motor |
| 20 | documentSymbol | 97% | 1 | funções locais | `crates/lsp` |

**Necessidades do motor que atravessam vários recursos:**
* **M1 — SDK como o analyzer vê:** declarações públicas sem os patches
  (alvo de definition/hover/assinatura e nomes de parâmetro `object` × `obj`)
  e as bibliotecas internas do dart2js (`_interceptors`, `_js_helper`…), que
  o analyzer resolve para implementation, references e workspace/symbol.
  Alternativa no LSP: mapear cada elemento do patch para a declaração
  `external` homônima da biblioteca pública (mesma classe, mesmo nome).
* **M2 — `InvalidType` distinto de `dynamic`** (hover, inlay hints).
* **M3 — argumentos de tipo inferidos das invocações genéricas**
  (`typeArgumentTypes`; inlay hints `<int>` e o `Type:` de tear-offs).
* **E1/E2** (seção 16).
