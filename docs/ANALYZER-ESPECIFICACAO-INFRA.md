# Especificação da infraestrutura do analyzer 3.6.2: pipeline, relato, supressões, saída do `dart analyze` e o catálogo dos códigos fora dos 436 (2026-10-04)

Este documento complementa `docs/ANALYZER-ESPECIFICACAO.md` (os 436 códigos
com perda no placar, por família, e os achados transversais T1–T8). Aquele
documento responde "o que cada código exige"; este responde **"em que ordem o
analyzer roda, como um diagnóstico nasce, é deduplicado, suprimido, reclassificado,
ordenado e impresso"**, e cataloga **todos os códigos da tabela oficial 3.6.2
que ficaram fora das listas `familia-*.txt`**. Nada do que está lá é repetido
aqui: as referências são `ANALYZER-ESPECIFICACAO.md §X` e `T1`…`T8`.

## Fontes e convenções

- **Fonte oficial, tag 3.6.2 do SDK**:
  `E:\references\dart-sdk-3.6.2\pkg\{analyzer,_fe_analyzer_shared,analysis_server}\lib`,
  `pkg/analyzer/messages.yaml`, `pkg/front_end/messages.yaml`. Citações
  `analyzer/lib/src/...:linha`, `_fe_analyzer_shared/lib/src/...:linha`,
  `analysis_server/lib/src/...:linha`, relativas a `E:\references\dart-sdk-3.6.2\pkg\`.
- **`dartdev`, `analyzer_cli`, `linter`, `analysis_server_client`** não estão na
  cópia `dart-sdk-3.6.2`. O checkout `E:\references\dart-sdk` (main/3.14) **tem a
  tag `3.6.2`**; as quatro pastas foram extraídas dela com
  `git archive 3.6.2 pkg/dartdev/lib pkg/analyzer_cli/lib pkg/linter/lib pkg/linter/messages.yaml pkg/analysis_server_client/lib`
  para `E:\dftemp\analise\spec-infra\sdk362\pkg\`. Citações `dartdev/lib/src/...:linha`,
  `linter/lib/src/...:linha` são relativas a essa pasta e **são da 3.6.2**, não do main.
- **Pacotes de terceiros** que o `dartdev` usa (`cli_util`, `path`, `args`) e o
  `package:lints` não estão na árvore do SDK (entram pelo `DEPS`). O que se cita
  deles vem do cache do pub desta máquina (`cli_util-0.4.2`, `lints-5.1.1`) e está
  marcado; a revisão exata fixada pelo `DEPS` da 3.6.2 (`tools_rev`, `lints_rev`,
  `DEPS:151,178`) **não foi conferida** contra essas versões.
- **Lado DartForge**: `crates/<crate>/src/...:linha`, relativas a
  `E:\MyRustProjects\dartforge`, no estado do working tree de 2026-10-04 (há
  trabalho não commitado em `crates/types` e `crates/elements`; as linhas citadas
  desses dois crates podem deslocar).
- **Tabela de códigos**: a 3.6.2 define **1.083 constantes** de código
  (`analyzer/lib/src/error/error_code_values.g.dart` lista 1.082;
  `ScannerErrorCode.UNEXPECTED_SEPARATOR_IN_NUMBER` está definido em
  `_fe_analyzer_shared/lib/src/scanner/errors.dart:163` e é emitido — `:64` —
  mas falta em `errorCodeValues`; ver a Parte II, II.3). Por classe:
  `CompileTimeErrorCode` 537, `ParserErrorCode` 265, `WarningCode` 144,
  `FfiCode` 47, `PubspecWarningCode` 24, `AnalysisOptionsWarningCode` 17,
  `ScannerErrorCode` 12, `HintCode` 8, `StaticWarningCode` 7,
  `ManifestWarningCode` 7, `TodoCode` 4, `AnalysisOptionsHintCode` 3,
  `AnalysisOptionsErrorCode` 2. Várias constantes compartilham o mesmo **nome
  emitido** (`ErrorCode.name`, o que o `dart analyze` imprime em minúsculas);
  `uniqueName` é `Classe.CONSTANTE`.
- **"Fora dos 436"** = constante cujo nome emitido não está em nenhuma
  `E:\dftemp\analise\trab\familia-*.txt` (435 nomes distintos nas seis listas; 15
  deles só existem no 3.13 e não entram na conta da 3.6.2). São **546
  constantes, 512 nomes distintos** (Parte II).
- **Estado no DartForge** (Parte II), conferido por grep em `crates/`:
  **publicado** (emitido e, se semântico, listado em `crates/analise/verificados.txt`;
  sintaxe é publicada sempre — `crates/analise/src/publicacao.rs:24-34`),
  **emitido, não publicado** (há emissor em `crates/{analise,types,frontend,elements}`,
  o nome não está em `verificados.txt`), **não implementado** (nenhum emissor).
- **Sondas no binário**: há um `dart` 3.6.2 nesta máquina
  (`C:\tools\dartsdk-3.6.2`, `Dart SDK version: 3.6.2 (stable)`). Onde o texto
  diz "conferido no binário" ou "sonda sN", o comportamento foi medido com
  `dart analyze --cache=E:/dftemp/analise/spec-infra/cache …` sobre os projetos
  mínimos de `E:\dftemp\analise\spec-infra\sonda\sN` (saídas guardadas ao lado).
  As sondas confirmam formato, ordenação, códigos de saída e supressões; **não**
  substituem a leitura da fonte para as condições de cada código.
- O que não foi conferido na fonte está marcado **"não verificado"**. Não há
  estimativas apresentadas como fato; contagens vêm de scripts sobre a fonte
  (`E:\dftemp\analise\spec-infra\{catalogo,grupos,auto}.py`).

## Sumário

- **Parte I — O pipeline do analyzer, de ponta a ponta**
  - §1 `AnalysisDriver` e `LibraryAnalyzer`: a ordem exata das fases
  - §2 Linking (`summary2`): elementos, inferência de topo, ciclos
  - §3 `ErrorReporter` e `RecordingErrorListener`: posição, argumentos, deduplicação
  - §4 Supressões: `// ignore:`, `IgnoreValidator`, `analysis_options.yaml`
  - §5 Severidade, tipo e a saída do `dart analyze` (texto, JSON, machine)
  - §6 Versão de linguagem e experimentos
  - §7 `TodoFinder` e arquivos não-Dart (pubspec, analysis_options, manifest)
  - §8 Lints: mecanismo, registro e lista de regras
- **Parte II — Catálogo dos códigos fora dos 436** (546 constantes, por emissor)
- **Parte III — Plano de implementação no DartForge** (etapas, critério de pronto, testes)

---

# Parte I — O pipeline do analyzer, de ponta a ponta

## 1. `AnalysisDriver` e `LibraryAnalyzer`: a ordem exata das fases

### 1.1 Quem chama quem

```text
dart analyze <dir>                                  dartdev/lib/src/commands/analyze.dart:100
  └─ processo analysis_server (protocolo legado)    dartdev/lib/src/analysis_server.dart:108-205
       analysis.setAnalysisRoots(included=[dir])    :197-202
       └─ ContextManagerImpl._createAnalysisContexts analysis_server/lib/src/context_manager.dart:533
            AnalysisContextCollectionImpl           :549   (um contexto/driver por raiz de contexto, §4.5)
            para cada arquivo de contextRoot.analyzedFiles():          :598-610
              analysis_options.yaml → _analyzeAnalysisOptionsYaml      :599-602  (§7)
              AndroidManifest.xml   → _analyzeAndroidManifestXml       :603-604
              *.dart                → driver.addFile(file)             :605-606
              pubspec.yaml          → _analyzePubspecYaml              :607-608
            lib/fix_data.yaml e lib/fix_data/**.yaml → _analyzeFixDataYaml   :612-625
  AnalysisDriver (um por contexto)
    _analyzeFileImpl(path)                          analyzer/lib/src/dart/analysis/driver.dart:1323
      library = file.kind.library ?? file.kind.asLibrary               :1334-1335
      sem dart:core / dart:async → resultado só com MISSING_DART_LIBRARY :1349-1363, :1923-1945
      libraryContext.load(targetLibrary)   (link, §2)                  :1367-1373
      LibraryAnalyzer(...).analyze()                                   :1393-1401
      para cada unidade da biblioteca: ErrorsResult + bytes no cache   :1406-1455
  servidor: handleFileResult                        analysis_server/lib/src/analysis_server.dart:1097
      se isAnalyzed(path): doAnalysisError_listFromEngine(result)      :1118-1122  (ErrorProcessor, §4.4)
      NotificationManager.recordAnalysisErrors → notificação analysis.errors
                                                    analysis_server/lib/src/plugin/notification_manager.dart:120-127, :321-324
  dartdev: junta os analysis.errors, ordena, imprime (§5)
```

Fatos que decorrem disso e que importam para a paridade:

- A unidade de trabalho é a **biblioteca**: analisar um arquivo analisa a
  biblioteca inteira dele (arquivo de biblioteca + partes) e produz um
  resultado **por unidade** (`driver.dart:1406-1455`). Um arquivo `part of`
  cuja biblioteca não é conhecida é analisado **como se fosse biblioteca**
  (`kind.library ?? kind.asLibrary`, `driver.dart:1334-1335`).
- Os erros de cada unidade são gravados no cache em bytes
  (`ErrorEncoding.encode`, `driver.dart:2542-2563`: `offset`, `length`,
  `uniqueName`, `message`, `correction`, `contextMessages`) e relidos por
  `ErrorEncoding.decode` (`:2501-2540`) — por `uniqueName`; um código que não
  existe mais é **descartado** com log (`:2507-2515`). Os **argumentos** não
  sobrevivem: a mensagem já sai formatada.
- O servidor só envia `analysis.errors` para arquivos **analisados** pela raiz
  (`isAnalyzed`, `analysis_server/lib/src/context_manager.dart:312-321` →
  `ContextRootImpl.isAnalyzed`, `analyzer/lib/src/dart/analysis/context_root.dart:79-94`):
  um arquivo de `package:` fora da raiz, o SDK e os excluídos são analisados
  quando importados, mas os erros deles **não saem**.

### 1.2 `LibraryAnalyzer.analyze()` — pseudocódigo fiel

`analyzer/lib/src/dart/analysis/library_analyzer.dart:107-124`:

```text
analyze():
  _parseAndResolve()                                  :108, corpo :642-668
  _computeDiagnostics()                               :109, corpo :301-349
  para cada FileAnalysis em _libraryFiles (ordem de inserção: biblioteca, depois
      cada parte na ordem das diretivas `part`, em profundidade — :731-743, :1084-1088):
    errors = fileAnalysis.errorListener.errors        :114   (Set → lista, §3.5)
    errors = _filterIgnoredErrors(fileAnalysis, errors) :115 (§4.1)
    UnitAnalysisResult(file, unit, errors)            :116-122
```

**Fase A — parse e diretivas** (`_parseAndResolve` → `_resolveDirectives`, `:642-647`, `:726-809`):

```text
_resolveDirectives(enclosingFile, fileKind, fileElement):          :726
  fileAnalysis = _parse(file, unitElement)                          :731-734 → :618-640
      RecordingErrorListener novo por arquivo                       :622
      file.parse(errorListener)  → FileState.parseCode              file_state.dart:655-690
          Scanner(...).configureFeatures(featureSetForOverriding: featureSet,
                    featureSet: featureSet.restrictToVersion(packageLanguageVersion))   file_state.dart:664-670
          token = scanner.tokenize(reportScannerErrors: false)      file_state.dart:671
              (// @dart=x.y > versão corrente → INVALID_LANGUAGE_VERSION_OVERRIDE_GREATER
               direto no listener, analyzer/lib/src/dart/scanner/scanner.dart:166-195)
          Parser(source, errorListener, featureSet: scanner.featureSet, lineInfo)
              .parseCompilationUnit(token)                          file_state.dart:674-681
              (parser fasta → AstBuilder; os tokens de erro do scanner são relatados
               NO FIM do parseUnit: _fe_analyzer_shared/lib/src/parser/parser_impl.dart:403-407, :440, :9453-9459
               → AstBuilder.handleErrorToken → translateErrorToken,
               analyzer/lib/src/fasta/ast_builder.dart:4206-4208)
          unit.languageVersion = LibraryLanguageVersion(package, override)   file_state.dart:682-685
      FileAnalysis: ErrorReporter(errorListener, file.source) e
          IgnoreInfo.forDart(unit, file.content)                    file_analysis.dart:22-29
  para cada diretiva, NA ORDEM DO ARQUIVO:                          :743-777
    export  → _resolveLibraryExportDirective                        :744-751 → :898-950
    import  → _resolveLibraryImportDirective                        :752-759 → :952-969
                 → _reportImportDirectiveErrors                     :671-724
    library → directive.element = biblioteca                        :760-763
    part    → _resolvePartDirective (recursivo: _resolveDirectives da parte)   :764-772 → :971-1089
    part of → directive.element = biblioteca                        :773-776
  parte de macro (sem diretiva)                                     :779-793
  @docImport do comentário da diretiva `library`                    :795-808 → :882-896
```

Códigos da fase A relatados pelo próprio `LibraryAnalyzer` (todos em
`atNode(directive.uri, …)`, isto é, **no literal de string do URI**, com aspas):

| condição | import (`:676-723`) | export (`:909-949`) | part (`:993-1075`) |
|---|---|---|---|
| URI com interpolação (estado sem `UriStr`) | `uri_with_interpolation` (`:716-721`) | `uri_with_interpolation` (`:944-949`) | `uri_with_interpolation` (`:993-998`) |
| URI não analisável (estado `WithUriStr` sem `WithUri`) | `invalid_uri` [uri] (`:710-715`) | `invalid_uri` [uri] (`:938-943`) | `invalid_uri` [uri] (`:1000-1006`) |
| URI começa com `dart-ext:` | `use_of_native_extension` (`:677-682`) | `use_of_native_extension` (`:910-915`) | — |
| sem fonte para o URI (`importedSource == null`) | `uri_does_not_exist` (ou `uri_does_not_exist_in_doc_import` se `isDocImport`) [uri] (`:683-691`) | `uri_does_not_exist` [uri] (`:916-921`) | `uri_does_not_exist` [uri] (`:1008-1014`) |
| arquivo não existe | `uri_has_not_been_generated` se `isGeneratedSource`, senão `uri_does_not_exist` (doc import: `uri_does_not_exist_in_doc_import`) [uri] (`:692-702`) | idem, sem o caso doc import (`:922-930`) | idem, só quando o arquivo incluído não é `PartFileKind`; o argumento é `includedFile.uriStr` (`:1019-1033`) |
| o alvo não é biblioteca (é `part of`) | `import_of_non_library` [uri] (`:703-709`) | `export_of_non_library` [uri] (`:931-937`) | — |
| alvo de `part` existe e não tem `part of` | — | — | `part_of_non_part` [uriStr do arquivo] (`:1021-1022`) |
| mesma parte incluída duas vezes | — | — | `duplicate_part` [uri] (`:1038-1044`) |
| `part of nome;` com nome diferente (sem `enhanced-parts`) | — | — | `part_of_unnamed_library` [nome] se a biblioteca não tem nome; senão `part_of_different_library` [nomeDaBiblioteca, nome] (`:1049-1064`) |
| `part of 'uri';` apontando para outra biblioteca | — | — | `part_of_different_library` [uriStr do arquivo que inclui, uriStr da parte] (`:1065-1072`) |

Ordem real dos testes no import/export: `dart-ext:` → fonte nula → arquivo inexistente → não é biblioteca (dentro de `state is …WithUri`); depois `invalid_uri`; por fim interpolação. No `part`: interpolação → inválido → sem arquivo → não é parte → duplicada → `part of` de outra biblioteca. `isGeneratedSource` = `file_paths.isGenerated(source.fullName)` (`analyzer/lib/src/context/source.dart:21-26`; os sufixos estão em `analyzer/lib/src/util/file_paths.dart`, **não verificado** aqui; ver II.6).
A primeira condição verdadeira vence; uma parte com erro **não é parseada nem
entra em `_libraryFiles`** (os `return` de `:997`, `:1005`, `:1013`, `:1032`,
`:1043`, `:1074`), logo não tem resultado próprio vindo desta biblioteca.

**Fase B — resolução** (`:649-667`, `_resolveFile` `:811-880`), por arquivo, na ordem de `_libraryFiles`:

```text
escopos: importsTrackingInit() em cada unidade                      :651-655  (alimenta o ImportsVerifier)
para cada fileAnalysis: _resolveFile                                :657-659
  B1  ResolutionVisitor(unitElement, errorListener, nameScope, strictInference, strictCasts,
          ElementWalker…)                                           :821-834
        casa a AST com os elementos já construídos pelo link; resolve NamedType
        (NamedTypeResolver), anotações de tipo, record types, declarações locais
  B2  ScopeResolverVisitor(library, source, typeProvider, errorListener,
          nameScope, docImportLibraries)                            :844-851
        escopos de rótulos e nomes locais (resolver.dart:4453)
  B3  FlowAnalysisHelper(featureSet: unit.featureSet)               :857-859
      ResolverVisitor(inheritance, library, libraryResolutionContext, source, typeProvider,
          errorListener, analysisOptions: _library.file.analysisOptions,
          featureSet: unit.featureSet, flowAnalysisHelper, libraryFragment)   :863-875
        inferência de tipos, resolução de membros, fluxo; instancia
        NullableDereferenceVerifier, BaseOrFinalTypeVerifier, BoolExpressionVerifier,
        NullSafetyDeadCodeVerifier (resolver.dart:361-413)
importsTrackingDestroy()                                            :662-665
_computeConstants()                                                 :667 → :282-298
  ConstantFinder + ConstantExpressionsDependenciesFinder por unidade (:571-586),
  computeConstants(declaredVariables, constants, featureSet, configuration)  (T8)
```

Observações: `strictInference` e `strictCasts` entram na B1 pelas opções **do
contexto** (`_analysisOptions`, `:825-826`); a B3 recebe as opções **do arquivo
da biblioteca** (`_library.file.analysisOptions`, `:870`). O `featureSet` da
B3 é o **da unidade** (`unit.featureSet`), já restrito pelo `// @dart=`.

**Fase C — verificação** (`_computeDiagnostics`, `:301-349`):

```text
C1  para cada fileAnalysis: _computeVerifyErrors                    :302-304 → :420-457
      C1a ConstantVerifier(errorReporter, library, declaredVariables)   :427 → :272-280
      C1b InheritanceOverrideVerifier(typeSystem, inheritance, errorReporter).verifyUnit(unit)  :432-436
      C1c ErrorVerifier(errorReporter, library, unit.declaredElement, typeProvider, inheritance,
              libraryVerificationContext, analysisOptions, typeSystemOperations)   :441-451
            (dentro: UseResultVerifier, RequiredParametersVerifier, ConstArgumentsVerifier,
             DuplicateDefinitionVerifier, TypeArgumentsVerifier, ReturnTypeVerifier —
             analyzer/lib/src/generated/error_verifier.dart:269-284; GetterSetterTypesVerifier,
             LiteralElementVerifier, CorrectOverrideHelper sob demanda)
      C1d FfiVerifier(typeSystem, errorReporter, strictCasts)       :455-456
C2  MemberDuplicateDefinitionVerifier.checkLibrary(inheritance, libraryVerificationContext,
        libraryElement, files)                                      :306-311   (uma vez por biblioteca)
C3  libraryVerificationContext.constructorFieldsVerifier.report()   :313       (uma vez por biblioteca)
C4  se analysisOptions.warning (padrão true, engine.dart:211):      :315-331
      GatherUsedLocalElementsVisitor em TODAS as unidades → UsedLocalElements.merge   :316-324
      para cada fileAnalysis: _computeWarnings(usedElements)        :325-330 → :459-535
        C4a UnicodeTextVerifier(errorReporter).verify(unit, content)   :466
        C4b DeadCodeVerifier(errorReporter, library)                :468
        C4c BestPracticesVerifier(errorReporter, typeProvider, library, unit, typeSystem,
                inheritanceManager, analysisOptions, workspacePackage)   :470-481
              (dentro: DocCommentVerifier, AnnotationVerifier, DeprecatedMemberUseVerifier,
               ErrorHandlerVerifier, _InvalidAccessVerifier, MustCallSuperVerifier,
               NullSafeApiVerifier — best_practices_verifier.dart:79-116)
        C4d OverrideVerifier(inheritance, library, errorReporter)   :483-487
        C4e RedeclareVerifier(inheritance, library, errorReporter)  :489-493
        C4f TodoFinder(errorReporter).findIn(unit)                  :495
        C4g LanguageVersionOverrideVerifier(errorReporter).verify(unit)   :496
        C4h se !_hasDiagnosticReportedThatPreventsImportWarnings(): :499-510
              ImportsVerifier(fileAnalysis); addImports(unit);
              generateDuplicateExportWarnings; generateDuplicateImportWarnings;
              generateDuplicateShownHiddenNameWarnings; generateUnusedImportHints;
              generateUnusedShownNameHints; generateUnnecessaryImportHints
        C4i UnusedLocalElementsVerifier(errorListener, usedElements, inheritance, library)  :513-520
        C4j se o pacote é PubPackage com `environment: sdk:` válido:
              SdkConstraintVerifier(errorReporter, sdkVersionConstraint.withoutPreRelease)  :526-535
C5  se analysisOptions.lint: _computeLints()                        :333-335 → :352-418  (§8)
C6  _checkForInconsistentLanguageVersionOverride()                  :337 → :230-270
C7  para cada fileAnalysis: IgnoreValidator(errorReporter, errorListener.errors, ignoreInfo,
        unit.lineInfo, analysisOptions.unignorableNames).reportErrors()   :341-348  (§4.2)
```

**Fase D — filtro de ignores** (`_filterIgnoredErrors`, `:540-567`, §4.1), já dentro do `analyze()`.

Pontos finos, todos lidos:

1. **C1 é por arquivo, na ordem a→b→c→d**, e roda para todos os arquivos
   antes de C2. Não há porta de "erro de sintaxe" em nenhum ponto do
   `LibraryAnalyzer`: tudo roda sempre (T5).
2. **A porta dos avisos de import** (`:588-613`) olha os erros de **todas as
   unidades da biblioteca** até aquele momento (inclusive os dos avisos já
   computados nas unidades anteriores) e desliga o `ImportsVerifier` inteiro se
   houver qualquer um destes 14 códigos: `AMBIGUOUS_IMPORT`, `CONST_WITH_NON_TYPE`,
   `EXTENDS_NON_CLASS`, `IMPLEMENTS_NON_CLASS`, `MIXIN_OF_NON_CLASS`,
   `NEW_WITH_NON_TYPE`, `NOT_A_TYPE`, `PREFIX_IDENTIFIER_NOT_FOLLOWED_BY_DOT`,
   `UNDEFINED_ANNOTATION`, `UNDEFINED_CLASS`, `UNDEFINED_FUNCTION`,
   `UNDEFINED_IDENTIFIER`, `UNDEFINED_PREFIXED_NAME`, `WarningCode.DEPRECATED_EXPORT_USE`.
   A comparação é por **constante** (`errorCode`), não por nome: variantes de
   nome compartilhado não contam.
3. **`UnusedLocalElementsVerifier` recebe o `errorListener` cru**, não o
   `ErrorReporter` (`:514-515`): monta os `AnalysisError` por conta própria.
4. **`SdkConstraintVerifier` só roda dentro de um pacote pub com restrição de
   SDK analisável** (`workspace/pub.dart:448-462`); fora de pacote, ou com
   restrição malformada, nenhum `sdk_version_*` sai.
5. **Lints** (`_computeLints`, `:352-418`): pulam partes de macro com `return`
   (não `continue`, `:400`) — a primeira parte de macro encerra o laço e o
   `afterLibrary`. Sem macros (3.6.2 estável) é irrelevante.
6. **C6** relata `inconsistent_language_version_override` no **URI da
   diretiva `part`** do arquivo da biblioteca, quando só um dos dois tem
   `// @dart=` ou quando `major`/`minor` diferem (`:245-266`).
7. `analysisOptions.warning` é `true` por padrão e nenhum caminho do
   `dart analyze` o desliga (`engine.dart:211`; `apply_options.dart` não o toca).

### 1.3 Tabela emissor → fase → códigos

Gerada por varredura das referências `Classe.CONSTANTE` em
`analyzer/lib` e `_fe_analyzer_shared/lib` da 3.6.2
(`E:\dftemp\analise\spec-infra\auto.py`); a coluna "nomes" lista o **nome
emitido** (deduplicado) de toda constante referenciada no arquivo. Uma
referência nem sempre é uma emissão (comparações, listas de supressão): os
casos conhecidos estão na nota depois da tabela. Os códigos do parser fasta
que chegam por índice (`fastaAnalyzerErrorCodes`,
`analyzer/lib/src/fasta/error_converter.dart:557-576`) não aparecem aqui: estão
na Parte II (II.1–II.3) e na §E do outro documento. Fases: 0 scanner, 1 parser,
2 diretivas, 3a/3b/3c resolução (B1/B2/B3), 4 constantes, 5a–5d = C1a–C1d,
6a/6b = C2/C3, 7a–7j = C4a–C4j, 10 = C7.

| fase | emissor (arquivo) | códigos | nomes |
|---|---|---:|---|
| 0 scanner (tradução dos tokens de erro, chamada pelo parser) | `_fe_analyzer_shared/lib/src/scanner/errors.dart` | 9 | `expected_token`, `illegal_character`, `missing_digit`, `missing_hex_digit`, `missing_identifier`, `unexpected_separator_in_number`, `unsupported_operator`, `unterminated_multi_line_comment`, `unterminated_string_literal` |
| 0 scanner (`// @dart=` maior que a versão corrente) | `analyzer/lib/src/dart/scanner/scanner.dart` | 1 | `invalid_language_version_override` |
| 1 parser (`FastaErrorReporter.reportByCode`) | `analyzer/lib/src/fasta/error_converter.dart` | 66 | `async_for_in_wrong_context`, `async_keyword_used_as_identifier`, `await_in_wrong_context`, `built_in_identifier_as_type`, `concrete_class_with_abstract_member`, `const_constructor_with_body`, `const_not_initialized`, `default_value_in_function_type`, `empty_enum_body`, `expected_class_member`, `expected_executable`, `expected_string_literal`, `expected_token`, `expected_type_name`, `field_initializer_redirecting_constructor`, `final_not_initialized`, `final_not_initialized_constructor`, `getter_with_parameters`, `illegal_character`, `import_of_non_library`, `invalid_assignment`, `invalid_cast_function`, `invalid_cast_function_expr`, `invalid_cast_literal_list`, `invalid_cast_literal_map`, `invalid_cast_literal_set`, `invalid_cast_method`, `invalid_cast_new_expr`, `invalid_code_point`, `invalid_generic_function_type`, `invalid_inline_function_type`, `invalid_literal_in_configuration`, `invalid_modifier_on_setter`, `invalid_operator_for_super`, `invalid_override`, `label_undefined`, `missing_digit`, `missing_enum_body`, `missing_function_body`, `missing_function_parameters`, `missing_hex_digit`, `missing_identifier`, `missing_method_parameters`, `missing_star_after_sync`, `missing_typedef_parameters`, `multiple_implements_clauses`, `named_function_expression`, `named_parameter_outside_group`, `non_part_of_directive_in_part`, `non_sync_factory`, `positional_after_named_argument`, `recursive_constructor_redirect`, `return_in_generator`, `super_in_redirecting_constructor`, `super_invocation_not_last`, `undefined_class`, `undefined_getter`, `undefined_method`, `undefined_setter`, `unexpected_dollar_in_string`, `unexpected_token`, `unterminated_multi_line_comment`, `unterminated_string_literal`, `wrong_number_of_parameters_for_setter`, `wrong_separator_for_positional_parameter`, `yield_in_non_generator` |
| 1 parser (`AstBuilder`) | `analyzer/lib/src/fasta/ast_builder.dart` | 11 | `declaration_named_augmented_inside_augmentation`, `expected_named_type`, `expected_representation_field`, `expected_representation_type`, `external_constructor_with_field_initializers`, `invalid_use_of_identifier_augmented`, `member_with_class_name`, `multiple_representation_fields`, `part_of_name`, `representation_field_modifier`, `representation_field_trailing_comma` |
| 1 parser (comentários de documentação) | `analyzer/lib/src/fasta/doc_comment_builder.dart` | 5 | `doc_directive_has_extra_arguments`, `doc_directive_missing_closing_brace`, `doc_directive_missing_closing_tag`, `doc_directive_missing_opening_tag`, `doc_directive_unknown` |
| 2 diretivas (`_resolveDirectives`) e fim (`_checkForInconsistentLanguageVersionOverride`) | `analyzer/lib/src/dart/analysis/library_analyzer.dart` | 26 | `ambiguous_import`, `creation_with_non_type`, `deprecated_export_use`, `duplicate_part`, `export_of_non_library`, `extends_non_class`, `implements_non_class`, `import_of_non_library`, `inconsistent_language_version_override`, `invalid_uri`, `mixin_of_non_class`, `not_a_type`, `part_of_different_library`, `part_of_non_part`, `part_of_unnamed_library`, `prefix_identifier_not_followed_by_dot`, `undefined_annotation`, `undefined_class`, `undefined_function`, `undefined_identifier`, `undefined_prefixed_name`, `uri_does_not_exist`, `uri_does_not_exist_in_doc_import`, `uri_has_not_been_generated`, `uri_with_interpolation`, `use_of_native_extension` |
| antes de tudo (`dart:core`/`dart:async` ausentes) | `analyzer/lib/src/dart/analysis/driver.dart` | 1 | `missing_dart_library` |
| 3a `ResolutionVisitor` | `analyzer/lib/src/dart/resolver/resolution_visitor.dart` | 13 | `duplicate_variable_pattern`, `extends_non_class`, `extension_type_implements_disallowed_type`, `extension_type_implements_not_supertype`, `extension_type_implements_representation_not_supertype`, `implements_non_class`, `missing_variable_pattern`, `mixin_of_non_class`, `mixin_super_class_constraint_non_interface`, `mixin_with_non_class_superclass`, `pattern_assignment_not_local_variable`, `sdk_version_constructor_tearoffs`, `undefined_identifier` |
| 3a `ResolutionVisitor` (`NamedTypeResolver`) | `analyzer/lib/src/dart/resolver/named_type_resolver.dart` | 20 | `cast_to_non_type`, `creation_with_non_type`, `instantiate_type_alias_expands_to_type_parameter`, `non_type_as_type_argument`, `non_type_in_catch_clause`, `not_a_type`, `nullable_type_in_extends_clause`, `nullable_type_in_implements_clause`, `nullable_type_in_on_clause`, `nullable_type_in_with_clause`, `prefix_shadowed_by_local_declaration`, `redirect_to_non_class`, `redirect_to_type_alias_expands_to_type_parameter`, `supertype_expands_to_type_parameter`, `type_test_with_non_type`, `type_test_with_undefined_name`, `undefined_class`, `undefined_identifier_await`, `wrong_number_of_type_arguments`, `wrong_number_of_type_arguments_constructor` |
| 3a `ResolutionVisitor` | `analyzer/lib/src/dart/resolver/record_type_annotation_resolver.dart` | 1 | `invalid_field_name` |
| 3a/3c reescrita da AST | `analyzer/lib/src/dart/resolver/ast_rewrite.dart` | 1 | `wrong_number_of_type_arguments_constructor` |
| 3a/3c escopos | `analyzer/lib/src/generated/scope_helpers.dart` | 1 | `deprecated_export_use` |
| 3b `ScopeResolverVisitor` e 3c `ResolverVisitor` | `analyzer/lib/src/generated/resolver.dart` | 34 | `assignment_to_final_local`, `augmented_expression_is_not_setter`, `augmented_expression_is_setter`, `body_might_complete_normally`, `body_might_complete_normally_catch_error`, `body_might_complete_normally_nullable`, `cast_from_nullable_always_fails`, `continue_label_invalid`, `definitely_unassigned_late_local_variable`, `duplicate_named_argument`, `enum_constant_invokes_factory_constructor`, `expected_two_map_pattern_type_arguments`, `extra_positional_arguments`, `extra_positional_arguments_could_be_named`, `invalid_pattern_variable_in_shared_case_scope`, `invocation_of_non_function_expression`, `label_in_outer_scope`, `label_undefined`, `late_final_local_already_assigned`, `missing_named_pattern_field_name`, `non_bool_expression`, `not_assigned_potentially_non_nullable_local_variable`, `not_enough_positional_arguments`, `pattern_never_matches_value_type`, `pattern_variable_assignment_inside_guard`, `positional_field_in_object_pattern`, `read_potentially_unassigned_final`, `top_level_cycle`, `unchecked_use_of_nullable_value`, `undefined_enum_constructor`, `undefined_getter`, `undefined_identifier`, `undefined_named_parameter`, `undefined_operator` |
| 3c `ResolverVisitor` (`ElementResolver`) | `analyzer/lib/src/generated/element_resolver.dart` | 5 | `non_generative_constructor`, `super_in_extension`, `super_in_extension_type`, `super_in_invalid_context`, `undefined_constructor_in_initializer` |
| 3c e 5c (mixin `ErrorDetectionHelpers`) | `analyzer/lib/src/generated/error_detection_helpers.dart` | 4 | `argument_type_not_assignable`, `field_initializer_not_assignable`, `record_literal_one_positional_no_trailing_comma`, `use_of_void_result` |
| 3c inferência genérica | `analyzer/lib/src/dart/element/generic_inferrer.dart` | 4 | `could_not_infer`, `inference_failure_on_function_invocation`, `inference_failure_on_generic_invocation`, `inference_failure_on_instance_creation` |
| 3c/5c (fábrica com `contextMessages`) | `analyzer/lib/src/diagnostic/diagnostic_factory.dart` | 10 | `duplicate_field_name`, `duplicate_pattern_assignment_variable`, `duplicate_pattern_field`, `duplicate_rest_element_in_pattern`, `equal_elements_in_const_set`, `equal_keys_in_const_map`, `equal_keys_in_map_pattern`, `invalid_null_aware_operator`, `invalid_override`, `referenced_before_declaration` |
| 3c (dos resolvedores de propriedade) | `analyzer/lib/src/error/assignment_verifier.dart` | 9 | `assignment_to_const`, `assignment_to_final`, `assignment_to_final_no_setter`, `assignment_to_function`, `assignment_to_method`, `assignment_to_type`, `prefix_identifier_not_followed_by_dot`, `undefined_identifier`, `undefined_setter` |
| 3c | `analyzer/lib/src/error/bool_expression_verifier.dart` | 4 | `non_bool_condition`, `non_bool_negation_expression`, `unchecked_use_of_nullable_value`, `use_of_void_result` |
| 3c | `analyzer/lib/src/error/nullable_dereference_verifier.dart` | 1 | `invalid_use_of_null_value` |
| 3c (`NullSafetyDeadCodeVerifier`, fluxo) e 7b (`DeadCodeVerifier`) | `analyzer/lib/src/error/dead_code_verifier.dart` | 6 | `dead_code`, `dead_code_catch_following_catch`, `dead_code_on_catch_subtype`, `undefined_hidden_name`, `undefined_shown_name`, `unused_label` |
| 3c (instanciado no `ResolverVisitor`) | `analyzer/lib/src/error/base_or_final_type_verifier.dart` | 2 | `invalid_use_of_type_outside_library`, `subtype_of_base_or_final_is_not_base_final_or_sealed` |
| 4 `computeConstants` e 5a `ConstantVerifier` | `analyzer/lib/src/dart/constant/evaluation.dart` | 36 | `collection_element_from_deferred_library`, `const_constructor_field_type_mismatch`, `const_constructor_param_type_mismatch`, `const_eval_assertion_failure`, `const_eval_assertion_failure_with_message`, `const_eval_extension_method`, `const_eval_extension_type_method`, `const_eval_for_element`, `const_eval_method_invocation`, `const_eval_property_access`, `const_eval_throws_exception`, `const_eval_type_bool`, `const_eval_type_bool_num_string`, `const_initialized_with_non_constant_value_from_deferred_library`, `const_spread_expected_list_or_set`, `const_spread_expected_map`, `const_type_parameter`, `const_with_non_const`, `const_with_type_parameters`, `expression_in_map`, `if_element_condition_from_deferred_library`, `invalid_annotation_constant_value_from_deferred_library`, `invalid_constant`, `map_entry_not_in_map`, `missing_const_in_list_literal`, `missing_const_in_map_literal`, `missing_const_in_set_literal`, `non_bool_condition`, `non_constant_case_expression_from_deferred_library`, `non_constant_default_value_from_deferred_library`, `non_constant_record_field_from_deferred_library`, `pattern_constant_from_deferred_library`, `recursive_compile_time_constant`, `spread_expression_from_deferred_library`, `variable_type_mismatch`, `wrong_number_of_type_arguments_function` |
| 4/5a (avaliador) | `analyzer/lib/src/dart/constant/value.dart` | 12 | `const_eval_throws_exception`, `const_eval_throws_idbze`, `const_eval_type_bool`, `const_eval_type_bool_int`, `const_eval_type_bool_num_string`, `const_eval_type_int`, `const_eval_type_num`, `const_eval_type_num_string`, `const_eval_type_string`, `const_eval_type_type`, `const_with_non_constant_argument`, `invalid_constant` |
| 5a `ConstantVerifier` | `analyzer/lib/src/dart/constant/constant_verifier.dart` | 64 | `case_expression_type_implements_equals`, `collection_element_from_deferred_library`, `const_constructor_field_type_mismatch`, `const_constructor_param_type_mismatch`, `const_constructor_with_field_initialized_by_non_const`, `const_eval_extension_method`, `const_eval_extension_type_method`, `const_eval_for_element`, `const_eval_method_invocation`, `const_eval_property_access`, `const_eval_throws_exception`, `const_eval_throws_idbze`, `const_eval_type_bool`, `const_eval_type_bool_int`, `const_eval_type_bool_num_string`, `const_eval_type_int`, `const_eval_type_num`, `const_eval_type_num_string`, `const_eval_type_string`, `const_initialized_with_non_constant_value`, `const_initialized_with_non_constant_value_from_deferred_library`, `const_map_key_not_primitive_equality`, `const_set_element_not_primitive_equality`, `const_spread_expected_list_or_set`, `const_spread_expected_map`, `const_type_parameter`, `const_with_non_constant_argument`, `const_with_type_parameters`, `constant_pattern_never_matches_value_type`, `constant_pattern_with_non_constant_expression`, `expression_in_map`, `if_element_condition_from_deferred_library`, `invalid_annotation_constant_value_from_deferred_library`, `invalid_constant`, `list_element_type_not_assignable`, `map_key_type_not_assignable`, `map_value_type_not_assignable`, `no_annotation_constructor_arguments`, `non_bool_condition`, `non_constant_annotation_constructor`, `non_constant_case_expression`, `non_constant_case_expression_from_deferred_library`, `non_constant_default_value`, `non_constant_default_value_from_deferred_library`, `non_constant_list_element`, `non_constant_map_element`, `non_constant_map_key`, `non_constant_map_pattern_key`, `non_constant_map_value`, `non_constant_record_field`, `non_constant_record_field_from_deferred_library`, `non_constant_relational_pattern_expression`, `non_constant_set_element`, `non_exhaustive_switch_expression`, `non_exhaustive_switch_statement`, `pattern_constant_from_deferred_library`, `recursive_compile_time_constant`, `recursive_constant_constructor`, `set_element_type_not_assignable`, `spread_expression_from_deferred_library`, `unreachable_switch_case`, `unreachable_switch_default`, `variable_type_mismatch`, `wrong_number_of_type_arguments_function` |
| 5a/5c (`LiteralElementVerifier`) | `analyzer/lib/src/error/literal_element_verifier.dart` | 9 | `expression_in_map`, `list_element_type_not_assignable`, `map_entry_not_in_map`, `map_key_type_not_assignable`, `map_value_type_not_assignable`, `not_iterable_spread`, `not_map_spread`, `not_null_aware_null_spread`, `set_element_type_not_assignable` |
| 5b `InheritanceOverrideVerifier` | `analyzer/lib/src/error/inheritance_override.dart` | 15 | `concrete_class_has_enum_superinterface`, `concrete_class_with_abstract_member`, `enum_mixin_with_instance_variable`, `enum_with_abstract_member`, `illegal_concrete_enum_member`, `illegal_enum_values`, `inconsistent_inheritance`, `inconsistent_inheritance_getter_and_method`, `invalid_implementation_override`, `invalid_override`, `missing_override_of_must_be_overridden`, `no_combined_super_signature`, `non_abstract_class_inherits_abstract_member`, `recursive_interface_inheritance`, `subtype_of_disallowed_type` |
| 5b/5c | `analyzer/lib/src/error/correct_override.dart` | 1 | `invalid_override` |
| 5b/5c | `analyzer/lib/src/error/getter_setter_types_verifier.dart` | 1 | `getter_not_subtype_setter_types` |
| 5c `ErrorVerifier` | `analyzer/lib/src/generated/error_verifier.dart` | 175 | `abstract_field_initializer`, `ambiguous_export`, `ambiguous_import`, `argument_type_not_assignable`, `assert_in_redirecting_constructor`, `assignment_to_const`, `assignment_to_final`, `assignment_to_final_no_setter`, `assignment_to_function`, `assignment_to_method`, `assignment_to_type`, `augmentation_extends_clause_already_present`, `augmentation_modifier_extra`, `augmentation_modifier_missing`, `augmentation_of_different_declaration_kind`, `augmentation_type_parameter_bound`, `augmentation_type_parameter_count`, `augmentation_type_parameter_name`, `augmentation_without_declaration`, `await_in_late_local_variable_initializer`, `await_in_wrong_context`, `await_of_incompatible_type`, `break_label_on_switch_member`, `built_in_identifier_in_declaration`, `class_used_as_mixin`, `conflicting_field_and_method`, `conflicting_generic_interfaces`, `conflicting_inherited_method_and_setter`, `conflicting_method_and_field`, `conflicting_static_and_instance`, `conflicting_type_variable_and_container`, `conflicting_type_variable_and_member`, `const_constructor_throws_exception`, `const_constructor_with_mixin_with_field`, `const_constructor_with_non_const_super`, `const_constructor_with_non_final_field`, `const_deferred_class`, `const_instance_field`, `const_not_initialized`, `const_with_non_const`, `const_with_undefined_constructor`, `const_with_undefined_constructor_default`, `dead_null_aware_expression`, `default_value_in_redirecting_factory_constructor`, `default_value_on_required_parameter`, `deferred_import_of_extension`, `deprecated_subtype_of_function`, `enum_instantiated_to_bounds_is_not_well_bounded`, `enum_without_constants`, `export_internal_library`, `extension_declares_member_of_object`, `extension_type_constructor_with_super_formal_parameter`, `extension_type_constructor_with_super_invocation`, `extension_type_declares_instance_field`, `extension_type_declares_member_of_object`, `extension_type_implements_itself`, `extension_type_inherited_member_conflict`, `extension_type_representation_depends_on_itself`, `extension_type_representation_type_bottom`, `extension_type_with_abstract_member`, `external_with_initializer`, `field_initializer_factory_constructor`, `field_initializer_outside_constructor`, `field_initializer_redirecting_constructor`, `field_initializing_formal_not_assignable`, `final_not_initialized`, `for_in_of_invalid_element_type`, `for_in_of_invalid_type`, `for_in_with_const_variable`, `generic_function_type_cannot_be_bound`, `illegal_language_version_override`, `implements_repeated`, `implements_super_class`, `implicit_super_initializer_missing_arguments`, `implicit_this_reference_in_initializer`, `import_internal_library`, `initializer_for_non_existent_field`, `initializer_for_static_field`, `initializing_formal_for_non_existent_field`, `instance_member_access_from_factory`, `instance_member_access_from_static`, `instantiate_abstract_class`, `instantiate_enum`, `integer_literal_imprecise_as_double`, `integer_literal_out_of_range`, `invalid_annotation_from_deferred_library`, `invalid_assignment`, `invalid_macro_application_target`, `invalid_modifier_on_constructor`, `invalid_null_aware_operator`, `invalid_reference_to_generative_enum_constructor`, `invalid_reference_to_this`, `invalid_super_formal_parameter_location`, `invalid_use_of_covariant`, `invalid_use_of_type_outside_library`, `late_final_field_with_const_constructor`, `macro_application_argument_error`, `macro_declarations_phase_introspection_cycle`, `macro_definition_application_same_library_cycle`, `macro_error`, `macro_info`, `macro_internal_exception`, `macro_not_allowed_declaration`, `macro_warning`, `main_first_positional_parameter_type`, `main_has_required_named_parameters`, `main_has_too_many_required_positional_parameters`, `main_is_not_function`, `missing_default_value_for_parameter`, `missing_enum_constant_in_switch`, `mixin_application_concrete_super_invoked_member_type`, `mixin_application_no_concrete_super_invoked_member`, `mixin_application_not_implemented_interface`, `mixin_class_declaration_extends_not_object`, `mixin_class_declares_constructor`, `mixin_inherits_from_not_object`, `mixin_instantiate`, `mixin_super_class_constraint_deferred_class`, `multiple_redirecting_constructor_invocations`, `multiple_super_initializers`, `native_clause_in_non_sdk_code`, `native_function_body_in_non_sdk_code`, `new_with_undefined_constructor`, `new_with_undefined_constructor_default`, `no_default_super_constructor`, `no_generative_constructors_in_superclass`, `non_const_generative_enum_constructor`, `non_const_map_as_expression_statement`, `non_covariant_type_parameter_position_in_representation_type`, `non_final_field_in_enum`, `non_generative_constructor`, `non_generative_implicit_constructor`, `non_void_return_for_operator`, `non_void_return_for_setter`, `not_initialized_non_nullable_instance_field`, `not_initialized_non_nullable_variable`, `not_instantiated_bound`, `on_repeated`, `optional_parameter_in_operator`, `private_collision_in_mixin_application`, `private_optional_parameter`, `recursive_constructor_redirect`, `redirect_generative_to_missing_constructor`, `redirect_generative_to_non_generative_constructor`, `redirect_to_abstract_class_constructor`, `redirect_to_invalid_function_type`, `redirect_to_invalid_return_type`, `redirect_to_missing_constructor`, `redirect_to_non_const_constructor`, `rethrow_outside_catch`, `return_in_generative_constructor`, `shared_deferred_prefix`, `static_access_to_instance_member`, `subtype_of_deferred_class`, `subtype_of_disallowed_type`, `super_formal_parameter_type_is_not_subtype_of_associated`, `super_formal_parameter_without_associated_named`, `super_formal_parameter_without_associated_positional`, `super_in_enum_constructor`, `super_in_redirecting_constructor`, `super_invocation_not_last`, `throw_of_invalid_type`, `type_alias_cannot_reference_itself`, `type_annotation_deferred_class`, `type_parameter_referenced_by_static`, `type_parameter_supertype_of_its_bound`, `undefined_constructor_in_initializer`, `unnecessary_non_null_assertion`, `unqualified_reference_to_non_local_static_member`, `unqualified_reference_to_static_member_of_extended_type`, `wrong_explicit_type_parameter_variance_in_superinterface`, `wrong_number_of_parameters_for_operator`, `wrong_number_of_parameters_for_setter`, `wrong_type_parameter_variance_in_superinterface`, `wrong_type_parameter_variance_position` |
| 5c (`DuplicateDefinitionVerifier`) e 6a (`MemberDuplicateDefinitionVerifier`) | `analyzer/lib/src/error/duplicate_definition_verifier.dart` | 12 | `conflicting_constructor_and_static_member`, `conflicting_field_and_method`, `conflicting_method_and_field`, `conflicting_static_and_instance`, `duplicate_constructor`, `duplicate_definition`, `duplicate_field_formal_parameter`, `enum_constant_same_name_as_enclosing`, `enum_with_name_values`, `extension_conflicting_static_and_instance`, `prefix_collides_with_top_level_member`, `values_declaration_in_enum` |
| 5c | `analyzer/lib/src/error/type_arguments_verifier.dart` | 8 | `expected_one_list_type_arguments`, `expected_one_set_type_arguments`, `expected_two_map_type_arguments`, `generic_function_type_cannot_be_type_argument`, `invalid_type_argument_in_const_literal`, `strict_raw_type`, `type_argument_not_matching_bounds`, `wrong_number_of_type_arguments_enum` |
| 5c (e 7c pelo `ErrorHandlerVerifier`) | `analyzer/lib/src/error/return_type_verifier.dart` | 9 | `illegal_async_generator_return_type`, `illegal_async_return_type`, `illegal_sync_generator_return_type`, `invalid_return_type_for_catch_error`, `record_literal_one_positional_no_trailing_comma`, `return_in_generative_constructor`, `return_of_invalid_type`, `return_of_invalid_type_from_closure`, `return_without_value` |
| 5c | `analyzer/lib/src/error/required_parameters_verifier.dart` | 2 | `missing_required_argument`, `missing_required_param` |
| 5c | `analyzer/lib/src/error/const_argument_verifier.dart` | 1 | `non_const_argument_for_const_parameter` |
| 5c | `analyzer/lib/src/error/use_result_verifier.dart` | 1 | `unused_result` |
| 5c | `analyzer/lib/src/error/super_formal_parameters_verifier.dart` | 1 | `positional_super_formal_parameter_with_positional_argument` |
| 5d `FfiVerifier` | `analyzer/lib/src/generated/ffi_verifier.dart` | 46 | `abi_specific_integer_invalid`, `abi_specific_integer_mapping_extra`, `abi_specific_integer_mapping_missing`, `abi_specific_integer_mapping_unsupported`, `address_position`, `address_receiver`, `annotation_on_pointer_field`, `argument_must_be_a_constant`, `argument_must_be_native`, `compound_implements_finalizable`, `creation_of_struct_or_union`, `empty_struct`, `extra_annotation_on_struct_field`, `extra_size_annotation_carray`, `ffi_native_invalid_duplicate_default_asset`, `ffi_native_invalid_multiple_annotations`, `ffi_native_must_be_external`, `ffi_native_only_classes_extending_nativefieldwrapperclass1_can_be_pointer`, `ffi_native_unexpected_number_of_parameters`, `ffi_native_unexpected_number_of_parameters_with_receiver`, `field_must_be_external_in_struct`, `generic_struct_subclass`, `invalid_exception_value`, `invalid_field_type_in_struct`, `leaf_call_must_not_return_handle`, `leaf_call_must_not_take_handle`, `mismatched_annotation_on_struct_field`, `missing_annotation_on_struct_field`, `missing_exception_value`, `missing_field_type_in_struct`, `missing_size_annotation_carray`, `must_be_a_native_function_type`, `must_be_a_subtype`, `must_return_void`, `native_field_invalid_type`, `native_field_missing_type`, `native_field_not_static`, `non_constant_type_argument`, `non_native_function_type_argument_to_pointer`, `non_positive_array_dimension`, `non_sized_type_argument`, `packed_annotation`, `packed_annotation_alignment`, `size_annotation_dimensions`, `subtype_of_struct_class`, `variable_length_array_not_last` |
| 6b `ConstructorFieldsVerifier.report` | `analyzer/lib/src/error/constructor_fields_verifier.dart` | 6 | `field_initialized_by_multiple_initializers`, `field_initialized_in_initializer_and_declaration`, `field_initialized_in_parameter_and_initializer`, `final_initialized_in_declaration_and_constructor`, `final_not_initialized_constructor`, `not_initialized_non_nullable_instance_field` |
| 7a | `analyzer/lib/src/error/unicode_text_verifier.dart` | 2 | `text_direction_code_point_in_comment`, `text_direction_code_point_in_literal` |
| 7c `BestPracticesVerifier` | `analyzer/lib/src/error/best_practices_verifier.dart` | 38 | `assignment_of_do_not_store`, `cast_from_null_always_fails`, `deprecated_colon_for_default_value`, `deprecated_new_in_comment_reference`, `equal_elements_in_set`, `equal_keys_in_map`, `import_deferred_library_with_load_function`, `inference_failure_on_function_return_type`, `inference_failure_on_untyped_parameter`, `invalid_export_of_internal_element`, `invalid_export_of_internal_element_indirectly`, `invalid_override_of_non_virtual_member`, `invalid_required_named_param`, `invalid_required_optional_positional_param`, `invalid_required_positional_param`, `invalid_use_of_internal_member`, `invalid_use_of_protected_member`, `invalid_use_of_visible_for_overriding_member`, `invalid_use_of_visible_for_template_member`, `invalid_use_of_visible_for_testing_member`, `mixin_on_sealed_class`, `must_be_immutable`, `non_const_call_to_literal_constructor`, `non_nullable_equals_parameter`, `null_check_always_fails`, `nullable_type_in_catch_clause`, `obsolete_colon_for_default_value`, `return_of_do_not_store`, `subtype_of_sealed_class`, `type_check_with_null`, `unnecessary_cast`, `unnecessary_final`, `unnecessary_nan_comparison`, `unnecessary_no_such_method`, `unnecessary_null_comparison`, `unnecessary_question_mark`, `unnecessary_set_literal`, `unnecessary_type_check` |
| 7c | `analyzer/lib/src/error/annotation_verifier.dart` | 11 | `invalid_annotation_target`, `invalid_factory_method_decl`, `invalid_factory_method_impl`, `invalid_internal_annotation`, `invalid_literal_annotation`, `invalid_non_virtual_annotation`, `invalid_reopen_annotation`, `invalid_visibility_annotation`, `invalid_visible_for_overriding_annotation`, `invalid_visible_outside_template_annotation`, `undefined_referenced_parameter` |
| 7c | `analyzer/lib/src/error/deprecated_member_use_verifier.dart` | 2 | `deprecated_member_use`, `deprecated_member_use_from_same_package` |
| 7c | `analyzer/lib/src/error/doc_comment_verifier.dart` | 6 | `doc_directive_argument_wrong_format`, `doc_directive_has_extra_arguments`, `doc_directive_has_unexpected_named_argument`, `doc_directive_missing_argument`, `doc_import_cannot_be_deferred`, `doc_import_cannot_have_configurations` |
| 7c | `analyzer/lib/src/error/error_handler_verifier.dart` | 2 | `argument_type_not_assignable_to_error_handler`, `invalid_return_type_for_catch_error` |
| 7c | `analyzer/lib/src/error/must_call_super_verifier.dart` | 1 | `must_call_super` |
| 7c | `analyzer/lib/src/error/null_safe_api_verifier.dart` | 1 | `null_argument_to_non_null_type` |
| 7d | `analyzer/lib/src/error/override_verifier.dart` | 1 | `override_on_non_overriding_member` |
| 7e | `analyzer/lib/src/error/redeclare_verifier.dart` | 1 | `redeclare_on_non_redeclaring_member` |
| 7f `TodoFinder` | `analyzer/lib/src/dart/error/todo_codes.dart` | 4 | `fixme`, `hack`, `todo`, `undone` |
| 7g | `analyzer/lib/src/error/language_version_override_verifier.dart` | 1 | `invalid_language_version_override` |
| 7h | `analyzer/lib/src/error/imports_verifier.dart` | 7 | `duplicate_export`, `duplicate_hidden_name`, `duplicate_import`, `duplicate_shown_name`, `unnecessary_import`, `unused_import`, `unused_shown_name` |
| 7i | `analyzer/lib/src/error/unused_local_elements_verifier.dart` | 5 | `unused_catch_clause`, `unused_catch_stack`, `unused_element`, `unused_field`, `unused_local_variable` |
| 7j | `analyzer/lib/src/hint/sdk_constraint_verifier.dart` | 2 | `sdk_version_gt_gt_gt_operator`, `sdk_version_since` |
| 10 `IgnoreValidator` | `analyzer/lib/src/error/ignore_validator.dart` | 1 | `duplicate_ignore` |
| 3c `ResolverVisitor` (resolvedor auxiliar) | `analyzer/lib/src/dart/resolver/annotation_resolver.dart` | 2 | `invalid_annotation`, `undefined_annotation` |
| 3c `ResolverVisitor` (resolvedor auxiliar) | `analyzer/lib/src/dart/resolver/assignment_expression_resolver.dart` | 6 | `assignment_to_final_local`, `invalid_assignment`, `late_final_local_already_assigned`, `record_literal_one_positional_no_trailing_comma`, `undefined_operator`, `use_of_void_result` |
| 3c `ResolverVisitor` (resolvedor auxiliar) | `analyzer/lib/src/dart/resolver/binary_expression_resolver.dart` | 9 | `augmented_expression_is_setter`, `augmented_expression_not_operator`, `non_bool_operand`, `not_binary_operator`, `receiver_of_type_never`, `undefined_extension_operator`, `undefined_operator`, `undefined_super_member`, `unnecessary_null_comparison` |
| 3c `ResolverVisitor` (resolvedor auxiliar) | `analyzer/lib/src/dart/resolver/constructor_reference_resolver.dart` | 3 | `class_instantiation_access_to_member`, `sdk_version_constructor_tearoffs`, `tearoff_of_generative_constructor_of_abstract_class` |
| 3c `ResolverVisitor` (resolvedor auxiliar) | `analyzer/lib/src/dart/resolver/extension_member_resolver.dart` | 7 | `ambiguous_extension_member_access`, `extension_override_argument_not_assignable`, `extension_override_without_access`, `invalid_extension_argument_count`, `type_argument_not_matching_bounds`, `use_of_void_result`, `wrong_number_of_type_arguments_extension` |
| 3c `ResolverVisitor` (resolvedor auxiliar) | `analyzer/lib/src/dart/resolver/for_resolver.dart` | 1 | `unchecked_use_of_nullable_value` |
| 3c `ResolverVisitor` (resolvedor auxiliar) | `analyzer/lib/src/dart/resolver/function_expression_invocation_resolver.dart` | 6 | `extension_override_access_to_static_member`, `invocation_of_extension_without_call`, `invocation_of_non_function_expression`, `receiver_of_type_never`, `unchecked_use_of_nullable_value`, `use_of_void_result` |
| 3c `ResolverVisitor` (resolvedor auxiliar) | `analyzer/lib/src/dart/resolver/function_reference_resolver.dart` | 13 | `disallowed_type_instantiation_expression`, `extension_override_access_to_static_member`, `extension_override_with_cascade`, `generic_method_type_instantiation_on_dynamic`, `instance_access_to_static_member`, `undefined_identifier`, `undefined_method`, `undefined_prefixed_name`, `unqualified_reference_to_non_local_static_member`, `unqualified_reference_to_static_member_of_extended_type`, `wrong_number_of_type_arguments`, `wrong_number_of_type_arguments_constructor`, `wrong_number_of_type_arguments_function` |
| 3c `ResolverVisitor` (resolvedor auxiliar) | `analyzer/lib/src/dart/resolver/invocation_inferrer.dart` | 3 | `type_argument_not_matching_bounds`, `wrong_number_of_type_arguments`, `wrong_number_of_type_arguments_method` |
| 3c `ResolverVisitor` (resolvedor auxiliar) | `analyzer/lib/src/dart/resolver/list_pattern_resolver.dart` | 1 | `expected_one_list_pattern_type_arguments` |
| 3c `ResolverVisitor` (resolvedor auxiliar) | `analyzer/lib/src/dart/resolver/method_invocation_resolver.dart` | 16 | `abstract_super_member_reference`, `extension_override_access_to_static_member`, `extension_override_with_cascade`, `instance_access_to_static_member`, `invocation_of_non_function`, `new_with_undefined_constructor_default`, `prefix_identifier_not_followed_by_dot`, `receiver_of_type_never`, `static_access_to_instance_member`, `undefined_extension_method`, `undefined_function`, `undefined_method`, `undefined_super_member`, `unqualified_reference_to_non_local_static_member`, `unqualified_reference_to_static_member_of_extended_type`, `use_of_void_result` |
| 3c `ResolverVisitor` (resolvedor auxiliar) | `analyzer/lib/src/dart/resolver/postfix_expression_resolver.dart` | 5 | `invalid_assignment`, `missing_assignable_selector`, `receiver_of_type_never`, `undefined_operator`, `undefined_super_member` |
| 3c `ResolverVisitor` (resolvedor auxiliar) | `analyzer/lib/src/dart/resolver/prefix_expression_resolver.dart` | 7 | `augmented_expression_is_setter`, `augmented_expression_not_operator`, `invalid_assignment`, `receiver_of_type_never`, `undefined_extension_operator`, `undefined_operator`, `undefined_super_member` |
| 3c `ResolverVisitor` (resolvedor auxiliar) | `analyzer/lib/src/dart/resolver/prefixed_identifier_resolver.dart` | 1 | `extension_as_expression` |
| 3c `ResolverVisitor` (resolvedor auxiliar) | `analyzer/lib/src/dart/resolver/property_element_resolver.dart` | 17 | `abstract_super_member_reference`, `extension_override_access_to_static_member`, `extension_override_with_cascade`, `instance_access_to_static_member`, `private_setter`, `receiver_of_type_never`, `static_access_to_instance_member`, `undefined_enum_constant`, `undefined_extension_getter`, `undefined_extension_operator`, `undefined_extension_setter`, `undefined_getter`, `undefined_operator`, `undefined_prefixed_name`, `undefined_setter`, `undefined_super_member`, `use_of_void_result` |
| 3c `ResolverVisitor` (resolvedor auxiliar) | `analyzer/lib/src/dart/resolver/record_literal_resolver.dart` | 2 | `invalid_field_name`, `use_of_void_result` |
| 3c `ResolverVisitor` (resolvedor auxiliar) | `analyzer/lib/src/dart/resolver/shared_type_analyzer.dart` | 15 | `case_expression_type_is_not_switch_expression_subtype`, `empty_map_pattern`, `for_in_of_invalid_type`, `inconsistent_pattern_variable_logical_or`, `non_bool_condition`, `pattern_type_mismatch_in_irrefutable_context`, `refutable_pattern_in_irrefutable_context`, `relational_pattern_operand_type_not_assignable`, `relational_pattern_operator_return_type_not_assignable_to_bool`, `rest_element_in_map_pattern`, `switch_case_completes_normally`, `unnecessary_cast_pattern`, `unnecessary_null_assert_pattern`, `unnecessary_null_check_pattern`, `unnecessary_wildcard_pattern` |
| 3c `ResolverVisitor` (resolvedor auxiliar) | `analyzer/lib/src/dart/resolver/simple_identifier_resolver.dart` | 5 | `extension_as_expression`, `invalid_factory_name_not_a_class`, `prefix_identifier_not_followed_by_dot`, `undefined_identifier`, `undefined_identifier_await` |
| 3c `ResolverVisitor` (resolvedor auxiliar) | `analyzer/lib/src/dart/resolver/type_property_resolver.dart` | 1 | `unchecked_use_of_nullable_value` |
| 3c `ResolverVisitor` (resolvedor auxiliar) | `analyzer/lib/src/dart/resolver/typed_literal_resolver.dart` | 3 | `ambiguous_set_or_map_literal_both`, `ambiguous_set_or_map_literal_either`, `inference_failure_on_collection_literal` |
| 3c `ResolverVisitor` (resolvedor auxiliar) | `analyzer/lib/src/dart/resolver/variable_declaration_resolver.dart` | 2 | `inference_failure_on_uninitialized_variable`, `invalid_assignment` |
| 3c `ResolverVisitor` (resolvedor auxiliar) | `analyzer/lib/src/dart/resolver/yield_statement_resolver.dart` | 4 | `unchecked_use_of_nullable_value`, `use_of_void_result`, `yield_in_non_generator`, `yield_of_invalid_type` |
| fora do `LibraryAnalyzer` (arquivo não-Dart, §7) | `analyzer/lib/src/lint/options_rule_validator.dart` | 7 | `deprecated_lint`, `deprecated_lint_with_replacement`, `duplicate_rule`, `incompatible_lint`, `removed_lint`, `replaced_lint`, `undefined_lint` |
| fora do `LibraryAnalyzer` (arquivo não-Dart, §7) | `analyzer/lib/src/manifest/manifest_validator.dart` | 7 | `camera_permissions_incompatible`, `no_touchscreen_feature`, `non_resizable_activity`, `permission_implies_unsupported_hardware`, `setting_orientation_on_activity`, `unsupported_chrome_os_feature`, `unsupported_chrome_os_hardware` |
| fora do `LibraryAnalyzer` (arquivo não-Dart, §7) | `analyzer/lib/src/pubspec/validators/dependency_validator.dart` | 6 | `dependencies_field_not_map`, `invalid_dependency`, `path_does_not_exist`, `path_not_posix`, `path_pubspec_does_not_exist`, `unnecessary_dev_dependency` |
| fora do `LibraryAnalyzer` (arquivo não-Dart, §7) | `analyzer/lib/src/pubspec/validators/field_validator.dart` | 1 | `deprecated_field` |
| fora do `LibraryAnalyzer` (arquivo não-Dart, §7) | `analyzer/lib/src/pubspec/validators/flutter_validator.dart` | 8 | `asset_directory_does_not_exist`, `asset_does_not_exist`, `asset_field_not_list`, `asset_missing_path`, `asset_not_string`, `asset_not_string_or_map`, `asset_path_not_string`, `flutter_field_not_map` |
| fora do `LibraryAnalyzer` (arquivo não-Dart, §7) | `analyzer/lib/src/pubspec/validators/missing_dependency_validator.dart` | 2 | `dependencies_field_not_map`, `missing_dependency` |
| fora do `LibraryAnalyzer` (arquivo não-Dart, §7) | `analyzer/lib/src/pubspec/validators/name_validator.dart` | 2 | `missing_name`, `name_not_string` |
| fora do `LibraryAnalyzer` (arquivo não-Dart, §7) | `analyzer/lib/src/pubspec/validators/platforms_validator.dart` | 3 | `invalid_platforms_field`, `platform_value_disallowed`, `unknown_platform` |
| fora do `LibraryAnalyzer` (arquivo não-Dart, §7) | `analyzer/lib/src/pubspec/validators/screenshot_validator.dart` | 1 | `path_does_not_exist` |
| fora do `LibraryAnalyzer` (arquivo não-Dart, §7) | `analyzer/lib/src/pubspec/validators/workspace_validator.dart` | 4 | `path_does_not_exist`, `workspace_field_not_list`, `workspace_value_not_string`, `workspace_value_not_subdirectory` |
| fora do `LibraryAnalyzer` (arquivo não-Dart, §7) | `analyzer/lib/src/task/options.dart` | 13 | `include_file_not_found`, `included_file_parse_error`, `included_file_warning`, `invalid_option`, `invalid_section_format`, `multiple_plugins`, `parse_error`, `recursive_include_file`, `unrecognized_error_code`, `unsupported_option_with_legal_value`, `unsupported_option_with_legal_values`, `unsupported_option_without_values`, `unsupported_value` |
| (referência que não é emissão: ver nota) | `analyzer/lib/src/lint/linter.dart` | 33 | `collection_element_from_deferred_library`, `const_constructor_with_field_initialized_by_non_const`, `const_eval_extension_method`, `const_eval_extension_type_method`, `const_eval_for_element`, `const_eval_method_invocation`, `const_eval_property_access`, `const_eval_throws_exception`, `const_eval_throws_idbze`, `const_eval_type_bool`, `const_eval_type_bool_int`, `const_eval_type_bool_num_string`, `const_eval_type_int`, `const_eval_type_num`, `const_eval_type_num_string`, `const_eval_type_string`, `const_map_key_not_primitive_equality`, `const_set_element_not_primitive_equality`, `const_type_parameter`, `const_with_non_const`, `const_with_non_constant_argument`, `const_with_type_parameters`, `invalid_constant`, `missing_const_in_list_literal`, `missing_const_in_map_literal`, `missing_const_in_set_literal`, `non_bool_condition`, `non_constant_list_element`, `non_constant_map_element`, `non_constant_map_key`, `non_constant_map_value`, `non_constant_record_field`, `non_constant_set_element` |

Notas à tabela:

- `analyzer/lib/src/lint/linter.dart` referencia 34 códigos `CONST_EVAL_*` e
  afins num `switch` de `_ConstantAnalysisErrorListener` (`linter.dart:368-420`)
  para decidir se uma expressão "pode ser const" nos lints; **não emite** nenhum.
- `library_analyzer.dart` aparece com 27 nomes: 13 são emissões (tabela da fase
  A e `inconsistent_language_version_override`); os outros 14 são a lista da
  porta dos avisos de import (`:594-609`).
- `error_detection_helpers.dart` é um mixin usado pelo `ResolverVisitor`
  (fase 3c) **e** pelo `ErrorVerifier` (5c); a fase de cada emissão depende de
  quem chama (ver §A do outro documento).
- `dead_code_verifier.dart` tem duas classes: `NullSafetyDeadCodeVerifier`
  (chamada pelo fluxo durante a 3c) e `DeadCodeVerifier` (7b).
- `duplicate_definition_verifier.dart` idem: `DuplicateDefinitionVerifier`
  (dentro do `ErrorVerifier`, 5c) e `MemberDuplicateDefinitionVerifier` (6a).

### 1.4 O que a ordem das fases muda no resultado

A lista final é ordenada pelo `dartdev` (§5.3), então a ordem das fases **não**
aparece na saída. Ela importa em quatro pontos:

1. **Deduplicação** (§3.5): dois relatos iguais (mesmo código, offset, length,
   mensagem) viram um. Verificadores diferentes que relatam o mesmo erro no
   mesmo nó não duplicam.
2. **Supressão cruzada**: verificadores posteriores consultam o que os
   anteriores fizeram — a porta de imports (item 2 acima), o
   `IgnoreValidator` (precisa de todos os erros já relatados, `:339-348`), o
   `hasConstError` dos lints.
3. **Estado da AST**: a 3c reescreve nós (`AstRewriter`) e grava
   `staticType`/`staticElement`; todos os verificadores de C leem esses campos.
   Um verificador de C nunca roda sobre AST não resolvida.
4. **Constantes**: `_computeConstants` (fase 4) avalia **antes** do
   `ConstantVerifier` (5a); os erros de avaliação ficam nos elementos
   (`evaluationResult`) e são relatados depois, no nó de uso (II.6, T8).

### 1.5 No DartForge

- O orquestrador é `Motor::analisar_com` (`crates/paridade/src/analise.rs:173-660`),
  usado pelo `dartforge analyze` (`crates/cli/src/analisar.rs:50-66`) e pelo LSP.
  Ele monta uma biblioteca de entrada sintética que importa todos os arquivos
  do lote (`analise.rs:183-210`), carrega o programa (`load_lenient_gerados`,
  `:217-225`) e roda: (1) sintaxe vinda da carga (`:244-270`); (2) diretivas sem
  alvo (`:272-293`, `diretiva_sem_alvo` `:662`); (3) por biblioteca, os
  verificadores sem tipos de `crates/analise` (`duplicatas`, `enums`,
  `inicializacao`, `locais`, `externos`, `nativos`, `operadores`, `privados` —
  `:328-354`; `heranca`, `modificadores`, `clausulas`, `membros` — `:362-378`);
  (4) tipos e constantes (`dartforge_types::constantes::verificar`, `:493`);
  (5) imports não usados (`importacoes::nao_usados`, `:598`).
- **Diferenças estruturais em relação ao oficial**:
  1. *Unidade de análise*: lote de arquivos de um pacote com entrada sintética,
     não biblioteca por biblioteca. Consequência: uma parte sem biblioteca
     conhecida não é analisada "como biblioteca" (`e_parte`, `analise.rs:64`).
  2. *Portas de sintaxe* que o oficial não tem: `libs_com_erro_de_sintaxe`
     (`analise.rs:300`, `:349-352`, `:525`) e `recuperacao_do_parser` (`:725`);
     ver T5 — cada porta é FN potencial.
  3. *Publicação*: só sintaxe e `verificados.txt` saem
     (`crates/analise/src/publicacao.rs:24-34`); o oficial publica tudo.
  4. *Fases ausentes por inteiro*: C4a (Unicode), C4c (`BestPracticesVerifier`
     e sub-verificadores), C4d/C4e (`OverrideVerifier`, `RedeclareVerifier`),
     C4f (`TodoFinder`), C4j (`SdkConstraintVerifier`), C5 (lints), C7
     (`IgnoreValidator`). Parciais: C4b (`dead_code` vem da inferência), C4h
     (`unused_import` existe, `crates/analise/src/importacoes.rs:165`; os
     `duplicate_*`/`unnecessary_import` não), C4i (`locais.rs`, `privados.rs`).
     O detalhe por código está na Parte II.
  5. *Deduplicação*: não há um conjunto por arquivo equivalente ao
     `RecordingErrorListener` (§3.5); a deduplicação é local a cada verificador.

## 2. Linking (`summary2`): elementos, inferência de topo, ciclos

### 2.1 Quando o link roda

Antes de `LibraryAnalyzer.analyze()`, o driver carrega a biblioteca alvo
(`libraryContext.load`, `analyzer/lib/src/dart/analysis/driver.dart:1368`):

```text
LibraryContext.load(targetLibrary)                       analyzer/lib/src/dart/analysis/library_context.dart:123
  loadBundle(cycle):                                     :135
    para cada directDependency do ciclo: loadBundle      :143-145   (dependências primeiro)
    linkedBytes = byteStore.get(cycle.linkedKey)         :183       (chave = apiSignature + ".linked",
                                                                     library_graph.dart:133)
    se não há bytes: linkResult = await link(elementFactory, inputLibraries: cycle.libraries)   :192-203
                     byteStore.putGet(cycle.linkedKey, linkResult.resolutionBytes)              :211-212
    elementFactory.addBundle(BundleReader(resolutionBytes: linkedBytes, …))                     :231-237
  libraryCycle = targetLibrary.libraryCycle              :274-277   (componente fortemente conexa do
                                                                     grafo import/export — library_graph.dart:215-231)
  loadBundle(libraryCycle)                               :282
```

A unidade de link é o **ciclo de bibliotecas** (SCC do grafo de
imports/exports, `_LibraryWalker.evaluateScc`, `library_graph.dart:227`): todas
as bibliotecas de um ciclo são ligadas juntas, e um ciclo só é ligado depois de
todas as suas dependências. O resultado é serializado (`BundleWriter`,
`summary2/link.dart:478-493`) e os elementos usados pela análise vêm da
**releitura** desses bytes (`BundleReader`), não dos objetos do link.

### 2.2 Ordem das etapas do link

`Linker.link` (`analyzer/lib/src/summary2/link.dart:105-133`) e `_buildOutlines` (`:233-301`):

```text
1  LibraryBuilder.build(inputLibrary) para cada biblioteca do ciclo        link.dart:110-120, library_builder.dart:1323
2  _buildOutlines:                                                         link.dart:233
   2.1  _createTypeSystemIfNotLinkingDartCore()                            :236, :358-362
   2.2  _computeLibraryScopes():                                           :238-245, :315-344
          library.buildElements() em cada biblioteca                       :318-320 → library_builder.dart:199
              (ElementBuilder percorre a AST de cada unidade e cria os elementos na ORDEM TEXTUAL;
               library_builder.dart:207)
          _buildMacroApplier()                                             :322-327
          _buildExportScopes()                                             :330, :160-208
              escopo inicial de cada biblioteca; propagação pelos `export` até ponto fixo
              (laço `while (true)` :191-203, com combinadores show/hide em Export.addToExportScope)
          macros, fase de tipos; _buildExportScopes() de novo              :332-343
   2.3  _createTypeSystem()                                                :247, :346-353
          (TypeProvider de dart:core/dart:async e um InheritanceManager3 novo)
   2.4  _resolveTypes():                                                   :248, :461-470
          library.resolveTypes(nodesToBuildType) — ReferenceResolver resolve os NamedType das
              assinaturas contra os escopos (library_builder.dart:720-722)
          VarianceBuilder(this).perform()                                  :466
          computeSimplyBounded(this)                                       :467  → element.isSimplyBounded
          TypeAliasSelfReferenceFinder().perform(this)                     :468  → element.hasSelfReference
          TypesBuilder(this).build(nodesToBuildType)                       :469
              (supertipos, mixins, interfaces, tipos de campos/parâmetros declarados,
               tipos de typedefs; summary2/types_builder.dart; inclui buildExtensionTypes,
               types_builder.dart:94 → summary2/extension_type.dart:17, :36-40, :87, :177 — quebra de ciclos de `implements`
               e de representação, marcando hasImplementsSelfReference / hasRepresentationSelfReference)
   2.5  _setDefaultSupertypes()                                            :249
   2.6  macros, fase de declarações                                        :251-259
   2.7  _buildClassSyntheticConstructors(); _buildEnumSyntheticConstructors()   :261-262
   2.8  _replaceConstFieldsIfNoConstConstructor()                          :263
   2.9  _resolveConstructorFieldFormals()                                  :264
   2.10 _buildEnumChildren()                                               :265
   2.11 _computeFieldPromotability()                                       :266
   2.12 SuperConstructorResolver(this).perform()                           :267
   2.13 _performTopLevelInference()                                        :268 → :423-425   (§2.3)
   2.14 _resolveConstructors()                                             :269
   2.15 _resolveConstantInitializers()                                     :270 → ConstantInitializersResolver,
                                                                           summary2/top_level_inference.dart:34-93
   2.16 _resolveDefaultValues()                                            :271
   2.17 _resolveMetadata()                                                 :272
   2.18 macros, fase de definições                                         :275-282
   2.19 _collectMixinSuperInvokedNames(); _buildElementNameUnions(); _detachNodes()   :284-286
   2.20 fusão das augmentations de macro                                   :288-300
3  _writeLibraries()  (BundleWriter)                                       :128-132, :478-493
```

### 2.3 Inferência de topo

`TopLevelInference.infer` (`analyzer/lib/src/summary2/top_level_inference.dart:100-107`):

```text
initializerInference = _InitializerInference(linker)
initializerInference.createNodes()        :102, :132-147
    para cada biblioteca, unidade: campos de classes, enums, extensions, extension types, mixins,
    depois variáveis de topo (nesta ordem, :137-144); só entram os de tipo implícito
    (`hasImplicitType`, :177) e não sintéticos (exceto campo sintético de enum, :172-175);
    cada um recebe um `typeInference` preguiçoso (:182-183)
_performOverrideInference()               :104, :109-116
    InstanceMemberInferrer(linker.inheritance).inferCompilationUnit(unit) para cada unidade
    (analyzer/lib/src/task/strong_mode.dart:34): tipo de retorno e de parâmetros herdado das
    assinaturas sobrescritas; campos e getters/setters sem tipo (:92), construtores (:374)
initializerInference.perform()            :106, :150-155
    `element.type` de cada variável da lista força a inferência pelo inicializador
```

`_PropertyInducingElementTypeInference.perform` (`:215-279`):

1. sem inicializador → `dynamic` (`:216-219`);
2. se o próprio elemento já está "sendo inferido" → **ciclo**: os elementos da
   pilha `_inferring` a partir do início do ciclo recebem, todos, o mesmo
   `TopLevelInferenceError(kind: dependencyCycle, arguments: <nomes do ciclo, ORDENADOS>)`
   e tipo `dynamic` (`:229-245`; `cycle.map((e) => e._element.name).sorted()`, `:234`);
3. senão empilha, resolve o inicializador com `AstResolver` (sem tipo de
   contexto, `:260-263`), desempilha e refina: `Null` vira `dynamic`
   (`_refineType`, `:281-287`).

Constantes: `ConstantInitializersResolver` (`:34-93`) re-resolve o
inicializador de toda variável/campo `const` **com** o tipo declarado/inferido
como contexto (`contextType: element.type`, `:87-88`), e guarda a expressão
(possivelmente reescrita) em `constantInitializer` (`:91`).

### 2.4 O link não relata nada: o que ele deixa para a análise

Todas as resoluções do link usam **`AnalysisErrorListener.NULL_LISTENER`**
(`analyzer/lib/src/summary2/ast_resolver.dart:26-27`, usado em `:35`, `:44`,
`:57`; `library_builder.dart:616`). Nenhum diagnóstico nasce no link. O que o
link descobre e a análise precisa relatar é guardado **como estado dos
elementos** (e serializado no bundle, `summary2/bundle_writer.dart:407`, `:519`,
`:672`, `:1381-1386`; `bundle_reader.dart:1144`, `:1354`, `:1635-1641`, `:1677`):

| estado no elemento | quem grava (link) | quem relata (análise) | código |
|---|---|---|---|
| `PropertyInducingElementImpl.typeInferenceError` com `kind == dependencyCycle`, `arguments` = nomes ordenados | `top_level_inference.dart:229-245` | `ResolverVisitor._checkTopLevelCycle`, `analyzer/lib/src/generated/resolver.dart:4009-4030`: só para não-`const` (`:4016-4018`), `atToken(node.name)`, argumentos `[nome, nomes.join(', ')]` | `top_level_cycle` |
| `MethodElementImpl.typeInferenceError` com `kind == overrideNoCombinedSuperSignature`, `arguments[0]` = explicação do conflito (`'Classe.nome (tipo), …'` ou `'<unknown>'`) | `InstanceMemberInferrer._inferExecutable`, `analyzer/lib/src/task/strong_mode.dart:437-452` | `_ClassVerifier._reportNoCombinedSuperSignature`, `analyzer/lib/src/error/inheritance_override.dart:953-970`: `atToken(node.name)`, `[classe, explicação]` | `no_combined_super_signature` |
| `TypeAliasElementImpl.hasSelfReference` | `TypeAliasSelfReferenceFinder`, `analyzer/lib/src/summary2/type_alias.dart:20`, `:25`, `:98` | `ErrorVerifier._checkForTypeAliasCannotReferenceItself`, `analyzer/lib/src/generated/error_verifier.dart:5354-5364`: `atToken(nameToken)` | `type_alias_cannot_reference_itself` |
| `ExtensionTypeElementImpl.hasImplementsSelfReference` | `summary2/extension_type.dart:87` | `error_verifier.dart:3442-3447`: `atToken(node.name)` | `extension_type_implements_itself` |
| `ExtensionTypeElementImpl.hasRepresentationSelfReference` | `summary2/extension_type.dart:177` | `error_verifier.dart:3494-3499`: `atToken(node.name)` | `extension_type_representation_depends_on_itself` |
| `TypeParameterizedElement.isSimplyBounded == false` | `computeSimplyBounded`, `summary2/simply_bounded.dart:49-67`, `:217-270` | `error_verifier.dart:7181-7188` (visitante dos limites de parâmetros de tipo; só para `NamedType` sem argumentos de tipo): `atNode(node)` | `not_instantiated_bound` |
| limites padrão com ciclo quebrado (`defaultType`) | `summary2/default_types_builder.dart:28-74`, `:103-137` | (sem código próprio; muda o tipo instanciado até os limites) | — |

Tudo o mais que parece "erro de link" (nome não resolvido numa assinatura,
supertipo inválido, ciclo de herança, conflito de membros herdados) é
**redescoberto na análise** sobre a AST e os elementos: `ResolutionVisitor`/
`NamedTypeResolver` re-resolvem cada `NamedType` e relatam (`undefined_class`,
`not_a_type`…), e os verificadores de herança recalculam
(`recursive_interface_inheritance` etc., §C do outro documento). Como o tipo
errado vira `InvalidType` no elemento (T4), as duas fases concordam.

Consequência para `const`: o ciclo de inicializadores `const` **não** dá
`top_level_cycle` (`resolver.dart:4014-4018`); dá `recursive_compile_time_constant`
pelo grafo de constantes (T8).

### 2.5 No DartForge

- O equivalente do link está em `crates/elements` (esboço das declarações:
  `crates/elements/src/outline.rs`; carga e resolução de diretivas:
  `crates/elements/src/load.rs`) e em `crates/types` (resolução de assinaturas:
  `crates/types/src/resolve.rs`; hierarquia: `crates/types/src/hierarchy.rs`;
  inferência de topo dentro de `crates/types/src/inferencia/`). Não há
  serialização/releitura de elementos por ciclo.
- `top_level_cycle` e `no_combined_super_signature` estão nas listas dos 436
  (§B/§C do outro documento): a regra de **quais nomes entram na mensagem e em
  que ordem** é a desta seção — nomes do ciclo ordenados lexicograficamente
  (`sorted()` do `package:collection` sobre `String`), unidos por `', '`; a
  explicação do conflito é `candidates.map('$classe.$nome ($tipo)').join(', ')`
  com `getDisplayString()` do tipo do candidato (`strong_mode.dart:441-446`), ou
  `'<unknown>'` quando o conflito não é um único `CandidatesConflict`.
- A ordem "campos de classes antes de variáveis de topo, biblioteca por
  biblioteca do ciclo" (`top_level_inference.dart:133-146`) só importa para
  **qual** elemento detecta o ciclo; como todos os membros do ciclo recebem o
  mesmo erro, o resultado não depende dela.
- Não verificado no DartForge: se a inferência de topo marca **todos** os
  elementos do ciclo (e só os do ciclo, não os que apenas dependem dele) — o
  oficial marca apenas `cycle = _inferring.slice(startIndex)` (`:230-231`).

## 3. `ErrorReporter` e `RecordingErrorListener`: posição, argumentos, deduplicação

Tudo em `analyzer/lib/error/listener.dart` (502 linhas) e
`analyzer/lib/error/error.dart` (282 linhas).

### 3.1 Um `ErrorReporter` por arquivo

`FileAnalysis` cria, por arquivo, um `RecordingErrorListener` e um
`ErrorReporter(errorListener, file.source)`
(`analyzer/lib/src/dart/analysis/file_analysis.dart:13-29`). Todo relato vai
para o arquivo **dono do reporter**, qualquer que seja o elemento ou nó
passado: `atElement` de um elemento declarado em **outro** arquivo relata os
offsets dele **neste** arquivo (o `ErrorReporter` não troca de `Source`,
`listener.dart:52`, `:167-185`). Os verificadores que relatam em elementos de
outra unidade da mesma biblioteca escolhem antes o reporter certo (ex.:
`libraryVerificationContext.files`, `library_analyzer.dart:92-99`).

### 3.2 Os pontos de entrada e o cálculo de offset/length

| método | `listener.dart` | offset | length |
|---|---|---|---|
| `atOffset(offset:, length:, errorCode:, arguments:, contextMessages:, data:)` | `:159-185` | dado | dado |
| `atNode(node, code, …)` | `:140-156` | `node.offset` | `node.length` |
| `atToken(token, code, …)` | `:208-224` | `token.offset` | `token.length` |
| `atEntity(entity, code, …)` (`SyntacticEntity`: nó ou token) | `:121-137` | `entity.offset` | `entity.length` |
| `atElement(element, code, …)` | `:101-117` | `element.nonSynthetic.nameOffset` | `element.nonSynthetic.nameLength` |
| `atConstructorDeclaration(node, code, …)` | `:72-97` | com nome: `node.returnType.offset`; sem nome: `atNode(node.returnType)` | com nome: `node.name.end - node.returnType.offset` (isto é, `C.nome` inteiro) |
| `atSourceSpan(span, code, …)` (YAML, §7) | `:189-205` | `span.start.offset` | `span.length` |
| `reportError(AnalysisError)` | `:226-228` | o do erro pronto | o do erro pronto |

Os métodos `reportErrorFor*` são apelidos obsoletos dos de cima (`:232-336`).
Todos, menos `reportError`, terminam em `atOffset`.

Regras de `offset`/`length` de um nó (`analyzer/lib/src/dart/ast/ast.dart`):

- `AstNodeImpl.offset = beginToken.offset` (`:1117-1120`);
  `length = endToken.offset + endToken.length - beginToken.offset`
  (`:1105-1109`); `end = offset + length` (`:1099`).
- **Declarações anotadas** (`AnnotatedNodeImpl`: classes, membros, variáveis de
  topo, parâmetros com metadados…): `beginToken` é o **primeiro entre o
  comentário de documentação e a primeira anotação** (`:147-163`). Logo
  `atNode(declaração)` começa no `///` ou no `@`, não na palavra-chave. Os
  verificadores que querem "a declaração sem comentário" usam
  `firstTokenAfterCommentAndMetadata` explicitamente ou relatam no token do nome.
- `CompilationUnitImpl`: `offset = 0`, `length = endToken.offset + endToken.length`
  (`:3572-3578`).
- Nó sintético (recuperação do parser) tem tokens sintéticos de `length` 0 no
  offset do token seguinte (`ast.dart:1004-1009`; detalhes na §E do outro
  documento e na Parte II, II.1).
- Offsets são em **unidades UTF-16** do conteúdo do arquivo (o analyzer escaneia
  a `String` com `scanString`, `analyzer/lib/src/dart/scanner/scanner.dart:126`).
  **BOM**: o `U+FEFF` inicial **não conta** — medido no binário 3.6.2 (sonda
  `s5/lib/c.dart`: `EF BB BF` + `int x = "a";` relata `"a"` em `offset 8`,
  `1:9`). O mecanismo (o decodificador UTF-8 do Dart descarta o BOM ao ler o
  arquivo) é **não verificado** na fonte; o scanner de bytes, que o analyzer não
  usa, também o pula (`_fe_analyzer_shared/lib/src/scanner/utf8_bytes_scanner.dart:96`, `:119-124`).

`nonSynthetic` (`analyzer/lib/src/dart/element/element.dart`):

| elemento | `nonSynthetic` | linha |
|---|---|---|
| padrão (`ElementImpl`) | `this` | `:2830` |
| construtor sintético (padrão implícito) | a classe (`enclosingElement3`) | `:1450-1452` |
| método sintético de enum (`values`/…) | o enum | `:7350-7355` |
| parâmetro do setter implícito | a variável | `:8369-8371` |
| getter/setter sintético (de campo ou variável) | a variável (`variable2`); se a variável também é sintética, o elemento envolvente | `:8906-8912`, `:8968` |
| campo/variável sintética (induzida por getter/setter) | o getter, senão o setter; campo sintético de enum → o enum | `:9082-9091` |

`nameLength` padrão é `displayName.length` (`:2818`) — o nome **sem** o `=` do
setter; para construtores é `nameEnd - nameOffset` (0 se não há `nameEnd`,
`:1437-1444`). `nameOffset` é o offset do token do nome gravado pelo
`ElementBuilder` (`:2821`).

### 3.3 `atOffset`: o caminho comum

```text
atOffset(offset, length, errorCode, arguments, contextMessages, data):   listener.dart:159
  se lockLevel != 0: retorna                 :167-169
      (lockLevel só é incrementado ao resolver referências dentro de comentários de documentação:
       analyzer/lib/src/dart/resolver/comment_reference_resolver.dart:30, :44 — nada é relatado
       de dentro de um `[ref]` de doc comment)
  _convertElements(arguments)                :171 → :338-355
  contextMessages ??= []                     :172
  contextMessages.addAll(_convertTypeNames(arguments))   :173 → :362-427
  _errorListener.onError(AnalysisError.tmp(source: _source, offset, length, errorCode,
      arguments: arguments ?? const [], contextMessages, data))            :174-184
```

`AnalysisError.tmp` (`error.dart:152-181`):

- `assert(arguments.length == errorCode.numParameters)` (`:161-168`) — só com
  asserts ligados. `numParameters` = maior `{n}` + 1 entre `problemMessage` e
  `correctionMessage` (`_fe_analyzer_shared/lib/src/base/errors.dart:83-95`).
- `problemMessage = formatList(errorCode.problemMessage, arguments)` (`:169`);
  se há `correctionMessage`, **também** é formatada com os mesmos argumentos
  (`:170-173`).
- `formatList` (`analyzer/lib/src/generated/java_core.dart:30-41`): troca cada
  `{n}` (regex `\{(\d+)\}`) por `arguments[n].toString()`; sem argumentos (ou
  lista vazia), devolve o molde intacto. Não há escape de chaves; um `{}` sem
  dígito (ex.: no texto de `unexpected_dollar_in_string`, "curly braces ({})")
  fica literal.
- `problemMessage`/`correctionMessage` do código passam antes por
  `customizedMessages`/`customizedCorrections`, que na 3.6.2 são mapas
  **vazios** (`_fe_analyzer_shared/lib/src/base/errors.dart:63-64`, `:80-81`;
  `customized_codes.dart`).

### 3.4 Formatação dos argumentos

`_convertElements` (`listener.dart:338-355`): argumento `Element` →
`element.getDisplayString()` (`ElementDisplayStringBuilder`,
`element.dart:2915-2927`: a "assinatura" do elemento — `int f(String a)`,
`class C`, …). Qualquer argumento que não seja `String`, `DartType`, `int` ou
`Uri` (depois da conversão de elementos) lança `ArgumentError` (`:347-354`).
`int` e `Uri` são formatados por `toString()` no `formatList`.

`_convertTypeNames` (`:362-427`) — só para argumentos `TypeImpl`:

```text
para cada argumento i que é TypeImpl:
  displayName = argumento.getDisplayString(preferTypeAlias: true)          :370-372
  typeGroups[displayName].add(_TypeToConvert(i, tipo, displayName))        :373-375
para cada grupo (mesmo displayName):
  se o grupo tem 1 tipo: arguments[i] = displayName                        :381-383
  senão (dois ou mais argumentos com o MESMO texto):                       :384-424
    nameToElementMap: para cada tipo do grupo, para cada elemento de allElements(tipo):
        nameToElementMap[element.name].add(element)                        :382-390
    para cada tipo do grupo:
      buffer = null
      para cada element de allElements(tipo):                              :397
        se nameToElementMap[name].length > 1:                              :399-406
            buffer += (primeiro ? 'where ' : ', ') + '$name is defined in ${element.source.fullName}'
        messages.add(DiagnosticMessageImpl(filePath: element.source.fullName,
            length: element.nameLength, message: '$name is defined in ${element.source.fullName}',
            offset: element.nameOffset, url: null))                        :408-413  ← FORA do if
      arguments[i] = buffer != null ? '$displayName ($buffer)' : displayName   :416-421
devolve messages (viram contextMessages do erro)
```

`_TypeToConvert.allElements()` (`:475-501`): os elementos de `InterfaceType`
alcançáveis pelo tipo — o próprio elemento e, recursivamente, os argumentos de
tipo; em `FunctionType`, o retorno e os tipos dos parâmetros (parâmetros de
tipo, records e aliases **não** são percorridos); só elementos com nome não
vazio.

Fatos:

1. A desambiguação só dispara quando **dois argumentos de tipo da mesma
   mensagem têm o mesmo texto** (ex.: `The argument type 'A' can't be assigned
   to the parameter type 'A'.`). Aí a mensagem vira
   `'A (where A is defined in C:\abs\a.dart)'` … `'A (where A is defined in C:\abs\b.dart)'`
   — com o **caminho absoluto** (`source.fullName`), separadores do sistema.
   Conferido no binário 3.6.2 (sonda `s6`: `import 'b.dart' as b; class A {} void f(A x) {} void g(b.A y) { f(y); }`):
   `The argument type 'A (where A is defined in E:\…\s6\lib\b.dart)' can't be assigned to the parameter type 'A (where A is defined in E:\…\s6\lib\a.dart)'. `
   (com o espaço final do `{2}` vazio), e duas `contextMessages`
   `A is defined in <caminho>` com `offset`/`length` do nome de cada classe.
2. Nesse caso o erro ganha `contextMessages` — **uma por elemento de cada tipo
   do grupo, mesmo os que não colidem** (a adição da mensagem está fora do
   `if`, `:408-413`) — que o `dart analyze` imprime (§5.4).
3. Argumentos passados já como `String` não passam por aqui: é a regra do T7
   (`getDisplayString()` sem `preferTypeAlias` × com). O T7 cobre o texto do
   tipo; esta seção cobre o sufixo `(where …)` e as mensagens de contexto,
   que **não existem no DartForge** (não há `contextMessages` em
   `dartforge_diagnostics::Diagnostic`; `crates/paridade/src/json.rs:24-37`
   não tem o campo).
4. `contextMessages` passados pelo emissor (`DiagnosticFactory`,
   `analyzer/lib/src/diagnostic/diagnostic_factory.dart`: "The first definition
   of this name.", "The declaration of 'x' is here." etc.) vêm **antes** dos de
   `_convertTypeNames` na lista (`addAll`, `:173`).

### 3.5 Deduplicação

`RecordingErrorListener` (`listener.dart:431-454`) guarda os erros num
`Set<AnalysisError>` (`{}` → `LinkedHashSet`, ordem de inserção):
`onError` faz `(_errors ??= {}).add(error)` (`:451-453`); `errors` devolve
`_errors.toList()` (`:434-439`).

Igualdade (`error.dart:231-256`): mesmo `errorCode` (**identidade** da
constante, `:239`), mesmo `offset` e `length` (`:242`), mesma `message`
(`:246`), mesma `source` (`:249`). `hashCode = offset ^ message.hashCode ^ source.hashCode`
(`:194-200`). **Não entram**: `correctionMessage`, `contextMessages`, `data`.

Consequências:

- Dois relatos idênticos (mesma constante, intervalo e texto) viram **um**; o
  primeiro inserido fica (com os `contextMessages` dele).
- Constantes diferentes com o mesmo nome emitido e a mesma mensagem **não** se
  fundem (a comparação é por identidade da constante).
- O conjunto é **por arquivo** (um listener por `FileAnalysis`).
- Os erros do scanner/parser entram no mesmo conjunto (o listener é criado
  antes do parse, `library_analyzer.dart:622-626`).

### 3.6 Ordem "final" dentro do analyzer

Não há ordenação no analyzer: a lista de uma unidade sai na **ordem de
inserção** (parser → diretivas → resolução → verificadores, §1.2), filtrada
pelos ignores. O servidor a envia nessa ordem em `analysis.errors`
(`analysis_server/lib/src/protocol_server.dart:98-122`); quem ordena é o
`dartdev` (§5.3). O `Comparator` de `AnalysisError` por severidade/tipo
(`error.dart:64-78`) está marcado `@Deprecated('Not used')`.

### 3.7 No DartForge

- `Diagnostic` (`crates/diagnostics/src/lib.rs`) tem `code`, `severity`,
  `span` (bytes UTF-8), `message` e argumentos; a mensagem é renderizada do
  molde pela mesma regra do `formatList` (`crates/diagnostics/src/lib.rs:5-8`).
  A conversão byte→UTF-16/linha/coluna é de `Linhas`
  (`crates/paridade/src/json.rs:60-117`).
- **Faltam** (cada item é uma etapa da Parte III):
  1. `contextMessages` (campo, produção pelos emissores que usam
     `DiagnosticFactory`, e pelo `_convertTypeNames`);
  2. o sufixo `(where X is defined in <caminho>)` para argumentos de tipo com o
     mesmo texto;
  3. um conjunto de deduplicação por arquivo com a chave
     (constante, offset, length, mensagem) aplicado a **todos** os emissores,
     inclusive entre sintaxe e semântica;
  4. a correção formatada com os argumentos (`d.correcao()` existe —
     `crates/diagnostics/src/lib.rs:324-327`, formata o molde da correção com os
     mesmos argumentos — já equivalente);
  5. `atElement` com `nonSynthetic` (tabela da §3.2) como função única no
     modelo de elementos, em vez de cada verificador escolher o intervalo.

## 4. Supressões: `// ignore:`, `IgnoreValidator`, `analysis_options.yaml`

Há **três camadas**, aplicadas nesta ordem, em lugares diferentes:

| # | camada | onde | efeito |
|---|---|---|---|
| 1 | arquivo fora da análise (`exclude:`, pastas `.x`, fora da raiz) | servidor/`ContextRootImpl` (§4.5) | nenhum `analysis.errors` para o arquivo |
| 2 | comentários `// ignore:` / `// ignore_for_file:` | analyzer, `LibraryAnalyzer._filterIgnoredErrors` (§4.1) | remove o erro da lista da unidade |
| 3 | `analyzer: errors:` (`ErrorProcessor`) | servidor, ao converter para o protocolo (§4.4) | remove (`ignore`) ou troca a severidade |

As opções de `language:` e `enable-experiment` não suprimem: mudam o que os
verificadores **emitem** (§4.6, §6).

### 4.1 `IgnoreInfo`: que comentários contam e a que linha se aplicam

`analyzer/lib/src/ignore_comments/ignore_info.dart`.

**Quais comentários** (`CompilationUnitExtension.ignoreComments`, `:332-359`):
percorre **todos os tokens** da unidade (do `beginToken` ao EOF inclusive,
`:350-355`) e, em cada um, a lista `precedingComments`; um comentário entra se
o lexema **começa** (`startsWith`) com uma das duas expressões:

- `ignoreMatcher = //+[ ]*ignore:` (`:81`) — duas **ou mais** barras (`///ignore:`
  conta), zero ou mais **espaços** (U+0020; tabulação não), `ignore:`;
- `ignoreForFileMatcher = //[ ]*ignore_for_file:` (`:86`) — **exatamente** duas
  barras no começo (`/// ignore_for_file:` não casa, porque depois de `//` vem
  `/`, que não é espaço).

Comentários de bloco (`/* ignore: x */`) nunca entram (não começam com `//`).
Como só o **início** do lexema é testado, `// texto ignore: x` não conta.

**A que se aplica** (`IgnoreInfo.forDart`, `:116-137`), para cada comentário:

```text
lexeme = comment.lexeme
se lexeme.contains('ignore:'):                       :120
    location    = lineInfo.getLocation(comment.offset)                     :121
    lineNumber  = location.lineNumber
    offsetOfLine= lineInfo.getOffsetOfLine(lineNumber - 1)                 :123
    beforeMatch = content.substring(offsetOfLine, offsetOfLine + location.columnNumber - 1)   :124-125
    se beforeMatch.trim().isEmpty: lineNumber++      :126-129   (comentário sozinho na linha → vale para a PRÓXIMA linha)
    _ignoredOnLine[lineNumber] += comment.ignoredElements                  :130-132
senão se lexeme.contains('ignore_for_file:'):        :133
    _ignoredForFile += comment.ignoredElements       :134
```

Fatos que saem daí:

1. A ordem dos testes é `ignore:` **antes** de `ignore_for_file:`. Um
   `// ignore_for_file: a` não contém a substring `ignore:` (tem `ignore_`),
   então cai no segundo ramo. Mas `// ignore_for_file: a // ignore: b` (ou
   qualquer `ignore_for_file` cujo texto contenha `ignore:` adiante) cai no
   **primeiro** ramo e vira ignore **de linha**.
2. "Sozinho na linha" = só espaço em branco (`trim()`) entre o início da linha
   e o comentário. Então o comentário vale para a linha **seguinte**, qualquer
   que seja o conteúdo dela (inclusive linha em branco ou outro comentário: não
   há "pular linhas"). Depois de código, vale para a **própria** linha.
3. A linha de um erro é a do **offset inicial** do erro
   (`ignored`, `:181-184`: `lineInfo.getLocation(error.offset).lineNumber`); um
   erro de várias linhas só é calado por um ignore que alcance a primeira.
4. `ignore_for_file` vale em qualquer lugar do arquivo (não precisa estar no topo).
5. Um arquivo `part` tem o seu próprio `IgnoreInfo` (por `FileAnalysis`).

Conferido no binário 3.6.2 (sonda `s5/lib/b.dart`, `unused_local_variable` em
`var x = 1;` na linha seguinte a cada comentário):

| comentário | resultado | regra |
|---|---|---|
| `// ignore: type=warning` | calado | `type=warning` ↔ `STATIC_WARNING` |
| `// ignore: type=static_warning` | relatado | tipo desconhecido não casa |
| `///ignore: unused_local_variable` | calado | `//+` aceita três barras |
| `//<TAB>ignore: unused_local_variable` | relatado | só espaços entre `//` e `ignore:` |
| `// ignore_for_file: dead_code // ignore: unused_local_variable` | relatado; e o `dead_code` de outra linha do arquivo **também** é relatado | o lexema contém `ignore:` → vira ignore **de linha** só de `dead_code` (o primeiro `:` é o de `ignore_for_file:`; depois de `dead_code` vem `/`, texto livre) |
| `var f = 1; /* ignore: unused_local_variable */` | relatado | comentário de bloco não conta |
| `var s = 'http://x'; // ignore: unused_local_variable` | calado | por tokens: o `//` dentro da string não é comentário |
| `// ignore: unused_local_variable porque sim` + linha em branco + `var h = 1;` | relatado | vale só para a linha imediatamente seguinte |
| `// ignore: unused_local_variable.` | relatado | nome seguido de `.` encerra a lista sem incluí-lo |
| `// ignore: UNUSED_LOCAL_VARIABLE , dead_code` | calado | maiúsculas e espaço antes da vírgula aceitos |

**Os nomes dentro do comentário** (`CommentTokenExtension.ignoredElements`, `:202-330`):

```text
offset = lexeme.indexOf(':') + 1                     :209   (o PRIMEIRO ':' do lexema)
hasIgnoredElements = false
laço:
  pula espaço em branco; fim → retorna               :239-243
  lê uma "palavra": começa com letra, segue com letra/dígito/_   :220-232, :246
  se não leu nada (caractere que não inicia palavra):            :247-256
      se já houve elemento: IgnoredDiagnosticComment(resto)      (texto livre; não casa com nada, :13-22)
      retorna
  palavra = …
  se palavra.toLowerCase() == 'type':                :258
      espera '=' (com espaços opcionais), depois outra palavra   :260-268
      (sem '=' → retorna sem nada; sem palavra → comentário/retorna)
      se depois da palavra vem algo que não é espaço nem vírgula → comentário, retorna   :279-290
      IgnoredDiagnosticType(tipo, offset, length)    :291-294
  senão:
      se depois da palavra vem algo que não é espaço nem vírgula
          (ex.: `ignore: http://x`) → comentário, retorna        :296-307
      IgnoredDiagnosticName(palavra, offset)         :308-309   (nome guardado em minúsculas, :31)
  fim → retorna; pula espaços; fim → retorna         :312-314
  se o próximo caractere não é vírgula: o resto é texto livre → IgnoredDiagnosticComment, retorna   :316-325
  consome a vírgula; fim → retorna                   :326-327
```

Logo: lista separada por **vírgulas**; depois do último nome pode vir texto
livre separado por espaço (`// ignore: a, b porque sim`); um nome seguido de
caractere estranho encerra a lista **sem** incluir esse nome.

**O que cada elemento casa**:

- `IgnoredDiagnosticName.matches(code)` (`:33-44`): `nome == code.name.toLowerCase()`
  **ou** `nome ==` a parte de `code.uniqueName` depois do primeiro `.`, em
  minúsculas. Ex.: `// ignore: abstract_field_constructor_initializer` cala a
  constante `ABSTRACT_FIELD_CONSTRUCTOR_INITIALIZER` (nome emitido
  `abstract_field_initializer`), e `// ignore: abstract_field_initializer` cala
  as duas constantes desse nome.
- `IgnoredDiagnosticType.matches(code)` (`:58-67`): só três tipos —
  `type=hint` ↔ `ErrorType.HINT`, `type=lint` ↔ `ErrorType.LINT`,
  `type=warning` ↔ `ErrorType.STATIC_WARNING`. Qualquer outro
  (`type=error`, `type=todo`, `type=compile_time_error`) **não casa com nada**.

**O filtro** (`LibraryAnalyzer._filterIgnoredErrors`, `library_analyzer.dart:540-567`):

```text
se errors vazio ou !ignoreInfo.hasIgnores: devolve errors            :544-551
isIgnored(error):
  code = error.errorCode
  se unignorableNames contém code.name, code.uniqueName ou code.name.toUpperCase(): false   :556-563
  senão ignoreInfo.ignored(error)                    :564
devolve os não ignorados
```

- **Não há consulta a `ErrorCode.isIgnorable`** (definido como
  `errorSeverity != ERROR`, `_fe_analyzer_shared/lib/src/base/errors.dart:73`;
  nenhum uso em `analyzer/lib` nem em `analysis_server/lib`). Um `// ignore:`
  cala **erros de compilação e de sintaxe** tanto quanto avisos. O DartForge já
  registra isso (`crates/paridade/src/filtros.rs:197-204`, com teste em `:230-249`).
- `ignoredAt` (`:187-199`): primeiro os de arquivo, depois os da linha.
- O filtro vale também para os diagnósticos do `IgnoreValidator` e dos lints
  (já estão na lista quando ele roda).

### 4.2 `IgnoreValidator`

`analyzer/lib/src/error/ignore_validator.dart`, chamado por unidade depois de
todos os outros verificadores (`library_analyzer.dart:341-348`).

```text
reportErrors():                                       :38
  se !ignoreInfo.hasIgnores: retorna                  :39-41
  ignoredOnLineMap = cópia de _ignoredOnLine; ignoredForFile = cópia   :42-43
  // 1. duplicados em ignore_for_file
  para cada elemento de ignoredForFile, na ordem:     :51-64
     Name:  se unignorableNames.contains(nome): vai para `unignorable`  (não relata — ver abaixo)
            senão se nome já visto: duplicated
     Type:  se tipo já visto: duplicated
  _reportUnignorableAndDuplicateIgnores(unignorable, duplicated, ignoredForFile)   :65-66
  // 2. duplicados por linha
  para cada lista de linha:                           :67-91
     Name:  unignorable → idem; senão se o nome está nos de arquivo OU já visto nesta linha: duplicated
     Type:  se o tipo está nos de arquivo OU já visto nesta linha: duplicated
     _reportUnignorableAndDuplicateIgnores(…)
  // 3. remove dos conjuntos os nomes que de fato calaram algo
  para cada erro relatado: remove dos de arquivo e dos da linha do erro os Name
     com nome == code.name.toLowerCase() ou == uniqueName sem a classe   :95-104, :186-204
  // 4. "desnecessários": corpo inteiro comentado     :108-111, :149-183
```

`_reportUnignorableAndDuplicateIgnores` (`:116-146`):

- o laço dos **não ignoráveis está comentado** (`:118-125`): `unignorable_ignore`
  **nunca é emitido** na 3.6.2;
- para cada duplicado: `duplicate_ignore`, argumento = o nome (minúsculo) ou o
  tipo; posição `atOffset(offset: elemento.offset, length: nome.length)` para
  nomes (`:127-135`) e `length: elemento.length` para tipos (`:136-143`) — o
  offset é o do nome dentro do comentário (`comment.offset + posição no lexema`,
  `ignore_info.dart:309`), e para tipo cobre de `type` até o fim da palavra do
  tipo (`ignore_info.dart:293-294`).

`_reportUnnecessaryOrRemovedOrDeprecatedIgnores` (`:149-183`) tem o corpo
**todo comentado**: `unnecessary_ignore`, `removed_lint_use` e
`replaced_lint_use` **não são emitidos** na 3.6.2 (as constantes existem na
tabela; ver Parte II, II.4).

Resumo: o único código do `IgnoreValidator` na 3.6.2 é **`duplicate_ignore`**
(aviso), que é ele próprio filtrável por `// ignore: duplicate_ignore`.

### 4.3 Códigos não ignoráveis

Por padrão **nenhum** (`AnalysisOptionsImpl.unignorableNames = {}`,
`analyzer/lib/src/generated/engine.dart:244`). Só `analyzer: cannot-ignore:`
preenche o conjunto (`applyUnignorables`,
`analyzer/lib/src/analysis_options/apply_options.dart:103-135`):

```text
se cannot-ignore não é lista: nada                    :104-106
stringValues = itens String da lista (conjunto)       :108
para cada severidade em ['error','info','warning'] (AnalyzerOptions.severities, task/options.dart:268):
  se stringValues contém a severidade:                :110
     remove-a; para cada código e em errorCodeValues: :113-116
        se errorProcessors tem processador para e.name e
           o PRIMEIRO deles tem severity.displayName == severidade: names.add(e.name); continue   :119-125
        se e.errorSeverity.displayName == severidade: names.add(e.name)                           :127-129
names.addAll(restantes.map(toUpperCase))              :133
unignorableNames = names
```

Fatos: (a) os nomes ficam em **MAIÚSCULAS** (`e.name` das constantes é
maiúsculo; os avulsos passam por `toUpperCase`), e o filtro compara `code.name`,
`code.uniqueName` e `code.name.toUpperCase()` (esta última para lints, cujo
`name` é minúsculo); (b) a severidade olha `errors:` **já aplicado**
(`errorProcessors` é atribuído em `:187`, antes de `:218`); (c) um código
cuja severidade foi **trocada** por `errors:` entra na lista da severidade nova
(primeiro `if`, `:119-125`) **e também** na da severidade padrão (o segundo
`if`, `:127-129`, roda sempre que o primeiro não deu `continue`): um erro
rebaixado para `warning` continua não ignorável com `cannot-ignore: [error]`,
e passa a sê-lo também com `cannot-ignore: [warning]`. **Conferido no binário
3.6.2** (sonda `s5/lib/a.dart`: `errors: invalid_assignment: warning` +
`cannot-ignore: [error]` + `// ignore_for_file: invalid_assignment` → o
diagnóstico **sai**, como `WARNING|COMPILE_TIME_ERROR|INVALID_ASSIGNMENT`). O
teste do DartForge `crates/paridade/src/filtros.rs:247-248`
(`rebaixado.ignoravel(&d)`) afirma o contrário: é um defeito; (d) lints não
entram por severidade (só `errorCodeValues` é percorrido), só por nome.

**Estado em 2026-10-05 (escrito, não compilado).** `Opcoes::ignoravel` (`crates/paridade/src/filtros.rs`)
segue o fato (c): o código não ignorável pela severidade padrão continua não ignorável quando
`errors:` o rebaixa, e passa a sê-lo também pela severidade nova; o teste que afirmava o contrário
foi corrigido. `Opcoes::lint_ignoravel` é o fato (d), para os lints: só o nome (ou o nome único),
sem caixa; `diagnosticos_json` (`crates/paridade/src/lib.rs`) não cala o lint da lista com
`// ignore:`.

### 4.4 `analyzer: errors:` — o `ErrorProcessor`

Leitura: `ErrorConfig` (`analyzer/lib/source/error_processor.dart:20-57`), a
partir de `analyzer: errors:` (`apply_options.dart:186-187`):

```text
para cada par (chave escalar, valor escalar) do mapa:               :46-54
  code   = toUpperCase(chave)                                        :34
  action = toLowerCase(valor)                                        :35
  se action ∈ {'ignore','false'} (AnalyzerOptions.ignoreSynonyms, task/options.dart:265):
        ErrorProcessor.ignore(code)        (severity = null)         :36-37
  senão se action ∈ {'error','info','warning'}: ErrorProcessor(code, severidade)   :39-42
  senão: nenhum processador (o validador de opções relata, II.10)
```

Aplicação — **no servidor**, não no analyzer: `mapEngineErrors`
(`analysis_server/lib/src/protocol_server.dart:98-122`):

```text
analysisOptions = contexto.getAnalysisOptionsForFile(result.file)    :105-106
para cada erro (na ordem da lista do analyzer):
  processor = ErrorProcessor.getProcessor(analysisOptions, error)    :109
      → o PRIMEIRO processador com code == error.errorCode.name
        ou == error.errorCode.name.toUpperCase()                     error_processor.dart:85-87, :102-106
  se processor != null:
      se processor.severity != null: emite com a severidade trocada  :111-116
      senão: descarta (filtrado)                                     :112-113
  senão: emite com a severidade padrão                               :118
```

Fatos:

1. O casamento é só pelo **nome emitido** (`ErrorCode.name`), nunca pelo
   `uniqueName`: `errors: abstract_field_constructor_initializer: ignore` não
   casa com nada (e o validador relata `unrecognized_error_code`, II.10).
2. Vale para **qualquer** severidade e tipo: dá para rebaixar erro de
   compilação a `info`, promover `todo` a `warning`, ignorar erro de sintaxe.
3. Só a **severidade** muda; o `type` (`ErrorType`) continua o do código
   (`protocol_server.dart:155-156`). Um `todo: warning` sai com
   `severity: WARNING`, `type: TODO` — e por isso passa no filtro de TODOs do
   `dartdev` (§5.2).
4. Ordem relativa aos ignores: os `// ignore:` já foram aplicados no analyzer;
   o `ErrorProcessor` vem depois. O resultado é o mesmo em qualquer ordem,
   exceto pelo `cannot-ignore` com severidade (§4.3), que consulta os
   processadores.
5. Para arquivos não-Dart o mesmo processamento é feito por
   `AnalyzerConverter.convertAnalysisErrors(errors, lineInfo:, options:)`
   (`analyzer_plugin/lib/utilities/analyzer_converter.dart:66-86`, tag 3.6.2),
   chamado de `analysis_server/lib/src/context_manager.dart:386-388`, `:409-411`,
   `:448-451`, `:477-480`.
6. As opções são **por arquivo**: `AnalysisOptionsMap.getOptions(file)` devolve
   as do `analysis_options.yaml` da pasta mais profunda que contém o arquivo
   (`analyzer/lib/src/dart/analysis/analysis_options_map.dart:41-63`: entradas
   ordenadas por caminho decrescente, primeira que contém); sem nenhuma, as
   opções padrão (`:13`, `:62`).

### 4.5 `exclude:`, pastas ocultas e a raiz de contexto

Quais arquivos são "analisados" (`ContextRootImpl`,
`analyzer/lib/src/dart/analysis/context_root.dart`):

```text
analyzedFiles():                                      :63-76
  para cada includedPath: se é arquivo → ele; se é pasta → _includedFilesInFolder   :98-132
      (recursivo; segue links simbólicos sem repetir o caminho canônico, :116-125)
isAnalyzed(path):                                     :79-94
  included é arquivo: path == included.path           (um arquivo pedido explicitamente é sempre analisado)
  included é pasta:   pasta contém path && !_isExcluded(path, pasta)
_isExcluded(path, includedPath):                      :139-173
  1. algum componente do caminho, do arquivo até a raiz do contexto (exclusive o que está
     fora da raiz), começa com '.'  → excluído        :142-147   (.dart_tool, .git, .foo.dart…)
  2. path == excludedPath ou está dentro de um excludedPath   :149-164   (pastas que viraram
     outra raiz de contexto, e os `excluded` do pedido)
  3. algum glob de `exclude:` casa com path E NÃO casa com o includedPath   :166-170
```

Globs (`ContextLocatorImpl._getExcludedGlobs`,
`analyzer/lib/src/dart/analysis/context_locator.dart:561-600`):

- lidos de `analyzer: exclude:` (só itens `String` de uma **lista** YAML) das
  opções **já com os `include:` fundidos** (`getOptionsFromFile`, `:565-567`);
- cada padrão é um `Glob(pattern, context: pathContext)` do `package:glob`,
  **relativo à pasta do arquivo de opções** (`LocatedGlob(optionsFile.parent, …)`;
  `LocatedGlob.matches` usa o caminho relativo a essa pasta,
  `context_root.dart:177-190`);
- um padrão terminado em `/**` gera **dois** globs: o original e o mesmo sem o
  `**` final (`:588-590`) — para excluir a própria pasta, o que impede a
  descida e a criação de raízes de contexto dentro dela
  (`_createContextRootsIn`, `:411-423`);
- `analysis_options.yaml` de **subpastas** também contribuem: cada um é
  registrado em `optionsFileMap[pasta]` e os seus `exclude:` são somados aos da
  raiz que o contém (`:374-380`).

Raízes de contexto (`_createContextRoots`, `context_locator.dart:302-393`): uma
subpasta vira **outra raiz** (outro driver, outro `package_config`) quando tem
`.dart_tool/package_config.json` próprio, um `BUILD.gn`, ou um
`analysis_options.yaml` com conjunto de plugins legados diferente (`:330-336`).
A nova raiz herda o `analysis_options.yaml` mais próximo entre ela e a raiz que
a contém (`:354-364`) e a pasta é **excluída** da raiz de fora (`:366`).
Pastas cujo nome começa com `.` não são visitadas (`:412-413`).

### 4.6 `include:`, `language:`, `enable-experiment` e o resto das opções

**Carga** (`ContextBuilderImpl`, `analyzer/lib/src/dart/analysis/context_builder.dart`):
`AnalysisOptionsImpl(file: optionsFile)` + `applyOptions(provider.getOptionsFromFile(optionsFile))`
(`:303-309`, e por pasta em `:204-209`); exceções são engolidas (`:310-312`):
um YAML inválido deixa as opções **padrão** (e o erro de sintaxe do YAML sai
pelo validador, II.10). Sem arquivo de opções: `AnalysisOptionsImpl()` puro —
`lint = false`, `warning = true`, sem processadores, sem estritos
(`engine.dart:207-234`).

**`include:`** (`AnalysisOptionsProvider.getOptionsFromSource`,
`analyzer/lib/src/analysis_options/analysis_options_provider.dart:66-84`):

```text
options = YAML do arquivo (mapa; não-mapa → mapa vazio; erro de YAML → OptionsFormatException
          → resultado é mapa VAZIO para o arquivo todo, :81-83)
node = options['include']
se há sourceFactory e node é escalar String:
    parent = sourceFactory.resolveUri(source, path)     (relativo ao arquivo, ou package:)   :74
    se parent != null: options = merge(getOptionsFromSource(parent), options)                :75-77
```

- Um único `include` (escalar) por arquivo na 3.6.2; listas de `include` não
  são seguidas aqui (`node is YamlScalar`, `:71`).
- Recursivo, sem detecção de ciclo nesta função (o ciclo é diagnosticado pelo
  **validador**, `recursive_include_file`, II.10; a carga em ciclo: **não
  verificado** como termina).
- `merge` (`Merger`, `analyzer/lib/src/util/yaml.dart:23-100`): mapas fundem-se
  recursivamente (o incluidor sobrescreve); listas concatenam sem repetir
  (`mergeList`, `:66-75`); lista de strings × mapa de booleanos: a lista é
  promovida a mapa `{item: true}` (`:35-50`) — é assim que `linter: rules:` em
  forma de lista e de mapa se combinam; escalar do incluidor sobrescreve, salvo
  se for `null` (`:58-62`).

**`applyOptions`** (`apply_options.dart:179-237`), na ordem do código:

| chave | linhas | efeito |
|---|---|---|
| `analyzer: errors:` | `:186-187` | `errorProcessors` (§4.4) |
| `analyzer: enable-experiment:` (lista) | `:190-203` | `contextFeatures = FeatureSet.fromEnableFlags2(sdkLanguageVersion: currentVersion, flags)` (§6) |
| `analyzer: optional-checks:` | `:206-207`, `:78-101` | `chrome-os-manifest-checks` (liga o validador do `AndroidManifest.xml`, §7), `propagate-linter-exceptions` |
| `analyzer: language:` | `:210-211`, `:53-76` | `strict-casts`, `strict-inference`, `strict-raw-types` (só valores booleanos) |
| `analyzer: exclude:` | `:214-215`, `:44-51` | `excludePatterns` (os globs efetivos vêm do `ContextLocator`, §4.5) |
| `analyzer: cannot-ignore:` | `:217-218` | §4.3 |
| `analyzer: plugins:` | `:221-222`, `:151-174` | só o **primeiro** plugin legado |
| `code-style: format:` | `:226-227`, `:137-149` | `codeStyleOptions.useFormatter` |
| `linter: rules:` | `:229-236` | `lint = true` e `lintRules` **se** alguma regra registrada foi ligada (§8) |

`analyzer: strong-mode:` (`implicit-casts`, `implicit-dynamic`,
`declaration-casts`) é reconhecido só pelo **validador** (códigos de opção
removida/obsoleta, II.10); `applyOptions` não lê.

**O que cada opção de `language:` muda nos códigos** (lido nos usos):

- **`strict-casts: true`** — `TypeSystemImpl.isAssignableTo(from, to, strictCasts: true)`
  deixa de aceitar `dynamic` → qualquer tipo
  (`analyzer/lib/src/dart/element/type_system.dart:877-908`: subtipo → sim;
  `from` é `InvalidType` → sim; tear-off de `call` → recursão; **`strictCasts` →
  não**; senão `from is DynamicType`). Passa a sair, em todo ponto que usa
  `isAssignableTo` com a opção: `argument_type_not_assignable`,
  `invalid_assignment`, `return_of_invalid_type*`, `non_bool_condition`,
  `list_element_type_not_assignable` etc. com `'dynamic'` como tipo de origem.
  Pontos que leem a opção: `assignment_expression_resolver.dart:127`,
  `extension_member_resolver.dart:233`, `invocation_inference_helper.dart:138`,
  `invocation_inferrer.dart:234`, `postfix_expression_resolver.dart:81`,
  `prefix_expression_resolver.dart:104`, `typed_literal_resolver.dart:448/508/613`,
  `yield_statement_resolver.dart:94/111/135`, `bool_expression_verifier.dart:58`,
  `best_practices_verifier.dart:109/112`, `ffi_verifier` (via
  `library_analyzer.dart:456`), `TypeSystemOperations` (`driver.dart:1389-1391`),
  `ResolutionVisitor` (`library_analyzer.dart:826`).
- **`strict-inference: true`** — liga os `inference_failure_on_*`:
  `inference_failure_on_instance_creation`, `…_function_invocation`,
  `…_generic_invocation` (`generic_inferrer.dart:333`, emissões em `:818-867`);
  `…_collection_literal` (`typed_literal_resolver.dart:471-479`, `:712-720`);
  `…_uninitialized_variable` (`variable_declaration_resolver.dart:31-34`);
  `…_untyped_parameter` e `…_function_return_type`
  (`best_practices_verifier.dart:1388-1431`, portas em `:1394`, `:1411`). Sem a
  opção, nenhum desses sai.
- **`strict-raw-types: true`** — liga `strict_raw_type`
  (`type_arguments_verifier.dart:243`, emissão em `:260`).

### 4.7 No DartForge

O que existe (`crates/paridade/src/filtros.rs`, usado por
`crates/paridade/src/lib.rs:58-75` e `crates/cli/src/analisar.rs:39-47`):

| item | DartForge | divergência da fonte |
|---|---|---|
| leitura do `analysis_options.yaml` | `Opcoes::ler` (`filtros.rs:33-38`): só o da **raiz do pacote**, por um leitor de YAML por indentação (`de_texto`, `:40-90`) | sem YAML real (âncoras, fluxo `{}`, escalares multilinha, chaves com aspas e `#` dentro de string); sem `include:` (`:12-13`); sem opções por subpasta (§4.4, item 6); sem `language:`, `enable-experiment`, `optional-checks`, `linter` |
| `errors:` | `Opcoes::processar` (`:122-132`) | casa por nome minúsculo — equivalente; sinônimo `false` de `ignore` não é lido (`:72-78` só aceita `ignore`) |
| `exclude:` | `Opcoes::excluido` + `glob` (`:117-119`, `:142-158`) | glob próprio: sem `{a,b}`, `[abc]`, escapes; relativo à raiz, não à pasta do arquivo de opções; sem a regra "`x/**` também exclui `x`"; sem a exceção do arquivo pedido explicitamente; a varredura (`corpus::arquivos_dart`, `crates/paridade/src/corpus.rs:60-80`) pula pastas começadas por `.` (igual ao oficial) **e a pasta `build`** (o oficial não pula `build`; só os globs de `exclude:` a tiram), e não pula **arquivos** começados por `.` (o oficial pula, `context_root.dart:142-147`) |
| `cannot-ignore:` | `Opcoes::ignoravel` (`:95-114`) | diverge no caso "rebaixado por `errors:`" (§4.3) |
| `// ignore:` | `Ignorados::de_texto` (`:168-195`): por linha de texto, primeiro `//` da linha | não usa tokens: um `//` dentro de string (`'http://x' // ignore: y`) ou de comentário de bloco engana; `///ignore:` e a regra de espaços-só; `ignore_for_file` com três barras é aceito aqui e não no oficial; o analisador de nomes é `split(',')` + primeira palavra (`:181-185`), sem a regra "caractere estranho encerra sem incluir", sem `type = lint` com espaços; não casa pelo `uniqueName` |
| `type=` | `ignora` (`:204-210`): `type=<tipo em minúsculas>` | o oficial só aceita `hint`, `lint`, `warning` (este último ↔ `STATIC_WARNING`); aqui `type=static_warning` casaria e `type=warning` não — **invertido** |
| `IgnoreValidator` | não existe | falta `duplicate_ignore` |

Tudo isso é a etapa 2 da Parte III, com os casos de teste.

## 5. Severidade, tipo e a saída do `dart analyze`

### 5.1 `ErrorSeverity`, `ErrorType` e o que cada classe de código declara

`_fe_analyzer_shared/lib/src/base/errors.dart`:

| `ErrorSeverity` | `ordinal` | `machineCode` | `displayName` | linha |
|---|---:|---|---|---|
| `NONE` | 0 | `" "` | `none` | `:125` |
| `INFO` | 1 | `I` | `info` | `:130` |
| `WARNING` | 2 | `W` | `warning` | `:136-137` |
| `ERROR` | 3 | `E` | `error` | `:142-143` |

| `ErrorType` | `ordinal` | severidade do tipo | linha |
|---|---:|---|---|
| `TODO` | 0 | INFO | `:196` |
| `HINT` | 1 | INFO | `:202` |
| `COMPILE_TIME_ERROR` | 2 | ERROR | `:209-210` |
| `CHECKED_MODE_COMPILE_TIME_ERROR` | 3 | ERROR | `:216-217` |
| `STATIC_WARNING` | 4 | WARNING | `:224-225` |
| `SYNTACTIC_ERROR` | 6 | ERROR | `:231-232` |
| `LINT` | 7 | INFO | `:238` |

(não existe ordinal 5.) A severidade e o tipo de um diagnóstico vêm da
**classe** do código, não da constante:

| classe | `errorSeverity` | `type` | onde |
|---|---|---|---|
| `CompileTimeErrorCode` | ERROR | `COMPILE_TIME_ERROR` | `analyzer/lib/src/error/codes.g.dart:6041-6044` |
| `StaticWarningCode` | WARNING | `STATIC_WARNING` | `codes.g.dart:6139-6142` |
| `WarningCode` | WARNING | `STATIC_WARNING` | `codes.g.dart:7704-7707` |
| `HintCode` | INFO | `HINT` | `analyzer/lib/src/dart/error/hint_codes.g.dart:133-136` |
| `FfiCode` | ERROR | `COMPILE_TIME_ERROR` | `analyzer/lib/src/dart/error/ffi_code.g.dart:522-525` |
| `ParserErrorCode` | ERROR | `SYNTACTIC_ERROR` | `analyzer/lib/src/dart/error/syntactic_errors.g.dart:2081-2084` |
| `ScannerErrorCode` | ERROR | `SYNTACTIC_ERROR` | `_fe_analyzer_shared/lib/src/scanner/errors.dart:205-208` |
| `TodoCode` | INFO | `TODO` | `analyzer/lib/src/dart/error/todo_codes.dart:92-95` |
| `LintCode` (e `SecurityLintCode`) | INFO | `LINT` | `analyzer/lib/src/dart/error/lint_codes.dart:30-36`, `:50` |
| `AnalysisOptionsErrorCode` | ERROR | `COMPILE_TIME_ERROR` | `analyzer/lib/src/analysis_options/error/option_codes.g.dart:62-65` |
| `AnalysisOptionsWarningCode` | WARNING | `STATIC_WARNING` | `option_codes.g.dart:347-350` |
| `AnalysisOptionsHintCode` | INFO | `HINT` | `option_codes.g.dart:117-120` |
| `PubspecWarningCode` | WARNING | `STATIC_WARNING` | `analyzer/lib/src/pubspec/pubspec_warning_code.g.dart:265-268` |
| `ManifestWarningCode` | WARNING | `STATIC_WARNING` | `analyzer/lib/src/manifest/manifest_warning_code.g.dart:128-131` |

Na 3.6.2 só **8 constantes** são `HintCode` (INFO)
(`analyzer/lib/src/error/error_code_values.g.dart:644-651`):
`DEPRECATED_COLON_FOR_DEFAULT_VALUE`, `DEPRECATED_MEMBER_USE`,
`DEPRECATED_MEMBER_USE_WITH_MESSAGE` (nome emitido `deprecated_member_use`),
`DEPRECATED_MEMBER_USE_FROM_SAME_PACKAGE`,
`DEPRECATED_MEMBER_USE_FROM_SAME_PACKAGE_WITH_MESSAGE` (nome emitido
`deprecated_member_use_from_same_package`),
`IMPORT_DEFERRED_LIBRARY_WITH_LOAD_FUNCTION`, `MACRO_INFO` e
`UNNECESSARY_IMPORT`. Todo o resto do que antes era "hint" é `WarningCode`
(WARNING): `unused_import`, `dead_code`, `unused_local_variable` etc. saem
como `warning`, e fazem o `dart analyze` devolver 2. `ErrorCode.url` = `https://dart.dev/diagnostics/<name minúsculo>`
quando `hasPublishedDocs` (`errors.dart:106-111`).

### 5.2 Do `AnalysisError` do analyzer ao `AnalysisError` do protocolo

`newAnalysisError_fromEngine` (`analysis_server/lib/src/protocol_server.dart:127-176`):

```text
location: file = error.source.fullName (caminho absoluto, separador do sistema)   :134
          offset, length = do erro                                                :135-136
          startLine/startColumn = result.lineInfo.getLocation(offset)             :139-141   (1-based)
          endLine/endColumn     = result.lineInfo.getLocation(offset + length)    :143-145
severity = (severidade do ErrorProcessor) ?? errorCode.errorSeverity  → .name     :152-155   ("ERROR"/"WARNING"/"INFO")
type     = errorCode.type.name                                                    :156
message  = error.message                                                          :157
code     = errorCode.name.toLowerCase()                                           :158
contextMessages = error.contextMessages.map(newDiagnosticMessage) se não vazio    :159-164
correction = error.correction                                                     :165
url      = errorCode.url                                                          :166
hasFix   = false (sempre)                                                         :174
```

`newDiagnosticMessage` (`:179-197`) converte cada mensagem de contexto com
`file = message.filePath`, `offset`, `length` da mensagem — mas calcula
linha/coluna com o **`lineInfo` do arquivo do erro** (`result.lineInfo`,
`:185-191`), mesmo quando a mensagem aponta para **outro arquivo**. Linha e
coluna de contexto em outro arquivo saem, portanto, calculadas com as quebras
de linha do arquivo errado (o `offset` está certo).

`LineInfo.getLocation` (`analyzer/lib/source/line_info.dart:77-109`): linha =
índice (1-based) do último início de linha ≤ offset; coluna =
`offset - inícioDaLinha + 1`, em **unidades UTF-16**. Os inícios de linha vêm
do scanner (`scanner.lineStarts`, `file_state.dart:672`); para arquivos
não-Dart, de `LineInfo.fromContent` (`line_info.dart:47-71`): quebra em `\n`, e
em `\r` **não seguido** de `\n` (o `\r\n` conta uma vez, no `\n`). Um offset
além do fim cai na última linha.

O `dartdev` recebe as notificações `analysis.errors` (`{"file":…, "errors":[…]}`,
`dartdev/lib/src/analysis_server.dart:81-93`) e **acumula sem substituir**
(`dartdev/lib/src/commands/analyze.dart:185-199`):

```text
para cada notificação (file, errors):
  isPriorityFile = basename(file) ∈ {'analysis_options.yaml', 'pubspec.yaml'}     :186-187
  para cada error com  !(error.type == 'TODO' && error.severity == 'INFO'):       :191-192
     se isPriorityFile && error.severity == 'ERROR': priorityErrors.add(error)    :193-194
     senão: errors.add(error)                                                     :195-197
```

- **TODOs são descartados** enquanto tiverem severidade INFO; promovidos por
  `errors: todo: warning` passam (conferido: sonda `s3`,
  `warning - lib\a.dart:1:4 - TODO: um - todo`, `WARNING|TODO|TODO|…|1|4|8|TODO: um`).
- Se o servidor enviar `analysis.errors` duas vezes para o mesmo arquivo, os
  dois lotes entram (não há substituição por arquivo). Numa execução normal do
  `dart analyze` cada arquivo é analisado uma vez; quando isso deixa de valer
  (reanálise disparada por mudança durante a análise) é **não verificado**.

### 5.3 Ordenação

`priorityErrors.sort(); errors.sort();` (`analyze.dart:241-242`) com
`AnalysisError.compareTo` (`dartdev/lib/src/analysis_server.dart:385-402`):

```text
1. severidade: ERROR (0) < WARNING (1) < INFO (2) < desconhecida (3)      :388-391, enum :319-324, mapa :329-334
2. file: String.compareTo (unidades de código UTF-16) do CAMINHO ABSOLUTO :393-395
3. offset (numérico)                                                      :397-399
4. message: String.compareTo                                              :401
```

- A severidade usada é a **já processada** pelo `errors:` (a do JSON).
- O caminho é comparado como string com os separadores do sistema: no Windows,
  `\` (0x5C) fica depois de `.` (0x2E) e das maiúsculas e antes de `_` (0x5F) e
  das minúsculas — `lib\a.dart` < `lib\a\b.dart` < `lib\a_b.dart`.
- Empate nos quatro critérios (mesmo arquivo, offset e mensagem, com `code`
  ou `length` diferentes): a ordem é a do `List.sort` do Dart, que **não é
  estável**; indefinida.
- O `code`, o `length` e a correção **não** entram na comparação.

### 5.4 Formato padrão (texto)

`run()` (`analyze.dart:100-309`) e `emitDefaultFormat` (`:325-382`). Com
`log = Logger.standard(ansi: Ansi(terminalSupportsAnsi))`
(`dartdev/lib/src/core.dart:20-21`).

```text
"Analyzing <nomes>..."                    :167-171  (só fora de json/machine)
      nomes = targets.map(basename(entity.path)).join(', ')
      — basename do caminho COMO DIGITADO; sem argumento, Directory.current (nome da pasta)
      — sem ANSI: SimpleProgress imprime '$message...' e uma quebra; finish() não imprime nada
        (cli_util-0.4.2 lib/cli_logging.dart:183-195). Com ANSI: mesma linha, com animação e,
        no fim, o tempo "0.5s" (AnsiProgress, :197-250).
… análise …
se não há nada:  "No issues found!"       :230-235  (sem linha em branco antes)
senão:
  [se há priorityErrors:]                 :263-273
     ""                                              (linha em branco)
     "Errors were found in 'pubspec.yaml' and/or 'analysis_options.yaml' which might result in
      either invalid diagnostics being produced or valid diagnostics being missed."   (uma linha)
     emit(priorityErrors)
     [se também há errors:] "Errors in remaining files."
  [se há errors:] emit(errors)            :275-277
  "<N> issue found." | "<N> issues found."   :279-280   (N = priorityErrors + errors; plural se N != 1)
```

`emit(lista)` = `emitDefaultFormat` (`:325-382`):

```text
""                                                   :334   (linha em branco ANTES do bloco)
para cada erro:
  severity = error.severity.toLowerCase().padLeft(7) :341   ("  error", "warning", "   info")
             (em vermelho se erro e ANSI)             :342-344
  filePath = _relativePath(error.file, relativeToDir) :345
  codeRef  = error.code                               :346   (com --verbose e url != null: a URL, :348-350)
  message  = ansi.emphasized(error.message)           :353   (negrito com ANSI; texto puro sem)
  se error.correction != null: message += ' ' + correction   :354-356
  location = '$filePath:${startLine}:${startColumn}'  :357
  output   = '$location $bullet $message $bullet $codeRef'   :358-360   (codeRef em verde com ANSI)
  output   = wrapText(output, width: colunasDoTerminal - 10)   :364   (só com terminal; ver abaixo)
  imprime  '$severity $bullet ' + output.replaceAll('\n', '\n' + 10 espaços)   :365-368
  para cada contextMessage:                           :371-378
     contextPath = _relativePath(error.file, relativeToDir)   ← o arquivo DO ERRO, não o da mensagem
     imprime 10 espaços + ' - ' + trimEnd(message.message, '.') + ' at ' +
             '$contextPath:${message.line}:${message.column}.'
""                                                   :381   (linha em branco DEPOIS do bloco)
```

- `bullet` = `•` se `stdout.supportsAnsiEscapes`, senão `-`
  (`cli_util` `cli_logging.dart:48`). Com a saída redirecionada: `-`, sem cores.
- `relativeToDir` (`:249-258`): com **um** alvo — a pasta alvo, ou a pasta-mãe
  do arquivo alvo; com **vários** alvos — `null` (relativo ao diretório
  corrente).
- `_relativePath` (`:481-485`): `path.relative(givenPath, from: fromDir.absolute.resolveSymbolicLinksSync())`;
  usa o relativo **só se não for mais longo** que o absoluto
  (`relative.length <= givenPath.length`). Separador do sistema.
- `wrapText` (`dartdev/lib/src/utils.dart:155-187`): só quando `stdout.hasTerminal`
  (`dartdevUsageLineLength`, `utils.dart:18-19`); quebra no último espaço antes
  da largura, senão no próximo. Redirecionado: **uma linha por diagnóstico**.
- O separador do item de contexto é sempre `' - '` literal (não o `bullet`).
- A linha de contexto usa o caminho do arquivo **do erro** com linha/coluna da
  mensagem — combinando o item de §5.2, um contexto em outro arquivo sai com o
  caminho do arquivo do erro e uma linha calculada com as quebras desse arquivo.

Conferido no binário 3.6.2 (sonda `s1`, saída redirecionada; `$` = fim de linha `\n`):

```text
Analyzing ....$
$
  error - lib\a.dart:5:11 - A value of type 'String' can't be assigned to a variable of type 'int'. Try changing the type of the variable, or casting the right-hand type to 'int'. - invalid_assignment$
  error - lib\a.dart:13:7 - The name 'A' is already defined. Try renaming one of the declarations. - duplicate_definition$
           - The first definition of this name at lib\a.dart:12:7.$
warning - lib\a.dart:8:3 - Dead code. Try removing the code, or fixing the code before it so that it can be reached. - dead_code$
$
9 issues found.$
```

(`dart analyze .` → "Analyzing ." + "..."; `dart analyze` sem argumento na
pasta `s2` → `Analyzing s2...`; `dart analyze lib/b.dart` → `Analyzing b.dart...`
e caminhos `b.dart:3:14`; `dart analyze s4 s2` → `Analyzing s4, s2...` e
caminhos `s4\lib\a.dart`.) Sonda `s4`, com erro de YAML no `analysis_options.yaml`:

```text
Analyzing s4...$
$
Errors were found in 'pubspec.yaml' and/or 'analysis_options.yaml' which might result in either invalid diagnostics being produced or valid diagnostics being missed.$
$
  error - analysis_options.yaml:5:1 - Expected node content. - parse_error$
$
Errors in remaining files.$
$
warning - lib\a.dart:1:19 - The value of the local variable 'x' isn't used. Try removing the variable or using it. - unused_local_variable$
$
2 issues found.$
```

Fim de linha: `\n` (medido com a saída redirecionada no Windows). Codificação
da saída: **não verificado** (UTF-8 esperado do `print` do Dart).

### 5.5 `--format=json`

`emitJsonFormat` (`analyze.dart:385-445`): **uma linha**, `json.encode` de

```text
{"version":1,"diagnostics":[ D, D, … ]}            (+ ,"memory":<KB> com --memory)
D = {"code": error.code,                                             :425
     "severity": error.severity,                                     :426
     "type": error.type,                                             :427
     "location": {"file": error.file,
                  "range": {"start": {"offset":o,"line":l,"column":c},
                            "end":   {"offset":o+length,"line":endLine,"column":endColumn}}},   :428-433
     "problemMessage": error.message,                                :434
     ["correctionMessage": error.correction,]   só se != null        :435
     ["contextMessages": [ {"location": {"file": msg.filePath, "range": {start, end}},
                            "message": msg.message}, … ],]  só se não vazio   :409-422, :436
     ["documentation": error.url]               só se != null        :437
    }
```

- A ordem das chaves é a de inserção acima (mapas literais do Dart).
- `file` é o caminho **absoluto** com separador do sistema (em JSON, `\\`).
- **Só `errors` é emitido** (`:246-247`): os `priorityErrors` (ERROR em
  `pubspec.yaml`/`analysis_options.yaml`) **não aparecem** no JSON — mas contam
  para o código de saída (`:287-291`). Conferido na sonda `s4`: JSON com 1
  diagnóstico, saída 3.
- Sem diagnósticos: `{"version":1,"diagnostics":[]}` (`:231-232`).
- Não há linha "Analyzing…" nem contagem. Termina com `\n` (do `print`).
- O contexto no JSON traz o `file` **da mensagem** (`contextMessage.filePath`,
  `:414`) — diferente do formato texto.

### 5.6 `--format=machine`

`emitMachineFormat` (`analyze.dart:448-461`): uma linha por diagnóstico,

```text
SEVERITY|TYPE|CODE|FILE|LINE|COLUMN|LENGTH|MESSAGE
  SEVERITY = error.severity ("ERROR"/"WARNING"/"INFO")
  TYPE     = error.type ("COMPILE_TIME_ERROR", "STATIC_WARNING", "SYNTACTIC_ERROR", "HINT", "LINT", "TODO")
  CODE     = error.code.toUpperCase()
  FILE     = _escapeForMachineMode(caminho absoluto)
  LINE, COLUMN = startLine, startColumn;  LENGTH = length
  MESSAGE  = _escapeForMachineMode(error.message)      (sem a correção)
```

`_escapeForMachineMode` (`:463-478`): `\n` → `\n` (dois caracteres), `\r` →
`\r`, e `\` e `|` ganham uma `\` na frente. No Windows o caminho sai com
barras **dobradas** (`E:\\dftemp\\…`, conferido). Também só `errors` (sem os
prioritários, `:244-245`); sem diagnósticos, **nenhuma saída** (`:233` não
imprime "No issues found!" em machine). Sem contexto, correção ou URL.

### 5.7 Código de saída

`_Result` (`analyze.dart:489-508`): `success` 0, `infos` 1, `warnings` 2,
`errors` 3, `crash` 4.

```text
sem diagnósticos (depois do filtro de TODO): 0  (4 se o servidor relatou server.error)   :230-239
hasErrors (qualquer ERROR, inclusive prioritário) → 3                                    :287-297
senão, --fatal-warnings (padrão LIGADO, :46-47) e há WARNING → 2                         :299-303
senão, --fatal-infos e há INFO → 1                                                       :304-305
senão 0
servidor caiu → mensagens em stderr/stdout e exit(4)                                     :211-215
```

A severidade é a processada: um erro rebaixado a `info` por `errors:` não dá 3.
Conferido: `s1` → 3; `s3` (só um `deprecated_member_use_from_same_package`
INFO) → 0, com `--fatal-infos` → 1; `todo: warning` → 2, com
`--no-fatal-warnings` → 0.

Opções aceitas (`:41-94`): `--fatal-infos`, `--[no-]fatal-warnings`,
`--format=default|json|machine`, `--cache=<path>`, `--memory`,
`--packages=<path>`, `--sdk-path=<path>`, `--enable-experiment=…`
(experimento desconhecido → erro de uso, `:152-165`). Alvo inexistente →
erro de uso `Directory or file doesn't exist: <alvo>` (`:117`). Vários alvos
são aceitos.

### 5.8 No DartForge

`crates/cli/src/analisar.rs` (113 linhas) e `crates/paridade/src/json.rs`.

| aspecto | DartForge | divergência |
|---|---|---|
| opções | `--format=json`, `--format=default`, `--todos`; um alvo (`analisar.rs:23-36`) | faltam `--format=machine`, `--fatal-infos`, `--[no-]fatal-warnings`, `--packages`, `--sdk-path`, `--enable-experiment`, vários alvos; alvo inexistente não dá o erro de uso |
| cabeçalho | `Analyzing <file_name do caminho absoluto>...` + linha em branco (`:80`) | o oficial usa o basename **como digitado** (`.` → `Analyzing ....`); a linha em branco pertence ao bloco de erros: com zero diagnósticos o oficial imprime `Analyzing x...` e logo `No issues found!`, **sem** linha em branco (sonda `s2`) — o DartForge imprime uma a mais |
| linha de diagnóstico | `{:>7} - rel:linha:coluna - msg[ correção] - código` (`:89-97`) | igual ao oficial sem ANSI. Falta a versão com ANSI/`•` e a quebra por largura quando há terminal (decidir: paridade só no modo redirecionado) |
| caminho relativo | relativo à **raiz do pacote** (`raiz_do_pacote`, `:10-20`, `:82-83`) | o oficial relativiza ao **alvo** (pasta alvo, ou pasta do arquivo alvo), e só usa o relativo se não for mais longo: `dart analyze lib` imprime `a.dart:…`, não `lib\a.dart:…` |
| contexto | não há | faltam as linhas `           - … at arquivo:linha:coluna.` e o `contextMessages` do JSON (`json.rs:24-37` não tem o campo) |
| bloco prioritário | não há | nenhum arquivo não-Dart é analisado (§7) |
| rodapé | `\n<N> issue(s) found.` / `No issues found!` (`:99-103`) | igual, salvo a linha em branco do item "cabeçalho" |
| ordenação | severidade, arquivo, linha, coluna, mensagem (`json.rs:147-166`) | equivalente a (severidade, arquivo, offset, mensagem) enquanto o arquivo for a mesma string; o oficial compara o caminho **canônico** (`resolveSymbolicLinksSync`, `analysis_server.dart:178-182`) — conferir caixa da letra da unidade e links |
| JSON | compacto, mesma ordem de chaves (`json.rs:16-37`, `:168-171`), **sem** `\n` final (`print!`, `analisar.rs:78`) | o oficial termina com `\n`; falta `contextMessages`; o escape de strings do `serde_json` × `json.encode` do Dart é **não verificado** para caracteres de controle e substitutos soltos |
| TODO | não emite | equivalente por enquanto (o oficial descarta TODO INFO); muda com `errors: todo: …` |
| código de saída | 3 com erro, 2 com aviso, 0 (`:105-112`) | falta 1 (`--fatal-infos`), `--no-fatal-warnings` e 4 |
| filtro de publicação | só sintaxe + `verificados.txt` (`--todos` desliga) | por definição não é paridade; ver Parte III, etapa 9 |

## 6. Versão de linguagem e experimentos

### 6.1 De onde vem a versão de cada arquivo

Cada `FileState` nasce com um `featureSet` e um `packageLanguageVersion`
(`FileSystemState._newFile`, `analyzer/lib/src/dart/analysis/file_state.dart:1645-1664`):

```text
_getLanguageVersion(path, uri, workspacePackage, analysisOptions):          file_state.dart:1630-1643
  se workspacePackage.languageVersion != null: ela     (ganchos de workspaces; `null` no pub — workspace.dart:79)
  senão featureSetProvider.getLanguageVersion(path, uri, nonPackageLanguageVersion):
                                                        feature_set_provider.dart:80-98
     uri `dart:`            → sdk.languageVersion                           :85-87
     arquivo de um pacote   → package.languageVersion (do package_config.json), ou,
                              se o pacote não declara, sdk.languageVersion  :88-95
     fora de qualquer pacote→ analysisOptions.nonPackageLanguageVersion
                              (= ExperimentStatus.currentVersion, engine.dart:182)   :97
_getFeatureSet(path, uri, workspacePackage, analysisOptions):               file_state.dart:1612-1628
  se workspacePackage.enabledExperiments != null: featureSetForExperiments(eles)
  senão featureSetProvider.getFeatureSet(path, uri, contextFeatures, nonPackageFeatureSet):
                                                        feature_set_provider.dart:47-74
     uri `dart:`            → experimentos permitidos para a biblioteca do SDK
                              (allowed_experiments.json do SDK)             :53-62
     arquivo de um pacote   → os experimentos permitidos ao pacote (allowed_experiments.json), ou
                              analysisOptions.contextFeatures               :64-71
     fora de pacote         → analysisOptions.nonPackageFeatureSet          :73
```

"Pacote" = o de `package:`/`asset:` pelo nome, ou, para `file:`, o pacote do
`package_config.json` cuja raiz contém o caminho (`_findPackage`, `:109-128`).
A `languageVersion` do `package_config.json` é reduzida a `major.minor.0`
(`analyzer/lib/src/context/packages.dart:52-60`).

`contextFeatures` é o `ExperimentStatus()` padrão (todos os recursos lançados
até a versão corrente, `experiments.dart:50-52`, `:115-120`), trocado por:

- `analyzer: enable-experiment:` do `analysis_options.yaml`
  (`apply_options.dart:190-203`), ou
- `--enable-experiment=` do `dart analyze`, que o servidor aplica **por cima**
  das opções de cada contexto (`analysis_server/lib/src/context_manager.dart:563-574`:
  `analysisOptions.contextFeatures = FeatureSet.fromEnableFlags2(sdkLanguageVersion: sdk.languageVersion, flags: _enabledExperiments)`;
  a linha de comando **substitui** a lista do YAML, não soma).

Versão corrente da 3.6.2: **`3.6.0`** (`_currentVersion`,
`analyzer/lib/src/dart/analysis/experiments.g.dart:15`).

### 6.2 `// @dart=x.y`

O scanner fasta reconhece o comentário de versão no cabeçalho e chama
`_languageVersionChanged` (`analyzer/lib/src/dart/scanner/scanner.dart:166-195`):

```text
se major < 0 ou minor < 0: ignora                                           :170-172
overrideVersion = Version(major, minor, 0); _overrideVersion = ela          :174-175
se overrideVersion > ExperimentStatus.currentVersion (3.6.0):               :177-188
    relata WarningCode.INVALID_LANGUAGE_VERSION_OVERRIDE_GREATER
        offset/length = o token do comentário; argumentos [3, 6]
    _overrideVersion = null            (a versão do arquivo volta a ser a do pacote)
senão:                                                                      :189-194
    _featureSet = _featureSetForOverriding.restrictToVersion(overrideVersion)
    scanner.configuration = buildConfig(_featureSet)   (o resto do arquivo é escaneado com os novos recursos)
```

O scanner começa com `featureSet.restrictToVersion(packageLanguageVersion)` e
guarda o conjunto **sem restrição** para o caso de override
(`file_state.dart:664-670`). A unidade fica com
`languageVersion = LibraryLanguageVersion(package: …, override: scanner.overrideVersion)`
(`file_state.dart:682-685`) e `unit.featureSet = scanner.featureSet` (passado ao
`Parser`, `:674-679`).

A gramática do comentário aceito pelo scanner está em
`_fe_analyzer_shared/lib/src/scanner/abstract_scanner.dart` (reconhecimento em
torno de `:1502`, disparo do retorno de chamada em `:1574-1585`; **não
verificado** linha a linha aqui; o DartForge a reproduz em
`crates/frontend/src/features.rs:258-345`, com testes em `:366-422`). Os
comentários **quase** válidos (`// @dart = 2.x` com três barras, sem `@`, com
letra, fora do lugar…) são tratados depois, na fase C4g, pelo
`LanguageVersionOverrideVerifier`
(`analyzer/lib/src/error/language_version_override_verifier.dart:18-34`,
`:79-282`), que emite as variantes de `invalid_language_version_override`:
`_TWO_SLASHES` (`:192`), `_AT_SIGN` (`:201`), `_LOWER_CASE` (`:211`), `_EQUALS`
(`:223`), `_PREFIX` (`:232`), `_NUMBER` (`:241`), `_TRAILING_CHARACTERS` (`:278`)
e `_LOCATION` (comentário válido depois de código, `_verifyMisplaced`, `:284-307`). A versão `> corrente` é do
scanner (acima); a de um `part` diferente da biblioteca é do `LibraryAnalyzer`
(§1.2, C6).

### 6.3 Como a versão liga e desliga recursos

`restrictEnableFlagsToVersion` (`analyzer/lib/src/dart/analysis/experiments_impl.dart:111-153`),
para cada recurso conhecido:

```text
se explicitamente desligado (`no-<nome>`): desligado                        :119-122
se releaseVersion != null e version >= releaseVersion: ligado               :124-127
se explicitamente ligado (enable-experiment):                               :129-150
   sem experimentalReleaseVersion: ligado só se version == sdkLanguageVersion   :131-137
   com:  ligado se version >= experimentalReleaseVersion ou version >= sdkLanguageVersion   :145-148
```

Tabela dos experimentos da 3.6.2 (`experiments.g.dart`; "expirado" = o
sinalizador não tem mais efeito e o recurso está sempre ligado a partir da
versão de lançamento):

| experimento (`enable-experiment`) | ligado por padrão | expirado | versão experimental | versão de lançamento | `experiments.g.dart` |
|---|---|---|---|---|---:|
| `augmentations` | não | não | null | null | :174 |
| `class-modifiers` | sim | sim | null | 3.0.0 | :184 |
| `const-functions` | não | não | null | null | :194 |
| `constant-update-2018` | sim | sim | null | 2.0.0 | :205 |
| `constructor-tearoffs` | sim | sim | null | 2.15.0 | :215 |
| `control-flow-collections` | sim | sim | null | 2.0.0 | :226 |
| `digit-separators` | sim | não | null | 3.6.0 | :236 |
| `enhanced-enums` | sim | sim | null | 2.17.0 | :246 |
| `enhanced-parts` | não | não | null | null | :256 |
| `extension-methods` | sim | sim | null | 2.6.0 | :266 |
| `generic-metadata` | sim | sim | null | 2.14.0 | :276 |
| `inference-update-1` | sim | sim | null | 2.18.0 | :287 |
| `inference-update-2` | sim | sim | null | 3.2.0 | :298 |
| `inference-update-3` | sim | sim | null | 3.4.0 | :308 |
| `inference-update-4` | não | não | null | null | :319 |
| `inference-using-bounds` | não | não | null | null | :329 |
| `inline-class` | sim | sim | null | 3.3.0 | :340 |
| `macros` | não | não | 3.3.0 | null | :350 |
| `named-arguments-anywhere` | sim | sim | null | 2.17.0 | :360 |
| `native-assets` | não | não | null | null | :370 |
| `non-nullable` | sim | sim | 2.10.0 | 2.12.0 | :380 |
| `nonfunction-type-aliases` | sim | sim | null | 2.13.0 | :390 |
| `null-aware-elements` | não | não | null | null | :400 |
| `patterns` | sim | sim | null | 3.0.0 | :410 |
| `record-use` | não | não | null | null | :420 |
| `records` | sim | sim | null | 3.0.0 | :430 |
| `sealed-class` | sim | sim | null | 3.0.0 | :440 |
| `set-literals` | sim | sim | null | 2.0.0 | :450 |
| `spread-collections` | sim | sim | null | 2.0.0 | :460 |
| `super-parameters` | sim | sim | null | 2.17.0 | :470 |
| `test-experiment` | não | não | null | null | :480 |
| `triple-shift` | sim | sim | null | 2.14.0 | :491 |
| `unnamed-libraries` | sim | sim | null | 2.19.0 | :501 |
| `unquoted-imports` | não | não | null | null | :511 |
| `variance` | não | não | null | null | :521 |
| `wildcard-variables` | não | não | null | null | :531 |

Consequências práticas para o `dart analyze` 3.6.2:

- Um pacote com `languageVersion` 3.5 analisa **sem** `digit-separators`
  (lançado em 3.6.0); com 2.19, sem `records`/`patterns`/`class-modifiers`/
  `sealed-class` (3.0.0); com 3.2, sem `inline-class` (3.3.0).
- `enable-experiment: [wildcard-variables]` (sem versão experimental) só tem
  efeito em bibliotecas na versão **exatamente** 3.6 (a do SDK).
- O parser relata o uso de um recurso desligado com `experiment_not_enabled`
  (`AstBuilder._reportFeatureNotEnabled`, `analyzer/lib/src/fasta/ast_builder.dart:6078-6093`):
  argumentos `[feature.enableString, versão]`, onde a versão é
  `feature.releaseVersion ?? ExperimentStatus.currentVersion` — para um
  experimento ainda não lançado a mensagem cita **3.6.0** (T2); posição: de
  `startToken` ao fim de `endToken ?? startToken` (`handleRecoverableError`,
  `:5340-5357`). Os recursos lançados usam `experiment_not_enabled`; os não
  lançados podem usar `experiment_not_enabled_off_by_default` (ver II.4: na
  3.6.2 essa constante não tem emissor direto).
- O **verificador** também consulta recursos pela versão da biblioteca
  (`_libraryElement.featureSet`, `unit.featureSet`): `enhanced-parts`
  (`library_analyzer.dart:1050`), `inference-update-*`, `wildcard-variables`,
  `null-aware-elements`, `augmentations`, `macros`.

### 6.4 Restrição de SDK do pubspec: `sdk_version_*`

`SdkConstraintVerifier` (`analyzer/lib/src/hint/sdk_constraint_verifier.dart`,
237 linhas), fase C4j, só quando o arquivo pertence a um `PubPackage` cujo
`pubspec.yaml` tem `environment: sdk:` analisável
(`library_analyzer.dart:526-535`; `workspace/pub.dart:448-462`). Recebe
`sdkVersionConstraint.withoutPreRelease`.

Na 3.6.2 restam **dois** códigos (as outras variantes históricas
`sdk_version_async_exported_from_core`, `…_set_literal`, `…_never` etc. não
existem mais na tabela):

| código | condição | posição | linhas |
|---|---|---|---|
| `sdk_version_gt_gt_gt_operator` | a restrição admite alguma versão `< 2.14.0` (`checkTripleShift`, `:53-54`) e (a) uma `BinaryExpression` com operador `>>>`, ou (b) a declaração de um `operator >>>` | (a) `atToken(node.operator)`; (b) `atToken(node.name)` | `:78-89`, `:115-123` |
| `sdk_version_since` | o elemento referenciado tem `sinceSdkVersion` (da anotação `@Since('x.y')` do SDK) e `!constraint.requiresAtLeast(sinceSdkVersion)`; argumentos `[sinceSdkVersion.toString(), constraint.toString()]` | a entidade conforme o nó (tabela em `:171-198`): nome do construtor ou do tipo, nome do método, identificador, `[` do índice, lista de argumentos da invocação de função, o argumento posicional | `:162-211` |

Com `sdk: ^3.6.0` (ou qualquer mínimo ≥ a versão do `@Since`) nenhum sai. A
especificação completa está em II.8.

### 6.5 No DartForge

- Versões e recursos: `crates/frontend/src/features.rs`
  (`LanguageVersion` `:20`, `Feature` `:77`, `LibraryFeatures::para_biblioteca`
  `:201`, `marcador_versao` `:268`); `package_config.json`:
  `crates/elements/src/config.rs:154-230` (`languageVersion` em `:213-225`,
  inclusive o texto inválido).
- Diferenças a conferir contra esta seção:
  1. `enable-experiment` do `analysis_options.yaml` não é lido
     (`crates/paridade/src/filtros.rs` só lê `exclude`/`errors`/`cannot-ignore`);
     `--enable-experiment` não existe no `dartforge analyze`.
  2. A regra "experimento sem versão experimental só vale na versão exata do
     SDK" (`experiments_impl.dart:131-137`): **não verificado** em `features.rs`.
  3. Arquivo fora de qualquer pacote: versão corrente (3.6) e todos os recursos
     lançados — `LibraryFeatures::atual` (`features.rs:215`); conferir que é
     esse o caminho usado quando não há `package_config.json`.
  4. `allowed_experiments.json` (macros para o pacote `json`): irrelevante na
     prática; não implementado.
  5. `SdkConstraintVerifier`: não implementado (II.8); exige ler
     `environment: sdk:` do `pubspec.yaml` e os `@Since` do SDK.
  6. O oráculo por arquivo (3.6.2 × 3.13.4) é o T2 do outro documento; esta
     seção descreve só o comportamento do 3.6.2.

## 7. `TodoFinder` e arquivos não-Dart

### 7.1 `TodoFinder`: `todo`, `fixme`, `hack`, `undone`

Fase C4f (`library_analyzer.dart:495`), `analyzer/lib/src/error/todo_finder.dart`
e `analyzer/lib/src/dart/error/todo_codes.dart`.

Os quatro códigos são `TodoCode('TODO' | 'FIXME' | 'HACK' | 'UNDONE')`
(`todo_codes.dart:64-79`), todos com **molde `{0}`** (a mensagem é o próprio
texto do comentário), severidade INFO, tipo `TODO` (`:84-95`), sem correção e
sem URL.

Expressão (`Todo.TODO_REGEX`, `todo_codes.dart:42-44`, com
`_TODO_KIND_PATTERN = 'TODO|FIXME|HACK|UNDONE'`, `:46`):

```text
([\s/\*])                                   grupo 1: um caractere antes (espaço, '/', ou '*')
(  ((?<kind1>TODO|FIXME|HACK|UNDONE)        grupo 2 (alternativa A): a palavra,
      [^\w\d]                                 seguida de um caractere que não é de palavra nem dígito,
      [^\r\n]*                                o resto da linha,
      (?:\n\s*\*  [^\r\n]*)*  )               e linhas seguintes de comentário de bloco "* " + 2 espaços
 | ((?<kind2>TODO|FIXME|HACK|UNDONE):?$) )  grupo 2 (alternativa B): a palavra (com ':' opcional) no fim do texto
```

Sensível a maiúsculas (`todo` não casa); `TODOS` não casa (precisa de
não-palavra depois, ou do fim); `TODO(user): x`, `TODO: x`, `TODO x` casam.
Sem a opção `multiLine`, `$` é o fim do **lexema** do comentário.

Varredura (`_gatherTodoComments`, `todo_finder.dart:44-57`): para cada token da
unidade (até o EOF, exclusive — `!token.isEof`, `:45`; comentários pendurados
no token EOF **não** são examinados), cada comentário de `precedingComments`
do tipo `SINGLE_LINE_COMMENT` ou `MULTI_LINE_COMMENT` (`:48-49`; isso inclui
`///` e `/** */`, que o scanner entrega com esses tipos).

`_scrapeTodoComment` (`:67-140`), para **cada** casamento no lexema:

```text
offset   = commentToken.offset + match.start + grupo1.length          :77
column   = colunaDoComentário + match.start + grupo1.length           :78-79
todoText = grupo 2;  kind = kind1 ?? kind2                            :80-81
end      = offset + todoText.length                                   :82
se comentário de bloco:                                               :84-93
    se todoText termina com '*/': corta e trimRight(); end = offset + todoText.length
    todoText = todoText.replaceAll(/\s*\n\s*\*\s*/, ' ')    (o length NÃO é recalculado)
se comentário de linha:                                               :94-129
    enquanto o próximo comentário for continuação:
        é SINGLE_LINE_COMMENT, não começa com '///',
        está na linha imediatamente seguinte,
        começa na mesma coluna do comentário original,
        o primeiro caractere que não é '/' nem espaço está na coluna (column + 1)
            (isto é, recuado UM espaço a mais que a palavra TODO),
        e não contém ele mesmo um TODO
      → end = nextComment.end; todoText += ' ' + texto da continuação (trimRight)
relata atOffset(offset, length: end - offset, Todo.forKind(kind), [todoText])   :131-136
```

A função devolve o próximo comentário a examinar, pulando as continuações
consumidas (`:139`).

Conferido no binário 3.6.2 (sonda `s3/lib/b.dart`, com
`errors: {todo, fixme, hack, undone}: warning`, formato machine
`…|linha|coluna|length|mensagem`):

| fonte | saída |
|---|---|
| `/// TODO: doc` (linha 1) | `TODO … 1\|5\|9\|TODO: doc` — comentário de documentação conta |
| `/** HACK bloco` / ` *  continua aqui` / ` */` | `HACK … 3\|5\|28\|HACK bloco continua aqui` — a mensagem é desdobrada, o length cobre as duas linhas |
| `// FIXME(x): linha um` / `//  continua` / `// nao continua` | `FIXME … 7\|4\|31\|FIXME(x): linha um continua` — só a linha recuada um espaço a mais continua |
| `void c() {} // UNDONE` | `UNDONE … 10\|16\|6\|UNDONE` — alternativa B (palavra no fim) |
| `// todo: minusculo TODOS nao` | nada |
| `// TODO: no fim do arquivo` (último comentário, depois da última declaração) | **nada** — comentário pendurado no EOF não é examinado |

Saída: o `dart analyze` **descarta** diagnósticos `type == 'TODO'` com
`severity == 'INFO'` (§5.2). Eles só aparecem quando `analyzer: errors:`
promove `todo`/`fixme`/`hack`/`undone` a `warning`/`error` (sonda `s3`:
`warning - lib\a.dart:1:4 - TODO: um - todo`, length 8). O servidor os envia
sempre (o LSP tem a sua própria opção para mostrá-los). Especificação em seis
campos: II.8.

### 7.2 Arquivos não-Dart: quem valida, quando, e como sai

O `LibraryAnalyzer` não participa. O **servidor**, ao montar os contextos,
percorre `contextRoot.analyzedFiles()` e despacha pelo **nome base**
(`analysis_server/lib/src/context_manager.dart:598-610`;
`analyzer/lib/src/util/file_paths.dart:60-67`, `:122-124`):

| arquivo | função | validador | códigos |
|---|---|---|---|
| `analysis_options.yaml` | `_analyzeAnalysisOptionsYaml` (`:369-394`) | `analyzeAnalysisOptions(source, content, sourceFactory, raizDoContexto, sdkVersionConstraint)` (`analyzer/lib/src/task/options.dart`) | `AnalysisOptionsErrorCode` (2), `AnalysisOptionsWarningCode` (17), `AnalysisOptionsHintCode` (3) |
| `pubspec.yaml` | `_analyzePubspecYaml` (`:461-486`) | `validatePubspec(contents:, source:, provider:, analysisOptions:)` (`analyzer/lib/src/pubspec/`) | `PubspecWarningCode` (24) |
| `AndroidManifest.xml` | `_analyzeAndroidManifestXml` (`:398-417`) | `ManifestValidator(source).validate(content, analysisOptions.chromeOsManifestChecks)` | `ManifestWarningCode` (7) — só com `optional-checks: chrome-os-manifest-checks` |
| `lib/fix_data.yaml`, `lib/fix_data/**.yaml` | `_analyzeFixDataYaml` (`:436-457`, chamado em `:612-625`) | `TransformSetParser` (do `analysis_server`) | códigos próprios do servidor (`TransformSetErrorCode`, fora da tabela do analyzer; **não verificado**) |

Regras comuns (todas lidas em `context_manager.dart`):

1. **Uma vez por arquivo**, pelo nome, em **qualquer subpasta analisada** da
   raiz (não só na raiz do pacote): um `example/pubspec.yaml` e um
   `test/analysis_options.yaml` também são validados; os excluídos (§4.5) não.
2. Se o alvo do `dart analyze` é um arquivo ou uma subpasta (`lib/`), o
   `pubspec.yaml` e o `analysis_options.yaml` da raiz **não** são validados
   (ficam fora de `analyzedFiles()`), embora as opções continuem **valendo**
   para a análise (são achadas subindo as pastas, `analysis_options_provider.dart:42-50`).
3. Qualquer exceção dentro do validador é engolida e o arquivo fica **sem
   diagnósticos** (`try { … } catch (exception) {}` em `:372-392`, `:400-415`,
   `:439-455`, `:463-484`); a lista vazia é enviada do mesmo jeito (`:393`,
   `:416`, `:456`, `:485`). Ex.: `pubspec.yaml` com YAML inválido —
   `loadYamlNode` lança (`:466`) → nenhum diagnóstico de pubspec.
4. Conversão para o protocolo: `AnalyzerConverter.convertAnalysisErrors(errors,
   lineInfo: LineInfo.fromContent(content), options: analysisOptions)`
   (`:386-388`, `:409-411`, `:448-451`, `:477-480`) — aplica o `ErrorProcessor`
   (`analyzer: errors:`) e calcula linha/coluna (§4.4, item 5; §5.2).
5. Posição: `atSourceSpan(node.span, …)` sobre os nós do `package:yaml`
   (`listener.dart:189-205`): offset/length do span do nó YAML.
6. Saída: iguais aos de arquivo Dart, com o caminho do arquivo YAML/XML. No
   `dartdev`, os de severidade ERROR em `pubspec.yaml`/`analysis_options.yaml`
   vão para o bloco prioritário (§5.4) e **somem** de `--format=json|machine`
   (§5.5, §5.6).

O detalhe — algoritmo do `include:`, validação da lista de lints, cada código
com condição e posição, e os defeitos da fonte que a paridade tem de
reproduzir — está na Parte II, II.10.

### 7.3 No DartForge

- `TodoFinder`: não existe (nenhum `todo`/`fixme`/`hack`/`undone` é emitido;
  os quatro códigos estão em `crates/diagnostics/src/codigos_g.rs`, classe
  `TodoCode`, cabeçalho `:10`). Como o `dart analyze` os descarta por padrão,
  só fazem falta (a) com `errors: todo: warning|error` e (b) no LSP.
- Arquivos não-Dart: nenhum validador. `dartforge analyze` só coleta `.dart`
  (`crates/cli/src/analisar.rs:40-47`, `crates/paridade/src/corpus.rs:60-80`) e
  o placar descarta do oráculo o que não é `.dart`
  (`crates/paridade/src/oraculo.rs:87-91`, citado pelo levantamento de II.10).
  O `analysis_options.yaml` é lido só para `exclude`/`errors`/`cannot-ignore`
  (`crates/paridade/src/filtros.rs:33-90`), sem posições. É a etapa 7 da Parte III.

## 8. Lints: mecanismo, registro e lista de regras

Esta seção especifica só o **mecanismo**; a regra de cada lint fica fora do
escopo (são 240 regras; nenhuma está implementada no DartForge).

### 8.1 Sem `analysis_options.yaml` não há lint nenhum

- `AnalysisOptionsImpl` nasce com `lint = false` e `lintRules` vazio
  (`analyzer/lib/src/generated/engine.dart:207-208`, `:215`, `:316`).
- Sem arquivo de opções o contexto usa `AnalysisOptionsImpl(file: null)` sem
  `applyOptions` (`analyzer/lib/src/dart/analysis/context_builder.dart:303-313`;
  `AnalysisOptionsMap._defaultOptions`, `analysis_options_map.dart:13`).
- `lint` só vira `true` em `applyOptions`, e só se pelo menos uma regra
  **registrada** foi ligada (`analyzer/lib/src/analysis_options/apply_options.dart:229-236`).
- `LibraryAnalyzer` só chama `_computeLints` com `analysisOptions.lint`
  (`library_analyzer.dart:333-335`).

Confirmado: sem `analysis_options.yaml` (sondas `s1`–`s3`) nenhum diagnóstico
`LINT` aparece; com `linter: rules: [camel_case_types]` (sonda `s7`) sai
`   info - lib\a.dart:1:7 - The type name 'a_b' isn't an UpperCamelCase identifier. Try changing the name to follow the UpperCamelCase style. - camel_case_types`
(`INFO|LINT|CAMEL_CASE_TYPES|…|1|7|3|…`).

Não há conjunto de lints "padrão do SDK". O que os projetos têm é o
`analysis_options.yaml` gerado pelo `dart create`/`flutter create` com
`include: package:lints/recommended.yaml` (ou `package:flutter_lints/flutter.yaml`).

### 8.2 Como `package:lints` liga as regras

`include: package:lints/recommended.yaml` é resolvido pelo `SourceFactory` do
contexto (`AnalysisOptionsProvider.getOptionsFromSource`,
`analyzer/lib/src/analysis_options/analysis_options_provider.dart:66-84`, §4.6) —
portanto **depende do `package_config.json`**: sem `dart pub get`, o include
não resolve, as regras não são ligadas, e o validador relata
`include_file_not_found` (II.10).

`package:lints` 5.1.1 (cache do pub; `pubspec.yaml` do pacote exige
`sdk: ^3.6.0`; a revisão fixada no `DEPS` da 3.6.2 é `lints_rev`
`5016d63…`, **não conferida** contra a 5.1.1):

- `lib/core.yaml`: 34 regras em `linter: rules:` (lista);
- `lib/recommended.yaml`: `include: package:lints/core.yaml` (`recommended.yaml:10`)
  + 55 regras próprias — 89 no total, pela fusão de listas do `Merger` (§4.6).

A coluna "`package:lints` 5.1.1" da tabela de §8.5 marca cada regra como
`core` ou `recommended`.

Leitura da seção `linter:` (`parseConfig` → `LintConfig.parseMap`,
`analyzer/lib/src/lint/config.dart:10-19`, `:116-169`): em `rules:`,

- **lista** de nomes → cada um `enabled: true` (`:133-140`);
- **mapa** `nome: true|false` → `enabled` conforme o valor (`:143-151`); os
  textos `'true'`/`'false'` também valem (`asBool`, `:76-90`);
- mapa de mapas (`grupo: {nome: bool}`) → idem, com `group` (`:154-164`).

`Registry.enabled(config)` (`analyzer/lib/src/lint/registry.dart:42-43`) =
as regras registradas para as quais **algum** `RuleConfig` tem
`enables(nome)` (`name == nome && args['enabled'] == true`, `config.dart:55`).
Um `false` só desliga porque, na fusão de mapas do `include`, o valor do
incluidor **substitui** o do incluído (uma regra não aparece duas vezes num
mapa); misturar lista (incluído) com mapa (incluidor) funciona porque a lista é
promovida a mapa `{nome: true}` antes de fundir (`analyzer/lib/src/util/yaml.dart:35-50`).

### 8.3 Registro

- As regras são objetos `LintRule` (`analyzer/lib/src/lint/linter.dart:205`)
  registrados no singleton `Registry.ruleRegistry`
  (`analyzer/lib/src/lint/registry.dart:12-63`: mapa nome → regra e mapa
  `uniqueName` → `LintCode`, `:17-20`, `:49-54`).
- Quem registra: `registerLintRules()` (`linter/lib/src/rules.dart:247`, com
  240 chamadas `..register(…)`), chamado pelo servidor ao iniciar
  (`analysis_server/lib/src/server/driver.dart:378`, `:486`). O analyzer
  sozinho, sem essa chamada, tem o registro **vazio** — e aí `linter: rules:`
  não liga nada.
- Estados (`analyzer/lib/src/lint/state.dart:17-89`): `stable`,
  `experimental`, `deprecated`, `removed`, `internal`. Na 3.6.2: 216 estáveis,
  12 removidas, 10 experimentais, 2 internas, **nenhuma obsoleta** (contado em
  `linter/lib/src/rules/*.dart`). Regras removidas continuam registradas (para
  o validador de opções reconhecer o nome), mas o corpo delas não relata nada
  e o único código é `removed_lint` (ex.: `linter/lib/src/rules/avoid_as.dart:15-19`).
- Códigos: `LinterLintCode` (`linter/lib/src/linter_lint_codes.dart`, 260
  constantes): `name` = nome da regra em **minúsculas** (é o que sai em `code`),
  `uniqueName` próprio quando a regra tem mais de uma mensagem
  (`always_declare_return_types_of_functions`/`…_of_methods`, `:23-34`);
  severidade INFO, tipo `LINT` (`analyzer/lib/src/dart/error/lint_codes.dart:16-36`).
  Por o `name` já ser minúsculo, o `ErrorProcessor` e o `cannot-ignore` comparam
  também `name.toUpperCase()` (§4.3, §4.4).

### 8.4 Execução

`LibraryAnalyzer._computeLints` (`library_analyzer.dart:352-418`), fase C5 —
**depois** de todos os erros e avisos, **antes** do `IgnoreValidator`:

```text
para cada unidade: LintRuleUnitContext(file, content, unit, errorReporter)     :357-369
context = LinterContextWithResolvedResults(todas as unidades, a unidade definidora,
            typeProvider, typeSystem, inheritance, workspacePackage)           :376-383
nodeRegistry = NodeLintRegistry(enableTiming)                                   :375
para cada regra de analysisOptions.lintRules (na ordem da lista):               :385-390
    regra.registerNodeProcessors(nodeRegistry, context)     (a regra se inscreve nos tipos de nó)
para cada unidade (ordem de _libraryFiles):                                     :396-415
    para cada regra: regra.reporter = errorReporter da unidade                  :405-407
    unit.accept(LinterVisitor(nodeRegistry, logException))                      :410-412
        (um único percurso da AST; em cada nó, chama os visitantes inscritos, na ordem de inscrição)
LinterVisitor(nodeRegistry, logException).afterLibrary()                        :417
```

Relato: `LintRule.reportLint(node, arguments:, contextMessages:, errorCode:)` →
`reporter.atNode(node, errorCode ?? lintCode, …)`, **pulando nós sintéticos**
(`linter.dart:286-299`); `reportLintForToken` idem para tokens (`:314-327`);
`reportLintForOffset` (`:301-312`). Tudo passa pelo `ErrorReporter` da unidade
(§3): mesma deduplicação, mesmos `// ignore:` (pelo nome da regra ou
`type=lint`), mesmo `errors:` (`camel_case_types: error` promove).

Lints também podem ler **constantes**: os que perguntam "isto pode ser
`const`?" reavaliam a expressão com um listener que observa os códigos
`CONST_*` (`linter.dart:368-420`; nota da §1.3).

Exceções dentro de uma regra são registradas e engolidas
(`LinterExceptionHandler`, `:392-394`), salvo
`optional-checks: propagate-linter-exceptions`.

### 8.5 Lista das regras registradas na 3.6.2

Gerada de `linter/lib/src/rules.dart` (ordem do registro) e de cada
`linter/lib/src/rules/<arquivo>.dart` (nome e `state:`), tag 3.6.2
(`E:\dftemp\analise\spec-infra\auto.py`). "estável" = sem `state:` explícito.

| # | regra | estado | `rules/…` | `package:lints` 5.1.1 |
|---:|---|---|---|---|
| 1 | `always_declare_return_types` | estável | `always_declare_return_types.dart` |  |
| 2 | `always_put_control_body_on_new_line` | estável | `always_put_control_body_on_new_line.dart` |  |
| 3 | `always_put_required_named_parameters_first` | estável | `always_put_required_named_parameters_first.dart` |  |
| 4 | `always_require_non_null_named_parameters` | removida (desde dart3_3) | `always_require_non_null_named_parameters.dart` |  |
| 5 | `always_specify_types` | estável | `always_specify_types.dart` |  |
| 6 | `always_use_package_imports` | estável | `always_use_package_imports.dart` |  |
| 7 | `analyzer_use_new_elements` | interna | `analyzer_use_new_elements.dart` |  |
| 8 | `annotate_overrides` | estável | `annotate_overrides.dart` | recommended |
| 9 | `annotate_redeclares` | experimental | `annotate_redeclares.dart` |  |
| 10 | `avoid_annotating_with_dynamic` | estável | `avoid_annotating_with_dynamic.dart` |  |
| 11 | `avoid_as` | removida (desde dart2_12) | `avoid_as.dart` |  |
| 12 | `avoid_bool_literals_in_conditional_expressions` | estável | `avoid_bool_literals_in_conditional_expressions.dart` |  |
| 13 | `avoid_catches_without_on_clauses` | estável | `avoid_catches_without_on_clauses.dart` |  |
| 14 | `avoid_catching_errors` | estável | `avoid_catching_errors.dart` |  |
| 15 | `avoid_classes_with_only_static_members` | estável | `avoid_classes_with_only_static_members.dart` |  |
| 16 | `avoid_double_and_int_checks` | estável | `avoid_double_and_int_checks.dart` |  |
| 17 | `avoid_dynamic_calls` | estável | `avoid_dynamic_calls.dart` |  |
| 18 | `avoid_empty_else` | estável | `avoid_empty_else.dart` | core |
| 19 | `avoid_equals_and_hash_code_on_mutable_classes` | estável | `avoid_equals_and_hash_code_on_mutable_classes.dart` |  |
| 20 | `avoid_escaping_inner_quotes` | estável | `avoid_escaping_inner_quotes.dart` |  |
| 21 | `avoid_field_initializers_in_const_classes` | estável | `avoid_field_initializers_in_const_classes.dart` |  |
| 22 | `avoid_final_parameters` | estável | `avoid_final_parameters.dart` |  |
| 23 | `avoid_function_literals_in_foreach_calls` | estável | `avoid_function_literals_in_foreach_calls.dart` | recommended |
| 24 | `avoid_futureor_void` | experimental | `avoid_futureor_void.dart` |  |
| 25 | `avoid_implementing_value_types` | estável | `avoid_implementing_value_types.dart` |  |
| 26 | `avoid_init_to_null` | estável | `avoid_init_to_null.dart` | recommended |
| 27 | `avoid_js_rounded_ints` | estável | `avoid_js_rounded_ints.dart` |  |
| 28 | `avoid_multiple_declarations_per_line` | estável | `avoid_multiple_declarations_per_line.dart` |  |
| 29 | `avoid_null_checks_in_equality_operators` | estável | `avoid_null_checks_in_equality_operators.dart` |  |
| 30 | `avoid_positional_boolean_parameters` | estável | `avoid_positional_boolean_parameters.dart` |  |
| 31 | `avoid_print` | estável | `avoid_print.dart` |  |
| 32 | `avoid_private_typedef_functions` | estável | `avoid_private_typedef_functions.dart` |  |
| 33 | `avoid_redundant_argument_values` | estável | `avoid_redundant_argument_values.dart` |  |
| 34 | `avoid_relative_lib_imports` | estável | `avoid_relative_lib_imports.dart` | core |
| 35 | `avoid_renaming_method_parameters` | estável | `avoid_renaming_method_parameters.dart` | recommended |
| 36 | `avoid_return_types_on_setters` | estável | `avoid_return_types_on_setters.dart` | recommended |
| 37 | `avoid_returning_null` | removida (desde dart3_3) | `avoid_returning_null.dart` |  |
| 38 | `avoid_returning_null_for_future` | removida (desde dart3_3) | `avoid_returning_null_for_future.dart` |  |
| 39 | `avoid_returning_null_for_void` | estável | `avoid_returning_null_for_void.dart` | recommended |
| 40 | `avoid_returning_this` | estável | `avoid_returning_this.dart` |  |
| 41 | `avoid_setters_without_getters` | estável | `avoid_setters_without_getters.dart` |  |
| 42 | `avoid_shadowing_type_parameters` | estável | `avoid_shadowing_type_parameters.dart` | core |
| 43 | `avoid_single_cascade_in_expression_statements` | estável | `avoid_single_cascade_in_expression_statements.dart` | recommended |
| 44 | `avoid_slow_async_io` | estável | `avoid_slow_async_io.dart` |  |
| 45 | `avoid_type_to_string` | estável | `avoid_type_to_string.dart` |  |
| 46 | `avoid_types_as_parameter_names` | estável | `avoid_types_as_parameter_names.dart` | core |
| 47 | `avoid_types_on_closure_parameters` | estável | `avoid_types_on_closure_parameters.dart` |  |
| 48 | `avoid_unnecessary_containers` | estável | `avoid_unnecessary_containers.dart` |  |
| 49 | `avoid_unstable_final_fields` | removida | `avoid_unstable_final_fields.dart` |  |
| 50 | `avoid_unused_constructor_parameters` | estável | `avoid_unused_constructor_parameters.dart` |  |
| 51 | `avoid_void_async` | estável | `avoid_void_async.dart` |  |
| 52 | `avoid_web_libraries_in_flutter` | estável | `avoid_web_libraries_in_flutter.dart` |  |
| 53 | `await_only_futures` | estável | `await_only_futures.dart` | core |
| 54 | `camel_case_extensions` | estável | `camel_case_extensions.dart` | core |
| 55 | `camel_case_types` | estável | `camel_case_types.dart` | core |
| 56 | `cancel_subscriptions` | estável | `cancel_subscriptions.dart` |  |
| 57 | `cascade_invocations` | estável | `cascade_invocations.dart` |  |
| 58 | `cast_nullable_to_non_nullable` | estável | `cast_nullable_to_non_nullable.dart` |  |
| 59 | `close_sinks` | estável | `close_sinks.dart` |  |
| 60 | `collection_methods_unrelated_type` | estável | `collection_methods_unrelated_type.dart` | core |
| 61 | `combinators_ordering` | estável | `combinators_ordering.dart` |  |
| 62 | `comment_references` | estável | `comment_references.dart` |  |
| 63 | `conditional_uri_does_not_exist` | estável | `conditional_uri_does_not_exist.dart` |  |
| 64 | `constant_identifier_names` | estável | `constant_identifier_names.dart` | recommended |
| 65 | `control_flow_in_finally` | estável | `control_flow_in_finally.dart` | recommended |
| 66 | `curly_braces_in_flow_control_structures` | estável | `curly_braces_in_flow_control_structures.dart` | core |
| 67 | `dangling_library_doc_comments` | estável | `dangling_library_doc_comments.dart` | core |
| 68 | `depend_on_referenced_packages` | estável | `pub/depend_on_referenced_packages.dart` | core |
| 69 | `deprecated_consistency` | estável | `deprecated_consistency.dart` |  |
| 70 | `deprecated_member_use_from_same_package` | estável | `deprecated_member_use_from_same_package.dart` |  |
| 71 | `diagnostic_describe_all_properties` | estável | `diagnostic_describe_all_properties.dart` |  |
| 72 | `directives_ordering` | estável | `directives_ordering.dart` |  |
| 73 | `discarded_futures` | estável | `discarded_futures.dart` |  |
| 74 | `document_ignores` | estável | `document_ignores.dart` |  |
| 75 | `do_not_use_environment` | estável | `do_not_use_environment.dart` |  |
| 76 | `empty_catches` | estável | `empty_catches.dart` | core |
| 77 | `empty_constructor_bodies` | estável | `empty_constructor_bodies.dart` | recommended |
| 78 | `empty_statements` | estável | `empty_statements.dart` | recommended |
| 79 | `enable_null_safety` | removida (desde dart3) | `enable_null_safety.dart` |  |
| 80 | `eol_at_end_of_file` | estável | `eol_at_end_of_file.dart` |  |
| 81 | `erase_dart_type_extension_types` | interna | `erase_dart_type_extension_types.dart` |  |
| 82 | `exhaustive_cases` | estável | `exhaustive_cases.dart` | recommended |
| 83 | `file_names` | estável | `file_names.dart` | core |
| 84 | `flutter_style_todos` | estável | `flutter_style_todos.dart` |  |
| 85 | `hash_and_equals` | estável | `hash_and_equals.dart` | core |
| 86 | `implementation_imports` | estável | `implementation_imports.dart` | recommended |
| 87 | `implicit_call_tearoffs` | estável | `implicit_call_tearoffs.dart` | core |
| 88 | `implicit_reopen` | experimental | `implicit_reopen.dart` |  |
| 89 | `invalid_case_patterns` | experimental | `invalid_case_patterns.dart` |  |
| 90 | `invariant_booleans` | removida (desde dart3) | `invariant_booleans.dart` |  |
| 91 | `iterable_contains_unrelated_type` | removida (desde dart3_3) | `iterable_contains_unrelated_type.dart` |  |
| 92 | `invalid_runtime_check_with_js_interop_types` | estável | `invalid_runtime_check_with_js_interop_types.dart` | recommended |
| 93 | `join_return_with_assignment` | estável | `join_return_with_assignment.dart` |  |
| 94 | `leading_newlines_in_multiline_strings` | estável | `leading_newlines_in_multiline_strings.dart` |  |
| 95 | `library_annotations` | estável | `library_annotations.dart` | core |
| 96 | `library_names` | estável | `library_names.dart` |  |
| 97 | `library_prefixes` | estável | `library_prefixes.dart` | recommended |
| 98 | `library_private_types_in_public_api` | estável | `library_private_types_in_public_api.dart` | recommended |
| 99 | `lines_longer_than_80_chars` | estável | `lines_longer_than_80_chars.dart` |  |
| 100 | `list_remove_unrelated_type` | removida (desde dart3_3) | `list_remove_unrelated_type.dart` |  |
| 101 | `literal_only_boolean_expressions` | estável | `literal_only_boolean_expressions.dart` |  |
| 102 | `matching_super_parameters` | estável | `matching_super_parameters.dart` |  |
| 103 | `missing_code_block_language_in_doc_comment` | estável | `missing_code_block_language_in_doc_comment.dart` |  |
| 104 | `missing_whitespace_between_adjacent_strings` | estável | `missing_whitespace_between_adjacent_strings.dart` |  |
| 105 | `no_adjacent_strings_in_list` | estável | `no_adjacent_strings_in_list.dart` |  |
| 106 | `no_default_cases` | experimental | `no_default_cases.dart` |  |
| 107 | `no_duplicate_case_values` | estável | `no_duplicate_case_values.dart` | core |
| 108 | `no_leading_underscores_for_library_prefixes` | estável | `no_leading_underscores_for_library_prefixes.dart` | recommended |
| 109 | `no_leading_underscores_for_local_identifiers` | estável | `no_leading_underscores_for_local_identifiers.dart` | recommended |
| 110 | `no_literal_bool_comparisons` | estável | `no_literal_bool_comparisons.dart` |  |
| 111 | `no_logic_in_create_state` | estável | `no_logic_in_create_state.dart` |  |
| 112 | `no_runtimeType_toString` | estável | `no_runtimeType_toString.dart` |  |
| 113 | `no_self_assignments` | estável | `no_self_assignments.dart` |  |
| 114 | `no_wildcard_variable_uses` | estável | `no_wildcard_variable_uses.dart` | core |
| 115 | `non_constant_identifier_names` | estável | `non_constant_identifier_names.dart` | core |
| 116 | `noop_primitive_operations` | estável | `noop_primitive_operations.dart` |  |
| 117 | `null_check_on_nullable_type_parameter` | estável | `null_check_on_nullable_type_parameter.dart` | core |
| 118 | `null_closures` | estável | `null_closures.dart` | recommended |
| 119 | `omit_local_variable_types` | estável | `omit_local_variable_types.dart` |  |
| 120 | `omit_obvious_local_variable_types` | experimental | `omit_obvious_local_variable_types.dart` |  |
| 121 | `one_member_abstracts` | estável | `one_member_abstracts.dart` |  |
| 122 | `only_throw_errors` | estável | `only_throw_errors.dart` |  |
| 123 | `overridden_fields` | estável | `overridden_fields.dart` | recommended |
| 124 | `package_api_docs` | estável | `package_api_docs.dart` |  |
| 125 | `package_names` | estável | `pub/package_names.dart` | recommended |
| 126 | `package_prefixed_library_names` | estável | `package_prefixed_library_names.dart` |  |
| 127 | `parameter_assignments` | estável | `parameter_assignments.dart` |  |
| 128 | `prefer_adjacent_string_concatenation` | estável | `prefer_adjacent_string_concatenation.dart` | recommended |
| 129 | `prefer_asserts_in_initializer_lists` | estável | `prefer_asserts_in_initializer_lists.dart` |  |
| 130 | `prefer_asserts_with_message` | estável | `prefer_asserts_with_message.dart` |  |
| 131 | `prefer_bool_in_asserts` | removida (desde dart3) | `prefer_bool_in_asserts.dart` |  |
| 132 | `prefer_collection_literals` | estável | `prefer_collection_literals.dart` | recommended |
| 133 | `prefer_conditional_assignment` | estável | `prefer_conditional_assignment.dart` | recommended |
| 134 | `prefer_const_constructors` | estável | `prefer_const_constructors.dart` |  |
| 135 | `prefer_const_constructors_in_immutables` | estável | `prefer_const_constructors_in_immutables.dart` |  |
| 136 | `prefer_const_declarations` | estável | `prefer_const_declarations.dart` |  |
| 137 | `prefer_const_literals_to_create_immutables` | estável | `prefer_const_literals_to_create_immutables.dart` |  |
| 138 | `prefer_constructors_over_static_methods` | estável | `prefer_constructors_over_static_methods.dart` |  |
| 139 | `prefer_contains` | estável | `prefer_contains.dart` | recommended |
| 140 | `prefer_double_quotes` | estável | `prefer_double_quotes.dart` |  |
| 141 | `prefer_equal_for_default_values` | removida (desde dart3) | `prefer_equal_for_default_values.dart` |  |
| 142 | `prefer_expression_function_bodies` | estável | `prefer_expression_function_bodies.dart` |  |
| 143 | `prefer_final_fields` | estável | `prefer_final_fields.dart` | recommended |
| 144 | `prefer_final_in_for_each` | estável | `prefer_final_in_for_each.dart` |  |
| 145 | `prefer_final_locals` | estável | `prefer_final_locals.dart` |  |
| 146 | `prefer_final_parameters` | estável | `prefer_final_parameters.dart` |  |
| 147 | `prefer_for_elements_to_map_fromIterable` | estável | `prefer_for_elements_to_map_fromIterable.dart` | recommended |
| 148 | `prefer_foreach` | estável | `prefer_foreach.dart` |  |
| 149 | `prefer_function_declarations_over_variables` | estável | `prefer_function_declarations_over_variables.dart` | recommended |
| 150 | `prefer_generic_function_type_aliases` | estável | `prefer_generic_function_type_aliases.dart` | core |
| 151 | `prefer_if_elements_to_conditional_expressions` | estável | `prefer_if_elements_to_conditional_expressions.dart` |  |
| 152 | `prefer_if_null_operators` | estável | `prefer_if_null_operators.dart` | recommended |
| 153 | `prefer_initializing_formals` | estável | `prefer_initializing_formals.dart` | recommended |
| 154 | `prefer_inlined_adds` | estável | `prefer_inlined_adds.dart` | recommended |
| 155 | `prefer_int_literals` | estável | `prefer_int_literals.dart` |  |
| 156 | `prefer_interpolation_to_compose_strings` | estável | `prefer_interpolation_to_compose_strings.dart` | recommended |
| 157 | `prefer_is_empty` | estável | `prefer_is_empty.dart` | core |
| 158 | `prefer_is_not_empty` | estável | `prefer_is_not_empty.dart` | core |
| 159 | `prefer_is_not_operator` | estável | `prefer_is_not_operator.dart` | recommended |
| 160 | `prefer_iterable_whereType` | estável | `prefer_iterable_whereType.dart` | core |
| 161 | `prefer_mixin` | estável | `prefer_mixin.dart` |  |
| 162 | `prefer_null_aware_method_calls` | estável | `prefer_null_aware_method_calls.dart` |  |
| 163 | `prefer_null_aware_operators` | estável | `prefer_null_aware_operators.dart` | recommended |
| 164 | `prefer_relative_imports` | estável | `prefer_relative_imports.dart` |  |
| 165 | `prefer_single_quotes` | estável | `prefer_single_quotes.dart` |  |
| 166 | `prefer_spread_collections` | estável | `prefer_spread_collections.dart` | recommended |
| 167 | `prefer_typing_uninitialized_variables` | estável | `prefer_typing_uninitialized_variables.dart` | core |
| 168 | `prefer_void_to_null` | estável | `prefer_void_to_null.dart` |  |
| 169 | `provide_deprecation_message` | estável | `provide_deprecation_message.dart` | core |
| 170 | `public_member_api_docs` | estável | `public_member_api_docs.dart` |  |
| 171 | `recursive_getters` | estável | `recursive_getters.dart` | recommended |
| 172 | `require_trailing_commas` | estável | `require_trailing_commas.dart` |  |
| 173 | `secure_pubspec_urls` | estável | `pub/secure_pubspec_urls.dart` | core |
| 174 | `sized_box_for_whitespace` | estável | `sized_box_for_whitespace.dart` |  |
| 175 | `sized_box_shrink_expand` | estável | `sized_box_shrink_expand.dart` |  |
| 176 | `slash_for_doc_comments` | estável | `slash_for_doc_comments.dart` | recommended |
| 177 | `sort_child_properties_last` | estável | `sort_child_properties_last.dart` |  |
| 178 | `sort_constructors_first` | estável | `sort_constructors_first.dart` |  |
| 179 | `sort_pub_dependencies` | estável | `pub/sort_pub_dependencies.dart` |  |
| 180 | `sort_unnamed_constructors_first` | estável | `sort_unnamed_constructors_first.dart` |  |
| 181 | `super_goes_last` | removida (desde dart3) | `super_goes_last.dart` |  |
| 182 | `specify_nonobvious_local_variable_types` | experimental | `specify_nonobvious_local_variable_types.dart` |  |
| 183 | `test_types_in_equals` | estável | `test_types_in_equals.dart` |  |
| 184 | `throw_in_finally` | estável | `throw_in_finally.dart` |  |
| 185 | `tighten_type_of_initializing_formals` | estável | `tighten_type_of_initializing_formals.dart` |  |
| 186 | `type_annotate_public_apis` | estável | `type_annotate_public_apis.dart` |  |
| 187 | `type_init_formals` | estável | `type_init_formals.dart` | recommended |
| 188 | `type_literal_in_constant_pattern` | estável | `type_literal_in_constant_pattern.dart` | core |
| 189 | `unawaited_futures` | estável | `unawaited_futures.dart` |  |
| 190 | `unintended_html_in_doc_comment` | estável | `unintended_html_in_doc_comment.dart` | core |
| 191 | `unnecessary_await_in_return` | estável | `unnecessary_await_in_return.dart` |  |
| 192 | `unnecessary_brace_in_string_interps` | estável | `unnecessary_brace_in_string_interps.dart` | recommended |
| 193 | `unnecessary_breaks` | estável | `unnecessary_breaks.dart` |  |
| 194 | `unnecessary_const` | estável | `unnecessary_const.dart` | recommended |
| 195 | `unnecessary_constructor_name` | estável | `unnecessary_constructor_name.dart` | recommended |
| 196 | `unnecessary_final` | estável | `unnecessary_final.dart` |  |
| 197 | `unnecessary_getters_setters` | estável | `unnecessary_getters_setters.dart` | recommended |
| 198 | `unnecessary_lambdas` | estável | `unnecessary_lambdas.dart` |  |
| 199 | `unnecessary_late` | estável | `unnecessary_late.dart` | recommended |
| 200 | `unnecessary_library_directive` | estável | `unnecessary_library_directive.dart` |  |
| 201 | `unnecessary_library_name` | estável | `unnecessary_library_name.dart` | recommended |
| 202 | `unnecessary_new` | estável | `unnecessary_new.dart` | recommended |
| 203 | `unnecessary_null_aware_assignments` | estável | `unnecessary_null_aware_assignments.dart` | recommended |
| 204 | `unnecessary_null_aware_operator_on_extension_on_nullable` | estável | `unnecessary_null_aware_operator_on_extension_on_nullable.dart` |  |
| 205 | `unnecessary_null_checks` | experimental | `unnecessary_null_checks.dart` |  |
| 206 | `unnecessary_null_in_if_null_operators` | estável | `unnecessary_null_in_if_null_operators.dart` | recommended |
| 207 | `unnecessary_nullable_for_final_variable_declarations` | estável | `unnecessary_nullable_for_final_variable_declarations.dart` | recommended |
| 208 | `unnecessary_overrides` | estável | `unnecessary_overrides.dart` | core |
| 209 | `unnecessary_parenthesis` | estável | `unnecessary_parenthesis.dart` |  |
| 210 | `unnecessary_raw_strings` | estável | `unnecessary_raw_strings.dart` |  |
| 211 | `unnecessary_statements` | estável | `unnecessary_statements.dart` |  |
| 212 | `unnecessary_string_escapes` | estável | `unnecessary_string_escapes.dart` | recommended |
| 213 | `unnecessary_string_interpolations` | estável | `unnecessary_string_interpolations.dart` | recommended |
| 214 | `unnecessary_this` | estável | `unnecessary_this.dart` | recommended |
| 215 | `unnecessary_to_list_in_spreads` | estável | `unnecessary_to_list_in_spreads.dart` | recommended |
| 216 | `unreachable_from_main` | estável (desde Version(3, 1, 0) | `unreachable_from_main.dart` |  |
| 217 | `unrelated_type_equality_checks` | estável | `unrelated_type_equality_checks.dart` | core |
| 218 | `unsafe_html` | estável | `unsafe_html.dart` |  |
| 219 | `use_build_context_synchronously` | estável (desde Version(3, 2, 0) | `use_build_context_synchronously.dart` |  |
| 220 | `use_colored_box` | estável | `use_colored_box.dart` |  |
| 221 | `use_decorated_box` | estável | `use_decorated_box.dart` |  |
| 222 | `use_enums` | estável | `use_enums.dart` |  |
| 223 | `use_full_hex_values_for_flutter_colors` | estável | `use_full_hex_values_for_flutter_colors.dart` |  |
| 224 | `use_function_type_syntax_for_parameters` | estável | `use_function_type_syntax_for_parameters.dart` | recommended |
| 225 | `use_if_null_to_convert_nulls_to_bools` | estável | `use_if_null_to_convert_nulls_to_bools.dart` |  |
| 226 | `use_is_even_rather_than_modulo` | estável | `use_is_even_rather_than_modulo.dart` |  |
| 227 | `use_key_in_widget_constructors` | estável | `use_key_in_widget_constructors.dart` |  |
| 228 | `use_late_for_private_fields_and_variables` | experimental | `use_late_for_private_fields_and_variables.dart` |  |
| 229 | `use_named_constants` | estável | `use_named_constants.dart` |  |
| 230 | `use_raw_strings` | estável | `use_raw_strings.dart` |  |
| 231 | `use_rethrow_when_possible` | estável | `use_rethrow_when_possible.dart` | recommended |
| 232 | `use_setters_to_change_properties` | estável | `use_setters_to_change_properties.dart` |  |
| 233 | `use_string_buffers` | estável | `use_string_buffers.dart` |  |
| 234 | `use_string_in_part_of_directives` | estável | `use_string_in_part_of_directives.dart` | core |
| 235 | `use_super_parameters` | experimental | `use_super_parameters.dart` | recommended |
| 236 | `use_test_throws_matchers` | estável | `use_test_throws_matchers.dart` |  |
| 237 | `use_to_and_as_if_applicable` | estável | `use_to_and_as_if_applicable.dart` |  |
| 238 | `use_truncating_division` | estável | `use_truncating_division.dart` |  |
| 239 | `valid_regexps` | estável | `valid_regexps.dart` | core |
| 240 | `void_checks` | estável | `void_checks.dart` | core |

### 8.6 No DartForge

- Nenhum lint, nenhum registro, nenhuma leitura de `linter:` nem de `include:`
  (`crates/paridade/src/filtros.rs:12-13` diz que o include "só traria lints,
  que são o passo A4" do plano — `docs/ANALISADOR-PARIDADE-PLANO.md`).
- Consequência para a paridade de um projeto real: todo projeto criado por
  `dart create` inclui `package:lints/recommended.yaml`; o `dart analyze`
  dele emite `info` de lint (e `--fatal-infos` os torna fatais). Enquanto não
  houver lints, a saída do `dartforge analyze` **não pode** ser idêntica nesses
  projetos, a não ser nos que não disparam nenhuma das 89 regras. O critério
  realista (Parte III, etapa 8): paridade em projetos **sem** `linter:`/`include`
  primeiro; depois as regras `core`, depois `recommended`, por frequência.
- O que precisa existir antes de qualquer regra: `include:` com resolução de
  `package:` (§4.6), `Registry.enabled`, e a validação da lista de regras
  (`undefined_lint`, `duplicate_rule`, `incompatible_lint`, `removed_lint`:
  II.10) — porque essas mensagens saem para **qualquer** projeto com `linter:`,
  mesmo sem nenhuma regra implementada. Para isso basta a **tabela de nomes e
  estados** desta seção.

---

# Parte II — Catálogo dos códigos fora dos 436

São **546 constantes** de diagnóstico da tabela oficial do 3.6.2 que não estão nas listas `familia-*.txt` do documento por código, em dez lotes. Três lotes (II.5 em parte, II.8 e II.10) foram escritos lendo cada método emissor; os demais foram gerados por script a partir do levantamento automático e estão marcados como tal: neles a condição é a mensagem oficial e a posição não foi conferida.

Estado no DartForge, por lote (contagem do script `final/gera2.py`: emissor = referência à constante em `crates/` fora de `diagnostics`, `paridade` e `lsp`; publicado = código de sintaxe ou nome em `crates/analise/verificados.txt`):

| lote | conteúdo | constantes | publicadas | emitidas, não publicadas | não implementadas | origem |
|---|---|---:|---:|---:|---:|---|
| II.1 | parser e scanner (a) | 56 | 32 | 0 | 24 | script |
| II.2 | parser e scanner (b) | 56 | 35 | 0 | 21 | script |
| II.3 | conversor do fasta e `AstBuilder` | 75 | 39 | 0 | 36 | script |
| II.4 | sem emissor localizado e restos do parser | 59 | 2 | 0 | 57 | script |
| II.5 | `ErrorVerifier` | 75 | 50 | 0 | 25 | lido (47) + script (28) |
| II.6 | constantes, herança, sobrescrita | 71 | 40 | 16 | 15 | script |
| II.7 | `BestPracticesVerifier`, anotações, documentação | 34 | 0 | 0 | 34 | script |
| II.8 | verificadores de aviso | 31 | 3 | 1 | 27 | lido |
| II.9 | resolução, inferência, FFI | 36 | 12 | 0 | 24 | script |
| II.10 | arquivos não-Dart | 53 | 0 | 0 | 53 | lido |
| **total** | | **546** | **213** | **17** | **316** | |

Os lotes escritos à mão trazem as próprias contagens no "Resumo do lote"; onde diferirem em uma ou duas unidades desta tabela, vale a do lote (o script conta por referência textual à constante, o lote conferiu o emissor).

### II.1 — Parser e scanner (lote a)

Lote `P1a`: **56 constantes** (32 publicadas, 0 emitidas e não publicadas, 24 não implementadas no DartForge).
Esta tabela foi **gerada por script** (`E:\dftempnalise\spec-infrainal\gera2.py`) a partir do levantamento automático (`catalogo.json`: definição, sítios de emissão por grep com o método que os contém, referências em `crates/`). A coluna "condição" traz a **mensagem oficial** do código (o texto do `messages.yaml`), não a condição lida no método emissor; a coluna "posição" está **não verificada** em todas as linhas. Antes de implementar um código deste lote, abra o emissor citado e escreva os seis campos (o molde está em `docs/ANALYZER-ESPECIFICACAO.md`).

#### `parser fasta` (53)

| código | constante | emissor (arquivo:linha, método) | condição (mensagem oficial) | posição | DartForge |
|---|---|---|---|---|---|
| `abstract_class_member` | `ParserErrorCode.ABSTRACT_CLASS_MEMBER` | fasta `AbstractClassMember`: `_fe_analyzer_shared/lib/src/parser/modifier_context.dart:271` `ModifierContext.parseModifiersAfterFactory`; fasta `AbstractClassMember`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:4625` `Parser.parseClassOrMixinOrExtensionOrEnumMemberImpl`; fasta `AbstractClassMember`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:4703` `Parser.parseClassOrMixinOrExtensionOrEnumMemberImpl`; fasta `AbstractClassMember`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:4824` `Parser.parseMethod` | Members of classes can't be declared to be 'abstract'. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:2258`, `crates/frontend/src/parser/declarations.rs:2675` |
| `abstract_external_field` | `ParserErrorCode.ABSTRACT_EXTERNAL_FIELD` | fasta `AbstractExternalField`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:3693` `Parser.parseFields` | Fields can't be declared both 'abstract' and 'external'. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:2315` |
| `abstract_final_base_class` | `ParserErrorCode.ABSTRACT_FINAL_BASE_CLASS` | fasta `AbstractFinalBaseClass`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:2649` `Parser.parseClassOrNamedMixinApplication` | An 'abstract' class can't be declared as both 'final' and 'base'. | não verificado | **não implementado** |
| `abstract_final_interface_class` | `ParserErrorCode.ABSTRACT_FINAL_INTERFACE_CLASS` | fasta `AbstractFinalInterfaceClass`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:2652` `Parser.parseClassOrNamedMixinApplication` | An 'abstract' class can't be declared as both 'final' and 'interface'. | não verificado | **não implementado** |
| `abstract_sealed_class` | `ParserErrorCode.ABSTRACT_SEALED_CLASS` | fasta `AbstractSealedClass`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:2645` `Parser.parseClassOrNamedMixinApplication` | A 'sealed' class can't be marked 'abstract' because it's already implicitly abstract. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:987` |
| `annotation_space_before_parenthesis` | `ParserErrorCode.ANNOTATION_SPACE_BEFORE_PARENTHESIS` | fasta `MetadataSpaceBeforeParenthesis`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:7732` `Parser.parseArgumentsOptMetadata`; fasta `MetadataSpaceBeforeParenthesis`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:7745` `Parser.parseArgumentsOptMetadata` | Annotations can't have spaces or comments before the parenthesis. | não verificado | **não implementado** |
| `base_enum` | `ParserErrorCode.BASE_ENUM` | fasta `BaseEnum`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:686` `Parser.parseTopLevelKeywordDeclaration` | Enums can't be declared to be 'base'. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:903` |
| `binary_operator_written_out` | `ParserErrorCode.BINARY_OPERATOR_WRITTEN_OUT` | fasta `BinaryOperatorWrittenOut`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:6218` `Parser._attemptPrecedenceLevelRecovery` | Binary operator '{0}' is written as '{1}' instead of the written out word. | não verificado | **publicado** — `crates/frontend/src/parser/expressions.rs:590` |
| `catch_syntax` | `ParserErrorCode.CATCH_SYNTAX` | fasta `CatchSyntax`: `_fe_analyzer_shared/lib/src/parser/identifier_context_impl.dart:33` `CatchParameterIdentifierContext.ensureIdentifier`; fasta `CatchSyntax`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:8855` `Parser.parseTryStatement`; fasta `CatchSyntax`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:8873` `Parser.parseTryStatement` | 'catch' must be followed by '(identifier)' or '(identifier, identifier)'. | não verificado | **não implementado** |
| `class_in_class` | `ParserErrorCode.CLASS_IN_CLASS` | fasta `ClassInClass`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:9484` `Parser.reportAndSkipClassInClass` | Classes can't be declared inside other classes. | não verificado | **não implementado** |
| `colon_in_place_of_in` | `ParserErrorCode.COLON_IN_PLACE_OF_IN` | fasta `ColonInPlaceOfIn`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:8441` `Parser.parseForLoopPartsMid` | For-in loops use 'in' rather than a colon. | não verificado | **não implementado** |
| `const_method` | `ParserErrorCode.CONST_METHOD` | fasta `ConstMethod`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:5065` `Parser.parseMethod` | Getters, setters and methods can't be declared to be 'const'. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:2275` |
| `constructor_with_type_arguments` | `ParserErrorCode.CONSTRUCTOR_WITH_TYPE_ARGUMENTS` | fasta `ConstructorWithTypeArguments`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:7229` `Parser.parseConstructorInvocationArguments` | A constructor invocation can't have type arguments after the constructor name. | não verificado | **publicado** — `crates/frontend/src/parser/expressions.rs:1628` |
| `covariant_member` | `ParserErrorCode.COVARIANT_MEMBER` | fasta `CovariantMember`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:4857` `Parser.parseMethod` | Getters, setters and methods can't be declared to be 'covariant'. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:2269` |
| `default_in_switch_expression` | `ParserErrorCode.DEFAULT_IN_SWITCH_EXPRESSION` | fasta `DefaultInSwitchExpression`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:10372` `Parser.parseSwitchExpression` | A switch expression may not use the `default` keyword. | não verificado | **publicado** — `crates/frontend/src/parser/expressions.rs:1669` |
| `deferred_after_prefix` | `ParserErrorCode.DEFERRED_AFTER_PREFIX` | fasta `DeferredAfterPrefix`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:1009` `Parser.parseImportRecovery` | The deferred keyword should come immediately before the prefix ('as' clause). | não verificado | **não implementado** |
| `duplicate_deferred` | `ParserErrorCode.DUPLICATE_DEFERRED` | fasta `DuplicateDeferred`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:1005` `Parser.parseImportRecovery` | An import directive can only have one 'deferred' keyword. | não verificado | **não implementado** |
| `duplicate_prefix` | `ParserErrorCode.DUPLICATE_PREFIX` | fasta `DuplicatePrefix`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:1017` `Parser.parseImportRecovery` | An import directive can only have one prefix ('as' clause). | não verificado | **não implementado** |
| `empty_record_literal_with_comma` | `ParserErrorCode.EMPTY_RECORD_LITERAL_WITH_COMMA` | fasta `RecordLiteralZeroFieldsWithTrailingComma`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:6809` `Parser.parseParenthesizedExpressionOrRecordLiteral` | A record literal without fields can't have a trailing comma. | não verificado | **não implementado** |
| `empty_record_type_with_comma` | `ParserErrorCode.EMPTY_RECORD_TYPE_WITH_COMMA` | fasta `RecordTypeZeroFieldsButTrailingComma`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:1676` `Parser.parseRecordType` | A record type without fields can't have a trailing comma. | não verificado | **não implementado** |
| `enum_in_class` | `ParserErrorCode.ENUM_IN_CLASS` | fasta `EnumInClass`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:9507` `Parser.reportAndSkipEnumInClass` | Enums can't be declared inside classes. | não verificado | **não implementado** |
| `equality_cannot_be_equality_operand` | `ParserErrorCode.EQUALITY_CANNOT_BE_EQUALITY_OPERAND` | fasta `EqualityCannotBeEqualityOperand`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:6084` `Parser._parsePrecedenceExpressionLoop` | A comparison expression can't be an operand of another comparison expression. | não verificado | **publicado** — `crates/frontend/src/parser/expressions.rs:608`, `crates/frontend/src/parser/expressions.rs:2570` |
| `expected_else_or_comma` | `ParserErrorCode.EXPECTED_ELSE_OR_COMMA` | fasta `ExpectedElseOrComma`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:6977` `Parser.parseLiteralListSuffix`; fasta `ExpectedElseOrComma`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:7083` `Parser.parseLiteralSetOrMapSuffix` | Expected 'else' or comma. | não verificado | **não implementado** |
| `expected_instead` | `ParserErrorCode.EXPECTED_INSTEAD` | fasta `ExpectedInstead`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:2796` `Parser.parseDeclarationHeaderRecoveryInternal`; fasta `ExpectedInstead`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:3002` `Parser.parseMixinHeaderRecovery`; fasta `ExpectedInstead`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:3145` `Parser.parseExtensionDeclaration` | Expected '{0}' instead of this. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:1583` |
| `extension_augmentation_has_on_clause` | `ParserErrorCode.EXTENSION_AUGMENTATION_HAS_ON_CLAUSE` | fasta `ExtensionAugmentationHasOnClause`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:3134` `Parser.parseExtensionDeclaration` | Extension augmentations can't have 'on' clauses. | não verificado | **não implementado** |
| `extension_declares_abstract_member` | `ParserErrorCode.EXTENSION_DECLARES_ABSTRACT_MEMBER` | fasta `ExtensionDeclaresAbstractMember`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:5080` `Parser.parseMethod` | Extensions can't declare abstract members. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:2293` |
| `extension_declares_constructor` | `ParserErrorCode.EXTENSION_DECLARES_CONSTRUCTOR` | fasta `ExtensionDeclaresConstructor`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:5044` `Parser.parseMethod`; fasta `ExtensionDeclaresConstructor`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:5185` `Parser.parseFactoryMethod` | Extensions can't declare constructors. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:2729` |
| `extension_declares_instance_field` | `ParserErrorCode.EXTENSION_DECLARES_INSTANCE_FIELD` | fasta `ExtensionDeclaresInstanceField`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:3808` `Parser.parseFields` | Extensions can't declare instance fields | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:2322` |
| `extension_type_extends` | `ParserErrorCode.EXTENSION_TYPE_EXTENDS` | fasta `ExtensionTypeExtends`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:2820` `Parser.parseDeclarationHeaderRecoveryInternal` | An extension type declaration can't have an 'extends' clause. | não verificado | **não implementado** |
| `extension_type_with` | `ParserErrorCode.EXTENSION_TYPE_WITH` | fasta `ExtensionTypeWith`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:2841` `Parser.parseDeclarationHeaderRecoveryInternal` | An extension type declaration can't have a 'with' clause. | não verificado | **não implementado** |
| `external_constructor_with_initializer` | `ParserErrorCode.EXTERNAL_CONSTRUCTOR_WITH_INITIALIZER` | fasta `ExternalConstructorWithInitializer`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:5028` `Parser.parseMethod` | An external constructor can't have any initializers. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:2864` |
| `external_factory_with_body` | `ParserErrorCode.EXTERNAL_FACTORY_WITH_BODY` | fasta `ExternalFactoryWithBody`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:5153` `Parser.parseFactoryMethod` | External factories can't have a body. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:3059`, `crates/frontend/src/parser/declarations.rs:3327` |
| `external_method_with_body` | `ParserErrorCode.EXTERNAL_METHOD_WITH_BODY` | fasta `ExternalMethodWithBody`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:3893` `Parser.parseTopLevelMethod`; fasta `ExternalMethodWithBody`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:4975` `Parser.parseMethod`; fasta `ExternalMethodWithBody`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:5426` `Parser.parseFunctionBody` | An external or native method can't have a body. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:3061`, `crates/frontend/src/parser/declarations.rs:3325` |
| `factory_top_level_declaration` | `ParserErrorCode.FACTORY_TOP_LEVEL_DECLARATION` | fasta `FactoryTopLevelDeclaration`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:3572` `Parser.parseTopLevelMemberImpl` | Top-level declarations can't be declared to be 'factory'. | não verificado | **não implementado** |
| `field_initialized_outside_declaring_class` | `ParserErrorCode.FIELD_INITIALIZED_OUTSIDE_DECLARING_CLASS` | fasta `FieldInitializedOutsideDeclaringClass`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:4179` `Parser.parseSuperInitializerExpression` | A field can only be initialized in its declaring class | não verificado | **não implementado** |
| `final_and_covariant` | `ParserErrorCode.FINAL_AND_COVARIANT` | fasta `FinalAndCovariant`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:3679` `Parser.parseFields` | Members can't be declared to be both 'final' and 'covariant'. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:2309` |
| `final_and_covariant_late_with_initializer` | `ParserErrorCode.FINAL_AND_COVARIANT_LATE_WITH_INITIALIZER` | fasta `FinalAndCovariantLateWithInitializer`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:3711` `Parser.parseFields` | Members marked 'late' with an initializer can't be declared to be both 'final' and 'covariant'. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:2311` |
| `final_enum` | `ParserErrorCode.FINAL_ENUM` | fasta `FinalEnum`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:689` `Parser.parseTopLevelKeywordDeclaration` | Enums can't be declared to be 'final'. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:879` |
| `final_mixin` | `ParserErrorCode.FINAL_MIXIN` | fasta `FinalMixin`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:768` `Parser.parseTopLevelKeywordDeclaration` | A mixin can't be declared 'final'. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:874` |
| `final_mixin_class` | `ParserErrorCode.FINAL_MIXIN_CLASS` | fasta `FinalMixinClass`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:820` `Parser._handleModifiersForClassDeclaration` | A mixin class can't be declared 'final'. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:869` |
| `function_typed_parameter_var` | `ParserErrorCode.FUNCTION_TYPED_PARAMETER_VAR` | fasta `FunctionTypedParameterVar`: `_fe_analyzer_shared/lib/src/parser/modifier_context.dart:240` `ModifierContext.parseFormalParameterModifiers`; fasta `FunctionTypedParameterVar`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:2094` `Parser.parseFormalParameter`; fasta `FunctionTypedParameterVar`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:2104` `Parser.parseFormalParameter` | Function-typed parameters can't specify 'const', 'final' or 'var' in place of a return type. | não verificado | **publicado** — `crates/frontend/src/parser/modificadores.rs:466` |
| `getter_constructor` | `ParserErrorCode.GETTER_CONSTRUCTOR` | fasta `GetterConstructor`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:5017` `Parser.parseMethod` | Constructors can't be a getter. | não verificado | **não implementado** |
| `illegal_pattern_assignment_variable_name` | `ParserErrorCode.ILLEGAL_PATTERN_ASSIGNMENT_VARIABLE_NAME` | fasta `IllegalPatternAssignmentVariableName`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:9937` `Parser.parseVariablePattern` | A variable assigned by a pattern assignment can't be named '{0}'. | não verificado | **publicado** — `crates/frontend/src/parser/patterns.rs:127` |
| `illegal_pattern_identifier_name` | `ParserErrorCode.ILLEGAL_PATTERN_IDENTIFIER_NAME` | fasta `IllegalPatternIdentifierName`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:9849` `Parser.parsePrimaryPattern` | A pattern can't refer to an identifier named '{0}'. | não verificado | **publicado** — `crates/frontend/src/parser/patterns.rs:100` |
| `illegal_pattern_variable_name` | `ParserErrorCode.ILLEGAL_PATTERN_VARIABLE_NAME` | fasta `IllegalPatternVariableName`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:9944` `Parser.parseVariablePattern` | The variable declared by a variable pattern can't be named '{0}'. | não verificado | **publicado** — `crates/frontend/src/parser/patterns.rs:129` |
| `implements_before_extends` | `ParserErrorCode.IMPLEMENTS_BEFORE_EXTENDS` | fasta `ImplementsBeforeExtends`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:2814` `Parser.parseDeclarationHeaderRecoveryInternal` | The extends clause must be before the implements clause. | não verificado | **não implementado** |
| `implements_before_on` | `ParserErrorCode.IMPLEMENTS_BEFORE_ON` | fasta `ImplementsBeforeOn`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:3015` `Parser.parseMixinHeaderRecovery` | The on clause must be before the implements clause. | não verificado | **não implementado** |
| `implements_before_with` | `ParserErrorCode.IMPLEMENTS_BEFORE_WITH` | fasta `ImplementsBeforeWith`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:2835` `Parser.parseDeclarationHeaderRecoveryInternal` | The with clause must be before the implements clause. | não verificado | **não implementado** |
| `initialized_variable_in_for_each` | `ParserErrorCode.INITIALIZED_VARIABLE_IN_FOR_EACH` | fasta `InitializedVariableInForEach`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:8556` `Parser.parseForInLoopPartsRest` | The loop variable in a for-each loop can't be initialized. | não verificado | **não implementado** |
| `interface_enum` | `ParserErrorCode.INTERFACE_ENUM` | fasta `InterfaceEnum`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:692` `Parser.parseTopLevelKeywordDeclaration` | Enums can't be declared to be 'interface'. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:904` |
| `interface_mixin` | `ParserErrorCode.INTERFACE_MIXIN` | fasta `InterfaceMixin`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:771` `Parser.parseTopLevelKeywordDeclaration` | A mixin can't be declared 'interface'. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:911` |
| `interface_mixin_class` | `ParserErrorCode.INTERFACE_MIXIN_CLASS` | fasta `InterfaceMixinClass`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:824` `Parser._handleModifiersForClassDeclaration` | A mixin class can't be declared 'interface'. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:907` |
| `invalid_await_in_for` | `ParserErrorCode.INVALID_AWAIT_IN_FOR` | fasta `InvalidAwaitFor`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:8436` `Parser.parseForLoopPartsMid` | The keyword 'await' isn't allowed for a normal 'for' statement. | não verificado | **não implementado** |

#### `analyzer/lib/src/fasta/error_converter.dart` (3)

| código | constante | emissor (arquivo:linha, método) | condição (mensagem oficial) | posição | DartForge |
|---|---|---|---|---|---|
| `default_value_in_function_type` | `ParserErrorCode.DEFAULT_VALUE_IN_FUNCTION_TYPE` | `analyzer/lib/src/fasta/error_converter.dart:90` `FastaErrorReporter.reportByCode`; fasta `FunctionTypeDefaultValue`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:2189` `Parser.parseFormalParameter` | Parameters in a function type can't have default values. | não verificado | **publicado** — `crates/frontend/src/parser/types.rs:758` |
| `expected_string_literal` | `ParserErrorCode.EXPECTED_STRING_LITERAL` | `analyzer/lib/src/fasta/error_converter.dart:126` `FastaErrorReporter.reportByCode`; fasta `ExpectedString`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:3261` `Parser.parseStringPart`; fasta `ExpectedString`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:4282` `Parser.ensureLiteralString` | Expected a string literal. | não verificado | **publicado** — `crates/frontend/src/parser/expressions.rs:307` |
| `getter_with_parameters` | `ParserErrorCode.GETTER_WITH_PARAMETERS` | `analyzer/lib/src/fasta/error_converter.dart:174` `FastaErrorReporter.reportByCode`; fasta `GetterWithFormals`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:1546` `Parser.parseGetterOrFormalParameters` | Getters must be declared without a parameter list. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:2163` |

### II.2 — Parser e scanner (lote b)

Lote `P1b`: **56 constantes** (35 publicadas, 0 emitidas e não publicadas, 21 não implementadas no DartForge).
Esta tabela foi **gerada por script** (`E:\dftempnalise\spec-infrainal\gera2.py`) a partir do levantamento automático (`catalogo.json`: definição, sítios de emissão por grep com o método que os contém, referências em `crates/`). A coluna "condição" traz a **mensagem oficial** do código (o texto do `messages.yaml`), não a condição lida no método emissor; a coluna "posição" está **não verificada** em todas as linhas. Antes de implementar um código deste lote, abra o emissor citado e escreva os seis campos (o molde está em `docs/ANALYZER-ESPECIFICACAO.md`).

#### `parser fasta` (51)

| código | constante | emissor (arquivo:linha, método) | condição (mensagem oficial) | posição | DartForge |
|---|---|---|---|---|---|
| `invalid_constant_const_prefix` | `ParserErrorCode.INVALID_CONSTANT_CONST_PREFIX` | fasta `InvalidConstantPatternConstPrefix`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:5956` `Parser._parsePrecedenceExpressionLoop`; fasta `InvalidConstantPatternConstPrefix`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:6402` `Parser.parseUnaryExpression`; fasta `InvalidConstantPatternConstPrefix`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:6563` `Parser.parsePrimary`; fasta `InvalidConstantPatternConstPrefix`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:6574` `Parser.parsePrimary` | The expression can't be prefixed by 'const' to form a constant pattern. | não verificado | **publicado** — `crates/frontend/src/parser/patterns.rs:402`, `crates/frontend/src/parser/patterns.rs:455` |
| `invalid_constant_pattern_binary` | `ParserErrorCode.INVALID_CONSTANT_PATTERN_BINARY` | fasta `InvalidConstantPatternBinary`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:5960` `Parser._parsePrecedenceExpressionLoop` | The binary operator {0} is not supported as a constant pattern. | não verificado | **publicado** — `crates/frontend/src/parser/patterns.rs:405` |
| `invalid_constant_pattern_duplicate_const` | `ParserErrorCode.INVALID_CONSTANT_PATTERN_DUPLICATE_CONST` | fasta `InvalidConstantPatternDuplicateConst`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:6635` `Parser.parsePrimary` | Duplicate 'const' keyword in constant expression. | não verificado | **publicado** — `crates/frontend/src/parser/patterns.rs:435` |
| `invalid_constant_pattern_empty_record_literal` | `ParserErrorCode.INVALID_CONSTANT_PATTERN_EMPTY_RECORD_LITERAL` | fasta `InvalidConstantPatternEmptyRecordLiteral`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:6816` `Parser.parseParenthesizedExpressionOrRecordLiteral` | The empty record literal is not supported as a constant pattern. | não verificado | **publicado** — `crates/frontend/src/parser/patterns.rs:425` |
| `invalid_constant_pattern_generic` | `ParserErrorCode.INVALID_CONSTANT_PATTERN_GENERIC` | fasta `InvalidConstantPatternGeneric`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:5910` `Parser.parsePrecedenceExpression`; fasta `InvalidConstantPatternGeneric`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:6044` `Parser._parsePrecedenceExpressionLoop` | This expression is not supported as a constant pattern. | não verificado | **publicado** — `crates/frontend/src/parser/expressions.rs:1037` |
| `invalid_constant_pattern_negation` | `ParserErrorCode.INVALID_CONSTANT_PATTERN_NEGATION` | fasta `InvalidConstantPatternNegation`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:6555` `Parser.parsePrimary`; fasta `InvalidConstantPatternNegation`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:6588` `Parser.parsePrimary`; fasta `InvalidConstantPatternNegation`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:6598` `Parser.parsePrimary`; fasta `InvalidConstantPatternNegation`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:6610` `Parser.parsePrimary` | Only negation of a numeric literal is supported as a constant pattern. | não verificado | **publicado** — `crates/frontend/src/parser/patterns.rs:573`, `crates/frontend/src/parser/patterns.rs:590` |
| `invalid_constant_pattern_unary` | `ParserErrorCode.INVALID_CONSTANT_PATTERN_UNARY` | fasta `InvalidConstantPatternUnary`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:6390` `Parser.parseUnaryExpression` | The unary operator {0} is not supported as a constant pattern. | não verificado | **publicado** — `crates/frontend/src/parser/patterns.rs:519` |
| `invalid_constructor_name` | `ParserErrorCode.INVALID_CONSTRUCTOR_NAME` | fasta `ConstructorWithWrongName`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:5010` `Parser.parseMethod` | The name of a constructor must match the name of the enclosing class. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:2700` |
| `invalid_inside_unary_pattern` | `ParserErrorCode.INVALID_INSIDE_UNARY_PATTERN` | fasta `InvalidInsideUnaryPattern`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:9619` `Parser.parsePattern`; fasta `InvalidInsideUnaryPattern`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:9631` `Parser.parsePattern`; fasta `InvalidInsideUnaryPattern`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:9640` `Parser.parsePattern` | This pattern cannot appear inside a unary pattern (cast pattern, null check pattern, or null assert pattern) without parentheses. | não verificado | **publicado** — `crates/frontend/src/parser/patterns.rs:264` |
| `invalid_operator` | `ParserErrorCode.INVALID_OPERATOR` | fasta `InvalidOperator`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:5232` `Parser.parseOperatorName` | The string '{0}' isn't a user-definable operator. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:1898` |
| `invalid_operator_questionmark_period_for_super` | `ParserErrorCode.INVALID_OPERATOR_QUESTIONMARK_PERIOD_FOR_SUPER` | fasta `SuperNullAware`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:6885` `Parser.parseSuperExpression` | The operator '?.' cannot be used with 'super' because 'super' cannot be null. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:2931`, `crates/frontend/src/parser/expressions.rs:1220` |
| `late_pattern_variable_declaration` | `ParserErrorCode.LATE_PATTERN_VARIABLE_DECLARATION` | fasta `LatePatternVariableDeclaration`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:8096` `Parser.parseExpressionStatementOrDeclarationAfterModifiers` | A pattern variable declaration may not use the `late` keyword. | não verificado | **não implementado** |
| `literal_with_class` | `ParserErrorCode.LITERAL_WITH_CLASS` | fasta `LiteralWithClass`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:7393` `Parser.parseConstExpression`; fasta `LiteralWithClass`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:7405` `Parser.parseConstExpression`; fasta `LiteralWithClass`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:7425` `Parser.parseConstExpression`; fasta `LiteralWithClass`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:7437` `Parser.parseConstExpression` | A {0} literal can't be prefixed by '{1}'. | não verificado | **não implementado** |
| `literal_with_class_and_new` | `ParserErrorCode.LITERAL_WITH_CLASS_AND_NEW` | fasta `LiteralWithClassAndNew`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:7265` `Parser.parseNewExpression`; fasta `LiteralWithClassAndNew`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:7279` `Parser.parseNewExpression` | A {0} literal can't be prefixed by 'new {1}'. | não verificado | **publicado** — `crates/frontend/src/parser/expressions.rs:1269` |
| `literal_with_new` | `ParserErrorCode.LITERAL_WITH_NEW` | fasta `LiteralWithNew`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:7299` `Parser.parseNewExpression`; fasta `LiteralWithNew`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:7305` `Parser.parseNewExpression` | A literal can't be prefixed by 'new'. | não verificado | **publicado** — `crates/frontend/src/parser/expressions.rs:1281` |
| `missing_assignment_in_initializer` | `ParserErrorCode.MISSING_ASSIGNMENT_IN_INITIALIZER` | fasta `MissingAssignmentInInitializer`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:4115` `Parser.parseInitializer`; fasta `MissingAssignmentInInitializer`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:4136` `Parser.parseInitializer` | Expected an assignment after the field name. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:3023` |
| `missing_catch_or_finally` | `ParserErrorCode.MISSING_CATCH_OR_FINALLY` | fasta `OnlyTry`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:8943` `Parser.parseTryStatement` | A try block must be followed by an 'on', 'catch', or 'finally' clause. | não verificado | **publicado** — `crates/frontend/src/parser/statements.rs:818` |
| `missing_expression_in_throw` | `ParserErrorCode.MISSING_EXPRESSION_IN_THROW` | fasta `MissingExpressionInThrow`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:8774` `Parser.parseThrowExpression` | Missing expression after 'throw'. | não verificado | **não implementado** |
| `missing_initializer` | `ParserErrorCode.MISSING_INITIALIZER` | fasta `ExpectedAnInitializer`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:4123` `Parser.parseInitializer` | Expected an initializer. | não verificado | **não implementado** |
| `missing_prefix_in_deferred_import` | `ParserErrorCode.MISSING_PREFIX_IN_DEFERRED_IMPORT` | fasta `MissingPrefixInDeferredImport`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:1041` `Parser.parseImportRecovery` | Deferred imports should have a prefix. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:584` |
| `missing_primary_constructor` | `ParserErrorCode.MISSING_PRIMARY_CONSTRUCTOR` | fasta `MissingPrimaryConstructor`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:3238` `Parser.parseExtensionTypeDeclaration` | An extension type declaration must have a primary constructor declaration. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:1639`, `crates/frontend/src/parser/declarations.rs:3790` |
| `missing_primary_constructor_parameters` | `ParserErrorCode.MISSING_PRIMARY_CONSTRUCTOR_PARAMETERS` | fasta `MissingPrimaryConstructorParameters`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:3232` `Parser.parseExtensionTypeDeclaration` | A primary constructor declaration must have formal parameters. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:1637`, `crates/frontend/src/parser/declarations.rs:3791` |
| `mixin_declares_constructor` | `ParserErrorCode.MIXIN_DECLARES_CONSTRUCTOR` | fasta `MixinDeclaresConstructor`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:5038` `Parser.parseMethod`; fasta `MixinDeclaresConstructor`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:5179` `Parser.parseFactoryMethod` | Mixins can't declare constructors. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:2728` |
| `mixin_with_clause` | `ParserErrorCode.MIXIN_WITH_CLAUSE` | fasta `MixinWithClause`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:3034` `Parser.parseMixinHeaderRecovery` | A mixin can't have a with clause. | não verificado | **não implementado** |
| `multiple_clauses` | `ParserErrorCode.MULTIPLE_CLAUSES` | fasta `MultipleClauses`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:2460` `Parser.parseEnumHeaderOpt`; fasta `MultipleClauses`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:2483` `Parser.parseEnumHeaderOpt`; fasta `MultipleClauses`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:2494` `Parser.parseEnumHeaderOpt` | Each '{0}' definition can have at most one '{1}' clause. | não verificado | **não implementado** |
| `multiple_extends_clauses` | `ParserErrorCode.MULTIPLE_EXTENDS_CLAUSES` | fasta `MultipleExtends`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:2807` `Parser.parseDeclarationHeaderRecoveryInternal`; fasta `MultipleExtends`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:2891` `Parser.parseClassExtendsSeenExtendsClause` | Each class definition can have at most one extends clause. | não verificado | **não implementado** |
| `multiple_on_clauses` | `ParserErrorCode.MULTIPLE_ON_CLAUSES` | fasta `MultipleOnClauses`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:3011` `Parser.parseMixinHeaderRecovery` | Each mixin definition can have at most one on clause. | não verificado | **não implementado** |
| `multiple_with_clauses` | `ParserErrorCode.MULTIPLE_WITH_CLAUSES` | fasta `MultipleWith`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:2831` `Parser.parseDeclarationHeaderRecoveryInternal` | Each class definition can have at most one with clause. | não verificado | **não implementado** |
| `null_aware_cascade_out_of_order` | `ParserErrorCode.NULL_AWARE_CASCADE_OUT_OF_ORDER` | fasta `NullAwareCascadeOutOfOrder`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:5984` `Parser._parsePrecedenceExpressionLoop` | The '?..' cascade operator must be first in the cascade sequence. | não verificado | **não implementado** |
| `out_of_order_clauses` | `ParserErrorCode.OUT_OF_ORDER_CLAUSES` | fasta `OutOfOrderClauses`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:2484` `Parser.parseEnumHeaderOpt` | The '{0}' clause must come before the '{1}' clause. | não verificado | **não implementado** |
| `pattern_assignment_declares_variable` | `ParserErrorCode.PATTERN_ASSIGNMENT_DECLARES_VARIABLE` | fasta `PatternAssignmentDeclaresVariable`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:9922` `Parser.parseVariablePattern` | Variable '{0}' can't be declared in a pattern assignment. | não verificado | **publicado** — `crates/frontend/src/parser/patterns.rs:119` |
| `pattern_variable_declaration_outside_function_or_method` | `ParserErrorCode.PATTERN_VARIABLE_DECLARATION_OUTSIDE_FUNCTION_OR_METHOD` | fasta `PatternVariableDeclarationOutsideFunctionOrMethod`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:3500` `Parser.parseTopLevelMemberImpl`; fasta `PatternVariableDeclarationOutsideFunctionOrMethod`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:4570` `Parser.parseClassOrMixinOrExtensionOrEnumMemberImpl` | A pattern variable declaration may not appear outside a function or method. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:1991` |
| `prefix_after_combinator` | `ParserErrorCode.PREFIX_AFTER_COMBINATOR` | fasta `PrefixAfterCombinator`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:1021` `Parser.parseImportRecovery` | The prefix ('as' clause) should come before any show/hide combinators. | não verificado | **não implementado** |
| `redirecting_constructor_with_body` | `ParserErrorCode.REDIRECTING_CONSTRUCTOR_WITH_BODY` | fasta `RedirectingConstructorWithBody`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:4089` `Parser.parseInitializer` | Redirecting constructors can't have a body. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:3014` |
| `redirection_in_non_factory_constructor` | `ParserErrorCode.REDIRECTION_IN_NON_FACTORY_CONSTRUCTOR` | fasta `RedirectionInNonFactory`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:4979` `Parser.parseMethod` | Only factory constructor can specify '=' redirection. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:2840`, `crates/frontend/src/parser/declarations.rs:3869` |
| `sealed_enum` | `ParserErrorCode.SEALED_ENUM` | fasta `SealedEnum`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:695` `Parser.parseTopLevelKeywordDeclaration` | Enums can't be declared to be 'sealed'. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:902` |
| `sealed_mixin` | `ParserErrorCode.SEALED_MIXIN` | fasta `SealedMixin`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:774` `Parser.parseTopLevelKeywordDeclaration` | A mixin can't be declared 'sealed'. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:910` |
| `sealed_mixin_class` | `ParserErrorCode.SEALED_MIXIN_CLASS` | fasta `SealedMixinClass`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:827` `Parser._handleModifiersForClassDeclaration` | A mixin class can't be declared 'sealed'. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:906` |
| `setter_constructor` | `ParserErrorCode.SETTER_CONSTRUCTOR` | fasta `SetterConstructor`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:5019` `Parser.parseMethod` | Constructors can't be a setter. | não verificado | **não implementado** |
| `stack_overflow` | `ParserErrorCode.STACK_OVERFLOW` | fasta `StackOverflow`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:5808` `Parser.parseExpression`; fasta `StackOverflow`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:9418` `Parser.recoverFromStackOverflow` | The file has too many nested expressions or statements. | não verificado | **publicado** — `crates/frontend/src/parser/mod.rs:639` |
| `static_operator` | `ParserErrorCode.STATIC_OPERATOR` | fasta `StaticOperator`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:4852` `Parser.parseMethod` | Operators can't be static. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:2264` |
| `switch_has_case_after_default_case` | `ParserErrorCode.SWITCH_HAS_CASE_AFTER_DEFAULT_CASE` | fasta `SwitchHasCaseAfterDefault`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:9011` `Parser.parseSwitchBlock` | The default case should be the last case in a switch statement. | não verificado | **publicado** — `crates/frontend/src/parser/statements.rs:749` |
| `switch_has_multiple_default_cases` | `ParserErrorCode.SWITCH_HAS_MULTIPLE_DEFAULT_CASES` | fasta `SwitchHasMultipleDefaults`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:8997` `Parser.parseSwitchBlock` | The 'default' case can only be declared once. | não verificado | **publicado** — `crates/frontend/src/parser/statements.rs:751` |
| `top_level_operator` | `ParserErrorCode.TOP_LEVEL_OPERATOR` | fasta `TopLevelOperator`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:641` `Parser.parseTopLevelDeclarationImpl`; fasta `TopLevelOperator`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:3574` `Parser.parseTopLevelMemberImpl` | Operators must be declared within a class. | não verificado | **não implementado** |
| `type_before_factory` | `ParserErrorCode.TYPE_BEFORE_FACTORY` | fasta `TypeBeforeFactory`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:4621` `Parser.parseClassOrMixinOrExtensionOrEnumMemberImpl` | Factory constructors cannot have a return type. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:2449` |
| `typedef_in_class` | `ParserErrorCode.TYPEDEF_IN_CLASS` | fasta `TypedefInClass`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:9530` `Parser.reportAndSkipTypedefInClass` | Typedefs can't be declared inside classes. | não verificado | **não implementado** |
| `unexpected_tokens` | `ParserErrorCode.UNEXPECTED_TOKENS` | fasta `UnexpectedTokens`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:2569` `Parser.recoverySmallLookAheadSkipTokens` | Unexpected tokens. | não verificado | **não implementado** |
| `var_and_type` | `ParserErrorCode.VAR_AND_TYPE` | fasta `TypeAfterVar`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:2113` `Parser.parseFormalParameter`; fasta `TypeAfterVar`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:3689` `Parser.parseFields`; fasta `TypeAfterVar`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:8245` `Parser.parseExpressionStatementOrDeclarationAfterModifiers`; fasta `TypeAfterVar`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:9912` `Parser.parseVariablePattern` | Variables can't be declared using both 'var' and a type name. | não verificado | **publicado** — `crates/frontend/src/parser/patterns.rs:112` |
| `var_return_type` | `ParserErrorCode.VAR_RETURN_TYPE` | fasta `VarReturnType`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:3621` `Parser.parseTopLevelMemberImpl`; fasta `VarReturnType`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:4869` `Parser.parseMethod` | The return type can't be 'var'. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:2248`, `crates/frontend/src/parser/declarations.rs:2278` |
| `variable_pattern_keyword_in_declaration_context` | `ParserErrorCode.VARIABLE_PATTERN_KEYWORD_IN_DECLARATION_CONTEXT` | fasta `VariablePatternKeywordInDeclarationContext`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:9905` `Parser.parseVariablePattern` | Variable patterns in declaration context can't specify 'var' or 'final' keyword. | não verificado | **publicado** — `crates/frontend/src/parser/patterns.rs:107` |
| `with_before_extends` | `ParserErrorCode.WITH_BEFORE_EXTENDS` | fasta `WithBeforeExtends`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:2811` `Parser.parseDeclarationHeaderRecoveryInternal` | The extends clause must be before the with clause. | não verificado | **não implementado** |

#### `analyzer/lib/src/fasta/error_converter.dart` (5)

| código | constante | emissor (arquivo:linha, método) | condição (mensagem oficial) | posição | DartForge |
|---|---|---|---|---|---|
| `missing_star_after_sync` | `ParserErrorCode.MISSING_STAR_AFTER_SYNC` | `analyzer/lib/src/fasta/error_converter.dart:355` `FastaErrorReporter.reportByCode`; fasta `InvalidSyncModifier`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:5566` `Parser.parseAsyncModifierOpt` | The modifier 'sync' must be followed by a star ('*'). | não verificado | **não implementado** |
| `missing_typedef_parameters` | `ParserErrorCode.MISSING_TYPEDEF_PARAMETERS` | `analyzer/lib/src/fasta/error_converter.dart:362` `FastaErrorReporter.reportByCode`; fasta `MissingTypedefParameters`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:1845` `Parser.missingParameterMessage` | Typedefs must have an explicit list of parameters. | não verificado | **não implementado** |
| `multiple_implements_clauses` | `ParserErrorCode.MULTIPLE_IMPLEMENTS_CLAUSES` | `analyzer/lib/src/fasta/error_converter.dart:369` `FastaErrorReporter.reportByCode`; fasta `MultipleImplements`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:2850` `Parser.parseDeclarationHeaderRecoveryInternal`; fasta `MultipleImplements`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:3026` `Parser.parseMixinHeaderRecovery` | Each class or mixin definition can have at most one implements clause. | não verificado | **não implementado** |
| `named_parameter_outside_group` | `ParserErrorCode.NAMED_PARAMETER_OUTSIDE_GROUP` | `analyzer/lib/src/fasta/error_converter.dart:383` `FastaErrorReporter.reportByCode`; fasta `RequiredParameterWithDefault`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:2182` `Parser.parseFormalParameter` | Named parameters must be enclosed in curly braces ('{' and '}'). | não verificado | **publicado** — `crates/frontend/src/parser/types.rs:753` |
| `wrong_separator_for_positional_parameter` | `ParserErrorCode.WRONG_SEPARATOR_FOR_POSITIONAL_PARAMETER` | `analyzer/lib/src/fasta/error_converter.dart:503` `FastaErrorReporter.reportByCode`; fasta `PositionalParameterWithEquals`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:2185` `Parser.parseFormalParameter` | The default value of a positional parameter should be preceded by '='. | não verificado | **não implementado** |

### II.3 — Parser: conversor de erros do fasta e `AstBuilder` (lote a)

Lote `P2a`: **75 constantes** (39 publicadas, 0 emitidas e não publicadas, 36 não implementadas no DartForge).
Esta tabela foi **gerada por script** (`E:\dftempnalise\spec-infrainal\gera2.py`) a partir do levantamento automático (`catalogo.json`: definição, sítios de emissão por grep com o método que os contém, referências em `crates/`). A coluna "condição" traz a **mensagem oficial** do código (o texto do `messages.yaml`), não a condição lida no método emissor; a coluna "posição" está **não verificada** em todas as linhas. Antes de implementar um código deste lote, abra o emissor citado e escreva os seis campos (o molde está em `docs/ANALYZER-ESPECIFICACAO.md`).

#### `parser fasta` (39)

| código | constante | emissor (arquivo:linha, método) | condição (mensagem oficial) | posição | DartForge |
|---|---|---|---|---|---|
| `abstract_late_field` | `ParserErrorCode.ABSTRACT_LATE_FIELD` | fasta `AbstractLateField`: `analyzer/lib/src/fasta/ast_builder.dart:1262` `AstBuilder.endClassFields` | Abstract fields cannot be late. | não verificado | **não implementado** |
| `abstract_static_field` | `ParserErrorCode.ABSTRACT_STATIC_FIELD` | fasta `AbstractStaticField`: `analyzer/lib/src/fasta/ast_builder.dart:1258` `AstBuilder.endClassFields` | Static fields can't be declared 'abstract'. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:2514`, `crates/frontend/src/parser/declarations.rs:3398` |
| `annotation_on_type_argument` | `ParserErrorCode.ANNOTATION_ON_TYPE_ARGUMENT` | fasta `AnnotationOnTypeArgument`: `_fe_analyzer_shared/lib/src/parser/type_info_impl.dart:1383` `ComplexTypeParamOrArgInfo.parseArguments` | Type arguments can't have annotations because they aren't declarations. | não verificado | **publicado** — `crates/frontend/src/parser/types.rs:360` |
| `catch_syntax_extra_parameters` | `ParserErrorCode.CATCH_SYNTAX_EXTRA_PARAMETERS` | fasta `CatchSyntaxExtraParameters`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:8916` `Parser.parseTryStatement`; fasta `CatchSyntaxExtraParameters`: `_fe_analyzer_shared/lib/src/parser/stack_listener.dart:401` `StackListener.isIgnoredError` | 'catch' must be followed by '(identifier)' or '(identifier, identifier)'. | não verificado | **não implementado** |
| `conflicting_modifiers` | `ParserErrorCode.CONFLICTING_MODIFIERS` | fasta `ConflictingModifiers`: `_fe_analyzer_shared/lib/src/parser/modifier_context.dart:644` `ModifierContext.reportConflictingModifiers` | Members can't be declared to be both '{0}' and '{1}'. | não verificado | **publicado** — `crates/frontend/src/parser/modificadores.rs:144` |
| `const_and_final` | `ParserErrorCode.CONST_AND_FINAL` | fasta `ConstAndFinal`: `_fe_analyzer_shared/lib/src/parser/modifier_context.dart:439` `ModifierContext._parseConst`; fasta `ConstAndFinal`: `_fe_analyzer_shared/lib/src/parser/modifier_context.dart:528` `ModifierContext._parseFinal` | Members can't be declared to be both 'const' and 'final'. | não verificado | **publicado** — `crates/frontend/src/parser/modificadores.rs:205`, `crates/frontend/src/parser/modificadores.rs:267` |
| `const_class` | `ParserErrorCode.CONST_CLASS` | fasta `ConstClass`: `_fe_analyzer_shared/lib/src/parser/modifier_context.dart:660` `ModifierContext.reportTopLevelModifierError` | Classes can't be declared to be 'const'. | não verificado | **publicado** — `crates/frontend/src/parser/modificadores.rs:486` |
| `const_factory` | `ParserErrorCode.CONST_FACTORY` | fasta `ConstFactory`: `analyzer/lib/src/fasta/ast_builder.dart:3975` `AstBuilder.handleConstFactory` | Only redirecting factory constructors can be declared to be 'const'. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:2904`, `crates/frontend/src/parser/declarations.rs:3463` |
| `covariant_and_static` | `ParserErrorCode.COVARIANT_AND_STATIC` | fasta `CovariantAndStatic`: `_fe_analyzer_shared/lib/src/parser/modifier_context.dart:476` `ModifierContext._parseCovariant`; fasta `CovariantAndStatic`: `_fe_analyzer_shared/lib/src/parser/modifier_context.dart:605` `ModifierContext._parseStatic` | Members can't be declared to be both 'covariant' and 'static'. | não verificado | **publicado** — `crates/frontend/src/parser/modificadores.rs:233`, `crates/frontend/src/parser/modificadores.rs:315` |
| `directive_after_declaration` | `ParserErrorCode.DIRECTIVE_AFTER_DECLARATION` | fasta `DirectiveAfterDeclaration`: `analyzer/lib/src/fasta/ast_builder.dart:209` `AstBuilder.addProblem`; fasta `DirectiveAfterDeclaration`: `_fe_analyzer_shared/lib/src/parser/directive_context.dart:51` `DirectiveContext.checkExport`; fasta `DirectiveAfterDeclaration`: `_fe_analyzer_shared/lib/src/parser/directive_context.dart:71` `DirectiveContext.checkImport`; fasta `DirectiveAfterDeclaration`: `_fe_analyzer_shared/lib/src/parser/directive_context.dart:105` `DirectiveContext.checkPart` | Directives must appear before any declarations. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:213`, `crates/frontend/src/parser/declarations.rs:226` |
| `duplicate_label_in_switch_statement` | `ParserErrorCode.DUPLICATE_LABEL_IN_SWITCH_STATEMENT` | fasta `DuplicateLabelInSwitchStatement`: `analyzer/lib/src/fasta/ast_builder.dart:3137` `AstBuilder.endSwitchBlock` | The label '{0}' was already used in this switch statement. | não verificado | **não implementado** |
| `duplicated_modifier` | `ParserErrorCode.DUPLICATED_MODIFIER` | fasta `DuplicatedModifier`: `_fe_analyzer_shared/lib/src/parser/modifier_context.dart:356` `ModifierContext._parseModifiers`; fasta `DuplicatedModifier`: `_fe_analyzer_shared/lib/src/parser/modifier_context.dart:382` `ModifierContext._parseAbstract`; fasta `DuplicatedModifier`: `_fe_analyzer_shared/lib/src/parser/modifier_context.dart:414` `ModifierContext._parseAugment`; fasta `DuplicatedModifier`: `_fe_analyzer_shared/lib/src/parser/modifier_context.dart:435` `ModifierContext._parseConst` | The modifier '{0}' was already specified. | não verificado | **publicado** — `crates/frontend/src/parser/modificadores.rs:129` |
| `export_directive_after_part_directive` | `ParserErrorCode.EXPORT_DIRECTIVE_AFTER_PART_DIRECTIVE` | fasta `ExportAfterPart`: `_fe_analyzer_shared/lib/src/parser/directive_context.dart:43` `DirectiveContext.checkExport` | Export directives must precede part directives. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:207` |
| `external_class` | `ParserErrorCode.EXTERNAL_CLASS` | fasta `ExternalClass`: `_fe_analyzer_shared/lib/src/parser/modifier_context.dart:663` `ModifierContext.reportTopLevelModifierError` | Classes can't be declared to be 'external'. | não verificado | **publicado** — `crates/frontend/src/parser/modificadores.rs:493` |
| `external_enum` | `ParserErrorCode.EXTERNAL_ENUM` | fasta `ExternalEnum`: `_fe_analyzer_shared/lib/src/parser/modifier_context.dart:665` `ModifierContext.reportTopLevelModifierError` | Enums can't be declared to be 'external'. | não verificado | **publicado** — `crates/frontend/src/parser/modificadores.rs:494` |
| `external_late_field` | `ParserErrorCode.EXTERNAL_LATE_FIELD` | fasta `ExternalLateField`: `analyzer/lib/src/fasta/ast_builder.dart:1268` `AstBuilder.endClassFields`; fasta `ExternalLateField`: `analyzer/lib/src/fasta/ast_builder.dart:3359` `AstBuilder.endTopLevelFields` | External fields cannot be late. | não verificado | **não implementado** |
| `external_typedef` | `ParserErrorCode.EXTERNAL_TYPEDEF` | fasta `ExternalTypedef`: `_fe_analyzer_shared/lib/src/parser/modifier_context.dart:667` `ModifierContext.reportTopLevelModifierError` | Typedefs can't be declared to be 'external'. | não verificado | **publicado** — `crates/frontend/src/parser/modificadores.rs:495` |
| `extraneous_modifier_in_extension_type` | `ParserErrorCode.EXTRANEOUS_MODIFIER_IN_EXTENSION_TYPE` | fasta `ExtraneousModifierInExtensionType`: `_fe_analyzer_shared/lib/src/parser/modifier_context.dart:689` `ModifierContext.reportExtraneousModifierInExtensionType` | Can't have modifier '{0}' in an extension type. | não verificado | **publicado** — `crates/frontend/src/parser/modificadores.rs:447` |
| `final_and_var` | `ParserErrorCode.FINAL_AND_VAR` | fasta `FinalAndVar`: `_fe_analyzer_shared/lib/src/parser/modifier_context.dart:530` `ModifierContext._parseFinal`; fasta `FinalAndVar`: `_fe_analyzer_shared/lib/src/parser/modifier_context.dart:634` `ModifierContext._parseVar` | Members can't be declared to be both 'final' and 'var'. | não verificado | **publicado** — `crates/frontend/src/parser/modificadores.rs:269`, `crates/frontend/src/parser/modificadores.rs:335` |
| `illegal_assignment_to_non_assignable` | `ParserErrorCode.ILLEGAL_ASSIGNMENT_TO_NON_ASSIGNABLE` | fasta `IllegalAssignmentToNonAssignable`: `analyzer/lib/src/fasta/ast_builder.dart:4253` `AstBuilder.handleExpressionStatement`; fasta `IllegalAssignmentToNonAssignable`: `analyzer/lib/src/fasta/ast_builder.dart:5670` `AstBuilder.handleUnaryPostfixAssignmentExpression` | Illegal assignment to non-assignable expression. | não verificado | **publicado** — `crates/frontend/src/parser/expressions.rs:947`, `crates/frontend/src/parser/statements.rs:424` |
| `import_directive_after_part_directive` | `ParserErrorCode.IMPORT_DIRECTIVE_AFTER_PART_DIRECTIVE` | fasta `ImportAfterPart`: `_fe_analyzer_shared/lib/src/parser/directive_context.dart:63` `DirectiveContext.checkImport` | Import directives must precede part directives. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:207` |
| `invalid_hex_escape` | `ParserErrorCode.INVALID_HEX_ESCAPE` | fasta `InvalidHexEscape`: `_fe_analyzer_shared/lib/src/parser/quote.dart:232` `.unescapeCodeUnits`; fasta `InvalidHexEscape`: `_fe_analyzer_shared/lib/src/parser/quote.dart:241` `.unescapeCodeUnits` | An escape sequence starting with '\\x' must be followed by 2 hexadecimal digits. | não verificado | **publicado** — `crates/frontend/src/parser/expressions.rs:118`, `crates/frontend/src/parser/expressions.rs:123` |
| `invalid_initializer` | `ParserErrorCode.INVALID_INITIALIZER` | fasta `InvalidInitializer`: `analyzer/lib/src/fasta/ast_builder.dart:2303` `AstBuilder.endInitializers` | Not a valid initializer. | não verificado | **não implementado** |
| `invalid_this_in_initializer` | `ParserErrorCode.INVALID_THIS_IN_INITIALIZER` | fasta `InvalidThisInInitializer`: `analyzer/lib/src/fasta/ast_builder.dart:766` `AstBuilder.buildInitializerTargetExpressionRecovery` | Can only use 'this' in an initializer for field initialization (e.g. 'this.x = something') and constructor redirection (e.g. 'this()' or 'this.namedConstructor()) | não verificado | **não implementado** |
| `invalid_unicode_escape_u_bracket` | `ParserErrorCode.INVALID_UNICODE_ESCAPE_U_BRACKET` | fasta `InvalidUnicodeEscapeUBracket`: `_fe_analyzer_shared/lib/src/parser/quote.dart:262` `.unescapeCodeUnits`; fasta `InvalidUnicodeEscapeUBracket`: `_fe_analyzer_shared/lib/src/parser/quote.dart:272` `.unescapeCodeUnits`; fasta `InvalidUnicodeEscapeUBracket`: `_fe_analyzer_shared/lib/src/parser/quote.dart:287` `.unescapeCodeUnits`; fasta `InvalidUnicodeEscapeUBracket`: `_fe_analyzer_shared/lib/src/parser/quote.dart:297` `.unescapeCodeUnits` | An escape sequence starting with '\\u{' must be followed by 1 to 6 hexadecimal digits followed by a '}'. | não verificado | **publicado** — `crates/frontend/src/parser/expressions.rs:138`, `crates/frontend/src/parser/expressions.rs:148` |
| `invalid_unicode_escape_u_no_bracket` | `ParserErrorCode.INVALID_UNICODE_ESCAPE_U_NO_BRACKET` | fasta `InvalidUnicodeEscapeUNoBracket`: `_fe_analyzer_shared/lib/src/parser/quote.dart:306` `.unescapeCodeUnits`; fasta `InvalidUnicodeEscapeUNoBracket`: `_fe_analyzer_shared/lib/src/parser/quote.dart:317` `.unescapeCodeUnits` | An escape sequence starting with '\\u' must be followed by 4 hexadecimal digits. | não verificado | **publicado** — `crates/frontend/src/parser/expressions.rs:157`, `crates/frontend/src/parser/expressions.rs:162` |
| `invalid_unicode_escape_u_started` | `ParserErrorCode.INVALID_UNICODE_ESCAPE_U_STARTED` | fasta `InvalidUnicodeEscapeUStarted`: `_fe_analyzer_shared/lib/src/parser/quote.dart:250` `.unescapeCodeUnits` | An escape sequence starting with '\\u' must be followed by 4 hexadecimal digits or from 1 to 6 digits between '{' and '}'. | não verificado | **publicado** — `crates/frontend/src/parser/expressions.rs:129` |
| `invalid_use_of_covariant_in_extension` | `ParserErrorCode.INVALID_USE_OF_COVARIANT_IN_EXTENSION` | fasta `ExtraneousModifierInExtension`: `_fe_analyzer_shared/lib/src/parser/modifier_context.dart:682` `ModifierContext.reportExtraneousModifierInExtension` | Can't have modifier '{0}' in an extension. | não verificado | **publicado** — `crates/frontend/src/parser/modificadores.rs:441` |
| `library_directive_not_first` | `ParserErrorCode.LIBRARY_DIRECTIVE_NOT_FIRST` | fasta `LibraryDirectiveNotFirst`: `_fe_analyzer_shared/lib/src/parser/directive_context.dart:86` `DirectiveContext.checkLibrary` | The library directive must appear before all other directives. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:237` |
| `missing_keyword_operator` | `ParserErrorCode.MISSING_KEYWORD_OPERATOR` | fasta `MissingOperatorKeyword`: `_fe_analyzer_shared/lib/src/parser/identifier_context_impl.dart:935` `MethodDeclarationIdentifierContext.ensureIdentifier`; fasta `MissingOperatorKeyword`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:9280` `Parser.parseInvalidOperatorDeclaration` | Operator declarations must be preceded by the keyword 'operator'. | não verificado | **não implementado** |
| `modifier_out_of_order` | `ParserErrorCode.MODIFIER_OUT_OF_ORDER` | fasta `ModifierOutOfOrder`: `_fe_analyzer_shared/lib/src/parser/modifier_context.dart:703` `ModifierContext.reportModifierOutOfOrder` | The modifier '{0}' should be before the modifier '{1}'. | não verificado | **publicado** — `crates/frontend/src/parser/modificadores.rs:136` |
| `multiple_library_directives` | `ParserErrorCode.MULTIPLE_LIBRARY_DIRECTIVES` | fasta `MultipleLibraryDirectives`: `_fe_analyzer_shared/lib/src/parser/directive_context.dart:82` `DirectiveContext.checkLibrary` | Only one library directive may be declared in a file. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:233` |
| `multiple_part_of_directives` | `ParserErrorCode.MULTIPLE_PART_OF_DIRECTIVES` | fasta `PartOfTwice`: `_fe_analyzer_shared/lib/src/parser/directive_context.dart:116` `DirectiveContext.checkPartOf` | Only one part-of directive may be declared in a file. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:245` |
| `multiple_variance_modifiers` | `ParserErrorCode.MULTIPLE_VARIANCE_MODIFIERS` | fasta `MultipleVarianceModifiers`: `_fe_analyzer_shared/lib/src/parser/type_info_impl.dart:1440` `ComplexTypeParamOrArgInfo.parseVariables` | Each type parameter can have at most one variance modifier. | não verificado | **publicado** — `crates/frontend/src/parser/types.rs:424` |
| `native_clause_should_be_annotation` | `ParserErrorCode.NATIVE_CLAUSE_SHOULD_BE_ANNOTATION` | fasta `NativeClauseShouldBeAnnotation`: `analyzer/lib/src/fasta/ast_builder.dart:5343` `AstBuilder.handleRecoverableError`; fasta `NativeClauseShouldBeAnnotation`: `_fe_analyzer_shared/lib/src/parser/parser.dart:53` `ErrorCollectingListener.handleRecoverableError`; fasta `NativeClauseShouldBeAnnotation`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:4362` `Parser.parseNativeClause`; fasta `NativeClauseShouldBeAnnotation`: `_fe_analyzer_shared/lib/src/parser/stack_listener.dart:397` `StackListener.isIgnoredError` | Native clause in this form is deprecated. | não verificado | **não implementado** |
| `type_parameter_on_constructor` | `ParserErrorCode.TYPE_PARAMETER_ON_CONSTRUCTOR` | fasta `ConstructorWithTypeParameters`: `analyzer/lib/src/fasta/ast_builder.dart:5899` `AstBuilder._buildConstructorDeclaration`; fasta `ConstructorWithTypeParameters`: `analyzer/lib/src/fasta/ast_builder.dart:5976` `AstBuilder._buildFactoryConstructorDeclaration` | Constructors can't have type parameters. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:2712` |
| `type_parameter_on_operator` | `ParserErrorCode.TYPE_PARAMETER_ON_OPERATOR` | fasta `OperatorWithTypeParameters`: `analyzer/lib/src/fasta/ast_builder.dart:1345` `AstBuilder.endClassMethod` | Types parameters aren't allowed when defining an operator. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:2198` |
| `var_as_type_name` | `ParserErrorCode.VAR_AS_TYPE_NAME` | fasta `VarAsTypeName`: `_fe_analyzer_shared/lib/src/parser/identifier_context_impl.dart:1225` `TypeReferenceIdentifierContext.ensureIdentifier` | The keyword 'var' can't be used as a type name. | não verificado | **não implementado** |
| `void_with_type_arguments` | `ParserErrorCode.VOID_WITH_TYPE_ARGUMENTS` | fasta `VoidWithTypeArguments`: `_fe_analyzer_shared/lib/src/parser/type_info_impl.dart:454` `VoidType.parseType` | Type 'void' can't have type arguments. | não verificado | **publicado** — `crates/frontend/src/parser/types.rs:155` |

#### `analyzer/lib/src/fasta/error_converter.dart` (20)

| código | constante | emissor (arquivo:linha, método) | condição (mensagem oficial) | posição | DartForge |
|---|---|---|---|---|---|
| `async_keyword_used_as_identifier` | `ParserErrorCode.ASYNC_KEYWORD_USED_AS_IDENTIFIER` | `analyzer/lib/src/fasta/error_converter.dart:45` `FastaErrorReporter.reportByCode`; fasta `AwaitAsIdentifier`: `_fe_analyzer_shared/lib/src/parser/identifier_context_impl.dart:1320` `.checkAsyncAwaitYieldAsIdentifier`; fasta `YieldAsIdentifier`: `_fe_analyzer_shared/lib/src/parser/identifier_context_impl.dart:1322` `.checkAsyncAwaitYieldAsIdentifier` | The keywords 'await' and 'yield' can't be used as identifiers in an asynchronous or generator function. | não verificado | **publicado** — `crates/frontend/src/parser/statements.rs:556` |
| `const_constructor_with_body` | `ParserErrorCode.CONST_CONSTRUCTOR_WITH_BODY` | `analyzer/lib/src/fasta/error_converter.dart:74` `FastaErrorReporter.reportByCode`; fasta `ConstConstructorWithBody`: `analyzer/lib/src/fasta/ast_builder.dart:5908` `AstBuilder._buildConstructorDeclaration` | Const constructors can't have a body. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:2893`, `crates/frontend/src/parser/declarations.rs:3431` |
| `empty_enum_body` | `ParserErrorCode.EMPTY_ENUM_BODY` | `analyzer/lib/src/fasta/error_converter.dart:105` `FastaErrorReporter.reportByCode` | An enum must declare at least one constant name. | não verificado | **não implementado** |
| `invalid_cast_function` | `CompileTimeErrorCode.INVALID_CAST_FUNCTION` | `analyzer/lib/src/fasta/error_converter.dart:219` `FastaErrorReporter.reportByCode` | The function '{0}' has type '{1}' that isn't of expected type '{2}'. This means its parameter or return type doesn't match what is expected. | não verificado | **não implementado** |
| `invalid_cast_function_expr` | `CompileTimeErrorCode.INVALID_CAST_FUNCTION_EXPR` | `analyzer/lib/src/fasta/error_converter.dart:226` `FastaErrorReporter.reportByCode` | The function expression type '{0}' isn't of type '{1}'. This means its parameter or return type doesn't match what is expected. Consider changing parameter type(s) or the returned type(s). | não verificado | **não implementado** |
| `invalid_cast_literal_list` | `CompileTimeErrorCode.INVALID_CAST_LITERAL_LIST` | `analyzer/lib/src/fasta/error_converter.dart:233` `FastaErrorReporter.reportByCode` | The list literal type '{0}' isn't of expected type '{1}'. The list's type can be changed with an explicit generic type argument or by changing the element types. | não verificado | **não implementado** |
| `invalid_cast_literal_map` | `CompileTimeErrorCode.INVALID_CAST_LITERAL_MAP` | `analyzer/lib/src/fasta/error_converter.dart:240` `FastaErrorReporter.reportByCode` | The map literal type '{0}' isn't of expected type '{1}'. The map's type can be changed with an explicit generic type arguments or by changing the key and value types. | não verificado | **não implementado** |
| `invalid_cast_literal_set` | `CompileTimeErrorCode.INVALID_CAST_LITERAL_SET` | `analyzer/lib/src/fasta/error_converter.dart:247` `FastaErrorReporter.reportByCode` | The set literal type '{0}' isn't of expected type '{1}'. The set's type can be changed with an explicit generic type argument or by changing the element types. | não verificado | **não implementado** |
| `invalid_cast_method` | `CompileTimeErrorCode.INVALID_CAST_METHOD` | `analyzer/lib/src/fasta/error_converter.dart:254` `FastaErrorReporter.reportByCode` | The method tear-off '{0}' has type '{1}' that isn't of expected type '{2}'. This means its parameter or return type doesn't match what is expected. | não verificado | **não implementado** |
| `invalid_cast_new_expr` | `CompileTimeErrorCode.INVALID_CAST_NEW_EXPR` | `analyzer/lib/src/fasta/error_converter.dart:261` `FastaErrorReporter.reportByCode` | The constructor returns type '{0}' that isn't of expected type '{1}'. | não verificado | **não implementado** |
| `invalid_code_point` | `ParserErrorCode.INVALID_CODE_POINT` | `analyzer/lib/src/fasta/error_converter.dart:268` `FastaErrorReporter.reportByCode`; fasta `InvalidCodePoint`: `_fe_analyzer_shared/lib/src/parser/quote.dart:328` `.unescapeCodeUnits` | The escape sequence '{0}' isn't a valid code point. | não verificado | **publicado** — `crates/frontend/src/parser/expressions.rs:168`, `crates/frontend/src/parser/expressions.rs:1904` |
| `invalid_generic_function_type` | `ParserErrorCode.INVALID_GENERIC_FUNCTION_TYPE` | `analyzer/lib/src/fasta/error_converter.dart:276` `FastaErrorReporter.reportByCode` | Invalid generic function type. | não verificado | **não implementado** |
| `invalid_inline_function_type` | `CompileTimeErrorCode.INVALID_INLINE_FUNCTION_TYPE` | `analyzer/lib/src/fasta/error_converter.dart:198` `FastaErrorReporter.reportByCode`; fasta `InvalidInlineFunctionType`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:2137` `Parser.parseFormalParameter` | Inline function types can't be used for parameters in a generic function type. | não verificado | **não implementado** |
| `invalid_literal_in_configuration` | `ParserErrorCode.INVALID_LITERAL_IN_CONFIGURATION` | `analyzer/lib/src/fasta/error_converter.dart:205` `FastaErrorReporter.reportByCode`; fasta `InterpolationInUri`: `analyzer/lib/src/fasta/ast_builder.dart:1454` `AstBuilder.endConditionalUri` | The literal in a configuration can't contain interpolation. | não verificado | **não implementado** |
| `invalid_operator_for_super` | `ParserErrorCode.INVALID_OPERATOR_FOR_SUPER` | `analyzer/lib/src/fasta/error_converter.dart:298` `FastaErrorReporter.reportByCode` | The operator '{0}' can't be used with 'super'. | não verificado | **não implementado** |
| `missing_enum_body` | `ParserErrorCode.MISSING_ENUM_BODY` | `analyzer/lib/src/fasta/error_converter.dart:313` `FastaErrorReporter.reportByCode`; fasta `ExpectedEnumBody`: `_fe_analyzer_shared/lib/src/parser/block_kind.dart:26` `BlockKind.toString` | An enum definition must have a body with at least one constant name. | não verificado | **não implementado** |
| `non_part_of_directive_in_part` | `ParserErrorCode.NON_PART_OF_DIRECTIVE_IN_PART` | `analyzer/lib/src/fasta/error_converter.dart:390` `FastaErrorReporter.reportByCode`; fasta `NonPartOfDirectiveInPart`: `_fe_analyzer_shared/lib/src/parser/directive_context.dart:48` `DirectiveContext.checkExport`; fasta `NonPartOfDirectiveInPart`: `_fe_analyzer_shared/lib/src/parser/directive_context.dart:68` `DirectiveContext.checkImport`; fasta `NonPartOfDirectiveInPart`: `_fe_analyzer_shared/lib/src/parser/directive_context.dart:84` `DirectiveContext.checkLibrary` | The part-of directive must be the only directive in a part. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:212`, `crates/frontend/src/parser/declarations.rs:225` |
| `positional_after_named_argument` | `ParserErrorCode.POSITIONAL_AFTER_NAMED_ARGUMENT` | `analyzer/lib/src/fasta/error_converter.dart:404` `FastaErrorReporter.reportByCode`; fasta `PositionalAfterNamedArgument`: `analyzer/lib/src/fasta/ast_builder.dart:924` `AstBuilder.endArguments` | Positional arguments must occur before named arguments. | não verificado | **não implementado** |
| `unexpected_dollar_in_string` | `ScannerErrorCode.UNEXPECTED_DOLLAR_IN_STRING` | `analyzer/lib/src/fasta/error_converter.dart:467` `FastaErrorReporter.reportByCode`; fasta `UnexpectedDollarInString`: `_fe_analyzer_shared/lib/src/scanner/abstract_scanner.dart:1896` `AbstractScanner.tokenizeInterpolatedIdentifier`; fasta `UnexpectedDollarInString`: `_fe_analyzer_shared/lib/src/scanner/errors.dart:87` `._makeError` | A '$' has special meaning inside a string, and must be followed by an identifier or an expression in curly braces ({}). | não verificado | **não implementado** |
| `unterminated_multi_line_comment` | `ScannerErrorCode.UNTERMINATED_MULTI_LINE_COMMENT` | `analyzer/lib/src/fasta/error_converter.dart:482` `FastaErrorReporter.reportByCode`; `_fe_analyzer_shared/lib/src/scanner/errors.dart:42` `_makeError`; fasta `UnterminatedComment`: `_fe_analyzer_shared/lib/src/scanner/abstract_scanner.dart:1624` `AbstractScanner.tokenizeMultiLineComment` | Unterminated multi-line comment. | não verificado | **publicado** — `crates/frontend/src/lexer.rs:238` |

#### `analyzer/lib/src/fasta/ast_builder.dart` (10)

| código | constante | emissor (arquivo:linha, método) | condição (mensagem oficial) | posição | DartForge |
|---|---|---|---|---|---|
| `declaration_named_augmented_inside_augmentation` | `ParserErrorCode.DECLARATION_NAMED_AUGMENTED_INSIDE_AUGMENTATION` | `analyzer/lib/src/fasta/ast_builder.dart:4007` `AstBuilder.handleDeclaredVariablePattern`; `analyzer/lib/src/fasta/ast_builder.dart:4436` `AstBuilder.handleIdentifier` | The identifier 'augmented' has a special meaning inside augmenting declarations. | não verificado | **não implementado** |
| `expected_named_type` | `ParserErrorCode.EXPECTED_NAMED_TYPE_EXTENDS` | `analyzer/lib/src/fasta/ast_builder.dart:2657` `AstBuilder.endNamedMixinApplication`; `analyzer/lib/src/fasta/ast_builder.dart:3890` `AstBuilder.handleClassExtends` | Expected a class name. | não verificado | **não implementado** |
| `expected_named_type` | `ParserErrorCode.EXPECTED_NAMED_TYPE_IMPLEMENTS` | `analyzer/lib/src/fasta/ast_builder.dart:2645` `AstBuilder.endNamedMixinApplication`; `analyzer/lib/src/fasta/ast_builder.dart:4497` `AstBuilder.handleImplements` | Expected the name of a class or mixin. | não verificado | **não implementado** |
| `expected_named_type` | `ParserErrorCode.EXPECTED_NAMED_TYPE_ON` | `analyzer/lib/src/fasta/ast_builder.dart:4942` `AstBuilder.handleMixinOn` | Expected the name of a class or mixin. | não verificado | **não implementado** |
| `expected_named_type` | `ParserErrorCode.EXPECTED_NAMED_TYPE_WITH` | `analyzer/lib/src/fasta/ast_builder.dart:3961` `AstBuilder.handleClassWithClause`; `analyzer/lib/src/fasta/ast_builder.dart:4195` `AstBuilder.handleEnumWithClause`; `analyzer/lib/src/fasta/ast_builder.dart:4961` `AstBuilder.handleMixinWithClause` | Expected a mixin name. | não verificado | **não implementado** |
| `invalid_use_of_identifier_augmented` | `ParserErrorCode.INVALID_USE_OF_IDENTIFIER_AUGMENTED` | `analyzer/lib/src/fasta/ast_builder.dart:4977` `AstBuilder.handleNamedArgument`; `analyzer/lib/src/fasta/ast_builder.dart:5619` `AstBuilder.handleType` | The identifier 'augmented' can only be used to reference the augmented declaration inside an augmentation. | não verificado | **não implementado** |
| `member_with_class_name` | `ParserErrorCode.MEMBER_WITH_CLASS_NAME` | `analyzer/lib/src/fasta/ast_builder.dart:1725` `AstBuilder.endExtensionTypeDeclaration`; fasta `MemberWithSameNameAsClass`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:3933` `Parser.parseFieldInitializerOpt`; fasta `MemberWithSameNameAsClass`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:4999` `Parser.parseMethod` | A class member can't have the same name as the enclosing class. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:2331` |
| `multiple_representation_fields` | `ParserErrorCode.MULTIPLE_REPRESENTATION_FIELDS` | `analyzer/lib/src/fasta/ast_builder.dart:2906` `AstBuilder.endPrimaryConstructor` | Each extension type should have exactly one representation field. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:1714`, `crates/frontend/src/parser/declarations.rs:1751` |
| `part_of_name` | `ParserErrorCode.PART_OF_NAME` | `analyzer/lib/src/fasta/ast_builder.dart:2806` `AstBuilder.endPartOf` | The 'part of' directive can't use a name with the enhanced-parts feature. | não verificado | **não implementado** |
| `representation_field_trailing_comma` | `ParserErrorCode.REPRESENTATION_FIELD_TRAILING_COMMA` | `analyzer/lib/src/fasta/ast_builder.dart:2901` `AstBuilder.endPrimaryConstructor` | The representation field can't have a trailing comma. | não verificado | **publicado** — `crates/frontend/src/parser/declarations.rs:1749` |

#### `analyzer/lib/src/fasta/doc_comment_builder.dart` (3)

| código | constante | emissor (arquivo:linha, método) | condição (mensagem oficial) | posição | DartForge |
|---|---|---|---|---|---|
| `doc_directive_missing_closing_tag` | `WarningCode.DOC_DIRECTIVE_MISSING_CLOSING_TAG` | `analyzer/lib/src/fasta/doc_comment_builder.dart:200` `DocCommentBuilder._endBlockDocDirectiveTag`; `analyzer/lib/src/fasta/doc_comment_builder.dart:297` `DocCommentBuilder._parseDocComment` | Doc directive is missing a closing tag. | não verificado | **não implementado** |
| `doc_directive_missing_opening_tag` | `WarningCode.DOC_DIRECTIVE_MISSING_OPENING_TAG` | `analyzer/lib/src/fasta/doc_comment_builder.dart:217` `DocCommentBuilder._endBlockDocDirectiveTag` | Doc directive is missing an opening tag. | não verificado | **não implementado** |
| `doc_directive_unknown` | `WarningCode.DOC_DIRECTIVE_UNKNOWN` | `analyzer/lib/src/fasta/doc_comment_builder.dart:387` `DocCommentBuilder._parseDocDirectiveTag` | Doc directive '{0}' is unknown. | não verificado | **não implementado** |

#### `sem emissor localizado` (2)

| código | constante | emissor (arquivo:linha, método) | condição (mensagem oficial) | posição | DartForge |
|---|---|---|---|---|---|
| `missing_quote` | `ScannerErrorCode.MISSING_QUOTE` | sem emissor localizado por grep no 3.6.2 (definição em `_fe_analyzer_shared/lib/src/scanner/errors.dart:148`) | Expected quote (' or "). | não verificado | nada a implementar: conferido em 2026-10-05 que nenhum arquivo de `_fe_analyzer_shared/lib`, `analyzer/lib` ou `front_end` da 3.6.2 relata o código; ele só existe na tabela |
| `unable_get_content` | `ScannerErrorCode.UNABLE_GET_CONTENT` | sem emissor localizado por grep no 3.6.2 (definição em `_fe_analyzer_shared/lib/src/scanner/errors.dart:155`) | Unable to get content of '{0}'. | não verificado | **não implementado** |

#### `_fe_analyzer_shared/lib/src/scanner/errors.dart` (1)

| código | constante | emissor (arquivo:linha, método) | condição (mensagem oficial) | posição | DartForge |
|---|---|---|---|---|---|
| `unexpected_separator_in_number` | `ScannerErrorCode.UNEXPECTED_SEPARATOR_IN_NUMBER` | `_fe_analyzer_shared/lib/src/scanner/errors.dart:64` `_makeError`; fasta `UnexpectedSeparatorInNumber`: `_fe_analyzer_shared/lib/src/scanner/abstract_scanner.dart:1267` `AbstractScanner.tokenizeNumber`; fasta `UnexpectedSeparatorInNumber`: `_fe_analyzer_shared/lib/src/scanner/abstract_scanner.dart:1275` `AbstractScanner.tokenizeNumber`; fasta `UnexpectedSeparatorInNumber`: `_fe_analyzer_shared/lib/src/scanner/abstract_scanner.dart:1290` `AbstractScanner.tokenizeNumber` | Digit separators ('_') in a number literal can only be placed between two digits. | não verificado | **publicado** — `crates/frontend/src/parser/expressions.rs:224` |

### II.4 — Constantes sem emissor localizado no 3.6.2 e restos do parser

Lote `P2b`: **59 constantes** (2 publicadas, 0 emitidas e não publicadas, 57 não implementadas no DartForge).
Esta tabela foi **gerada por script** (`E:\dftempnalise\spec-infrainal\gera2.py`) a partir do levantamento automático (`catalogo.json`: definição, sítios de emissão por grep com o método que os contém, referências em `crates/`). A coluna "condição" traz a **mensagem oficial** do código (o texto do `messages.yaml`), não a condição lida no método emissor; a coluna "posição" está **não verificada** em todas as linhas. Antes de implementar um código deste lote, abra o emissor citado e escreva os seis campos (o molde está em `docs/ANALYZER-ESPECIFICACAO.md`).

#### `sem emissor localizado` (59)

| código | constante | emissor (arquivo:linha, método) | condição (mensagem oficial) | posição | DartForge |
|---|---|---|---|---|---|
| `abstract_static_method` | `ParserErrorCode.ABSTRACT_STATIC_METHOD` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:254`) | Static methods can't be declared to be 'abstract'. | não verificado | **não implementado** |
| `annotation_with_type_arguments` | `ParserErrorCode.ANNOTATION_WITH_TYPE_ARGUMENTS` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:272`) | An annotation can't use type arguments. | não verificado | **não implementado** |
| `covariant_constructor` | `ParserErrorCode.COVARIANT_CONSTRUCTOR` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:417`) | A constructor can't be declared to be 'covariant'. | não verificado | **não implementado** |
| `expected_case_or_default` | `ParserErrorCode.EXPECTED_CASE_OR_DEFAULT` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:537`) | Expected 'case' or 'default'. | não verificado | **publicado** — `crates/frontend/src/parser/statements.rs:767` |
| `expected_list_or_map_literal` | `ParserErrorCode.EXPECTED_LIST_OR_MAP_LITERAL` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:609`) | Expected a list or map literal. | não verificado | **não implementado** |
| `experiment_not_enabled_off_by_default` | `ParserErrorCode.EXPERIMENT_NOT_ENABLED_OFF_BY_DEFAULT` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:714`) | This requires the experimental '{0}' language feature to be enabled. | não verificado | **publicado** — `crates/frontend/src/parser/mod.rs:613`, `crates/frontend/src/parser/types.rs:431` |
| `external_constructor_with_body` | `ParserErrorCode.EXTERNAL_CONSTRUCTOR_WITH_BODY` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:787`) | External constructors can't have a body. | não verificado | **não implementado** |
| `external_field` | `ParserErrorCode.EXTERNAL_FIELD` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:830`) | Fields can't be declared to be 'external'. | não verificado | **não implementado** |
| `external_getter_with_body` | `ParserErrorCode.EXTERNAL_GETTER_WITH_BODY` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:838`) | External getters can't have a body. | não verificado | **não implementado** |
| `external_operator_with_body` | `ParserErrorCode.EXTERNAL_OPERATOR_WITH_BODY` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:857`) | External operators can't have a body. | não verificado | **não implementado** |
| `external_setter_with_body` | `ParserErrorCode.EXTERNAL_SETTER_WITH_BODY` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:865`) | External setters can't have a body. | não verificado | **não implementado** |
| `factory_with_initializers` | `ParserErrorCode.FACTORY_WITH_INITIALIZERS` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:911`) | A 'factory' constructor can't have initializers. | não verificado | **não implementado** |
| `factory_without_body` | `ParserErrorCode.FACTORY_WITHOUT_BODY` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:905`) | A non-redirecting 'factory' constructor must have a body. | não verificado | **não implementado** |
| `final_constructor` | `ParserErrorCode.FINAL_CONSTRUCTOR` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:959`) | A constructor can't be declared to be 'final'. | não verificado | **não implementado** |
| `final_method` | `ParserErrorCode.FINAL_METHOD` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:971`) | Getters, setters and methods can't be declared to be 'final'. | não verificado | **não implementado** |
| `getter_in_function` | `ParserErrorCode.GETTER_IN_FUNCTION` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:1002`) | Getters can't be defined within methods or functions. | não verificado | **não implementado** |
| `getter_not_assignable_setter_types` | `CompileTimeErrorCode.GETTER_NOT_ASSIGNABLE_SETTER_TYPES` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/error/codes.g.dart:2202`) | The return type of getter '{0}' is '{1}' which isn't assignable to the type '{2}' of its setter '{3}'. | não verificado | **não implementado** |
| `inconsistent_case_expression_types` | `CompileTimeErrorCode.INCONSISTENT_CASE_EXPRESSION_TYPES` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/error/codes.g.dart:2439`) | Case expressions must have the same types, '{0}' isn't a '{1}'. | não verificado | **não implementado** |
| `invalid_cast_literal` | `CompileTimeErrorCode.INVALID_CAST_LITERAL` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/error/codes.g.dart:2721`) | The literal '{0}' with type '{1}' isn't of expected type '{2}'. | não verificado | **não implementado** |
| `invalid_comment_reference` | `ParserErrorCode.INVALID_COMMENT_REFERENCE` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:1117`) | Comment references should contain a possibly prefixed identifier and can start with 'new', but shouldn't contain anything else. | não verificado | **não implementado** |
| `invalid_star_after_async` | `ParserErrorCode.INVALID_STAR_AFTER_ASYNC` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:1237`) | The modifier 'async*' isn't allowed for an expression function body. | não verificado | **não implementado** |
| `invalid_sync` | `ParserErrorCode.INVALID_SYNC` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:1249`) | The modifier 'sync' isn't allowed for an expression function body. | não verificado | **não implementado** |
| `invalid_use_of_do_not_submit_member` | `WarningCode.invalid_use_of_do_not_submit_member` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/error/codes.g.dart:7682`) | Uses of '{0}' should not be submitted to source control. | não verificado | **não implementado** |
| `local_function_declaration_modifier` | `ParserErrorCode.LOCAL_FUNCTION_DECLARATION_MODIFIER` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:1339`) | Local function declarations can't specify any modifiers. | não verificado | **não implementado** |
| `missing_closing_parenthesis` | `ParserErrorCode.MISSING_CLOSING_PARENTHESIS` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:1373`) | The closing parenthesis is missing. | não verificado | **não implementado** |
| `missing_expression_in_initializer` | `ParserErrorCode.MISSING_EXPRESSION_IN_INITIALIZER` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:1394`) | Expected an expression after the assignment operator. | não verificado | **não implementado** |
| `missing_function_keyword` | `ParserErrorCode.MISSING_FUNCTION_KEYWORD` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:1417`) | Function types must have the keyword 'Function' before the parameter list. | não verificado | **não implementado** |
| `missing_get` | `ParserErrorCode.MISSING_GET` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:1430`) | Getters must have the keyword 'get' before the getter name. | não verificado | **não implementado** |
| `missing_name_for_named_parameter` | `ParserErrorCode.MISSING_NAME_FOR_NAMED_PARAMETER` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:1458`) | Named parameters in a function type must have a name | não verificado | **não implementado** |
| `missing_name_in_library_directive` | `ParserErrorCode.MISSING_NAME_IN_LIBRARY_DIRECTIVE` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:1466`) | Library directives must include a library name. | não verificado | **não implementado** |
| `missing_name_in_part_of_directive` | `ParserErrorCode.MISSING_NAME_IN_PART_OF_DIRECTIVE` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:1475`) | Part-of directives must include a library name. | não verificado | **não implementado** |
| `missing_terminator_for_parameter_group` | `ParserErrorCode.MISSING_TERMINATOR_FOR_PARAMETER_GROUP` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:1519`) | There is no '{0}' to close the parameter group. | não verificado | **não implementado** |
| `missing_variable_in_for_each` | `ParserErrorCode.MISSING_VARIABLE_IN_FOR_EACH` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:1532`) | A loop variable must be declared in a for-each loop before the 'in', but none was found. | não verificado | **não implementado** |
| `mixed_parameter_groups` | `ParserErrorCode.MIXED_PARAMETER_GROUPS` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:1539`) | Can't have both positional and named parameters in a single parameter list. | não verificado | **não implementado** |
| `multiple_named_parameter_groups` | `ParserErrorCode.MULTIPLE_NAMED_PARAMETER_GROUPS` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:1590`) | Can't have multiple groups of named parameters in a single parameter list. | não verificado | **não implementado** |
| `multiple_positional_parameter_groups` | `ParserErrorCode.MULTIPLE_POSITIONAL_PARAMETER_GROUPS` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:1611`) | Can't have multiple groups of positional parameters in a single parameter list. | não verificado | **não implementado** |
| `multiple_variables_in_for_each` | `ParserErrorCode.MULTIPLE_VARIABLES_IN_FOR_EACH` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:1628`) | A single loop variable must be declared in a for-each loop before the 'in', but {0} were found. | não verificado | **não implementado** |
| `named_function_type` | `ParserErrorCode.NAMED_FUNCTION_TYPE` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:1658`) | Function types can't be named. | não verificado | **não implementado** |
| `non_constructor_factory` | `ParserErrorCode.NON_CONSTRUCTOR_FACTORY` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:1694`) | Only a constructor can be declared to be a factory. | não verificado | **não implementado** |
| `non_identifier_library_name` | `ParserErrorCode.NON_IDENTIFIER_LIBRARY_NAME` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:1700`) | The name of a library must be an identifier. | não verificado | **não implementado** |
| `non_string_literal_as_uri` | `ParserErrorCode.NON_STRING_LITERAL_AS_URI` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:1714`) | The URI must be a string literal. | não verificado | **não implementado** |
| `non_user_definable_operator` | `ParserErrorCode.NON_USER_DEFINABLE_OPERATOR` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:1723`) | The operator '{0}' isn't user definable. | não verificado | **não implementado** |
| `normal_before_optional_parameters` | `ParserErrorCode.NORMAL_BEFORE_OPTIONAL_PARAMETERS` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:1728`) | Normal parameters must occur before optional parameters. | não verificado | **não implementado** |
| `object_cannot_extend_another_class` | `CompileTimeErrorCode.OBJECT_CANNOT_EXTEND_ANOTHER_CLASS` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/error/codes.g.dart:4301`) | The class 'Object' can't extend any other class. | não verificado | **não implementado** |
| `positional_parameter_outside_group` | `ParserErrorCode.POSITIONAL_PARAMETER_OUTSIDE_GROUP` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:1787`) | Positional parameters must be enclosed in square brackets ('[' and ']'). | não verificado | **não implementado** |
| `removed_lint_use` | `WarningCode.REMOVED_LINT_USE` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/error/codes.g.dart:7197`) | '{0}' was removed in Dart '{1}' | não verificado | **não implementado** |
| `replaced_lint_use` | `WarningCode.REPLACED_LINT_USE` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/error/codes.g.dart:7209`) | '{0}' was replaced by '{2}' in Dart '{1}'. | não verificado | **não implementado** |
| `setter_in_function` | `ParserErrorCode.SETTER_IN_FUNCTION` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:1872`) | Setters can't be defined within methods or functions. | não verificado | **não implementado** |
| `static_getter_without_body` | `ParserErrorCode.STATIC_GETTER_WITHOUT_BODY` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:1890`) | A 'static' getter must have a body. | não verificado | **não implementado** |
| `static_setter_without_body` | `ParserErrorCode.STATIC_SETTER_WITHOUT_BODY` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:1903`) | A 'static' setter must have a body. | não verificado | **não implementado** |
| `super_initializer_in_object` | `CompileTimeErrorCode.SUPER_INITIALIZER_IN_OBJECT` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/error/codes.g.dart:5056`) | The class 'Object' can't invoke a constructor from a superclass. | não verificado | **não implementado** |
| `type_arguments_on_type_variable` | `ParserErrorCode.TYPE_ARGUMENTS_ON_TYPE_VARIABLE` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:1939`) | Can't use type arguments with type variable '{0}'. | não verificado | **não implementado** |
| `unexpected_terminator_for_parameter_group` | `ParserErrorCode.UNEXPECTED_TERMINATOR_FOR_PARAMETER_GROUP` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:1968`) | There is no '{0}' to open a parameter group. | não verificado | **não implementado** |
| `unignorable_ignore` | `WarningCode.UNIGNORABLE_IGNORE` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/error/codes.g.dart:7382`) | The diagnostic '{0}' can't be ignored. | não verificado | **não implementado** |
| `unnecessary_ignore` | `WarningCode.UNNECESSARY_IGNORE` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/error/codes.g.dart:7416`) | The diagnostic '{0}' isn't produced at this location so it doesn't need to be ignored. | não verificado | **não implementado** |
| `var_class` | `ParserErrorCode.VAR_CLASS` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:2009`) | Classes can't be declared to be 'var'. | não verificado | **não implementado** |
| `var_enum` | `ParserErrorCode.VAR_ENUM` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:2015`) | Enums can't be declared to be 'var'. | não verificado | **não implementado** |
| `var_typedef` | `ParserErrorCode.VAR_TYPEDEF` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:2029`) | Typedefs can't be declared to be 'var'. | não verificado | **não implementado** |
| `wrong_terminator_for_parameter_group` | `ParserErrorCode.WRONG_TERMINATOR_FOR_PARAMETER_GROUP` | sem emissor localizado por grep no 3.6.2 (definição em `analyzer/lib/src/dart/error/syntactic_errors.g.dart:2059`) | Expected '{0}' to close parameter group. | não verificado | **não implementado** |

### II.5 — ErrorVerifier (error_verifier.dart)

Este lote cobre **75 constantes** cujo emissor principal é o `ErrorVerifier`
(`analyzer/lib/src/generated/error_verifier.dart`, 7191 linhas) e a classe auxiliar
`_MacroDiagnosticsReporter` do mesmo arquivo (`:6666-6904`): 69 `CompileTimeErrorCode`,
2 `StaticWarningCode`, 2 `ParserErrorCode` (os dois `native_*`, emitidos aqui e não pelo parser),
1 `WarningCode` e 1 `HintCode` (macros); 59 nomes emitidos distintos, pois vários compartilham o nome
(`built_in_identifier_in_declaration` ×6, `conflicting_type_variable_and_container` ×5,
`subtype_of_deferred_class` ×3, `external_with_initializer` ×3, `abstract_field_initializer` ×2,
`implements_super_class` ×2, `wrong_number_of_parameters_for_operator` ×2).
O `ErrorVerifier` é o **terceiro** passo de `LibraryAnalyzer._computeVerifyErrors`
(`analyzer/lib/src/dart/analysis/library_analyzer.dart:420-457`): `_computeConstantErrors`
(`ConstantVerifier`, :427) → `InheritanceOverrideVerifier.verifyUnit` (:432-436) → `ErrorVerifier`
(:441-451, um por unidade, `unit.accept(errorVerifier)`) → `FfiVerifier` (:455-456); ver T5.
Ele roda sobre a AST **já resolvida** (usa `staticType`, `staticElement`, `declaredElement`), não tem porta
de erro de sintaxe e relata tudo pelo `errorReporter` da unidade (os de macro podem relatar no
`errorReporter` de **outra** unidade, :6805, :6823). Estado no DartForge: **51 publicados**,
**24 não implementados** (8 de `augment`, 9 de macros — ambos experimentos desligados no 3.6.2, T1 —
e 7 regras comuns), nenhum "emitido, não publicado".

#### ErrorVerifier (`analyzer/lib/src/generated/error_verifier.dart`)

Mecanismo (só o que foi lido). O verificador é um `RecursiveAstVisitor` com estado de contexto; as
"portas" que decidem o que é pulado:

```text
construtor (:258-290)
  _isInSystemLibrary = _currentLibrary.source.uri.isScheme('dart')      (:277)
     -> desliga native_clause_in_non_sdk_code (:1284) e native_function_body_in_non_sdk_code (:4580);
        também desliga a checagem de "classe proibida" (:3366) e as de import/export interno (:3266, :3798)
visitCompilationUnit (:572-586)
  _featureSet = node.featureSet            (usado por generic_function_type_cannot_be_bound, :3737)
  _duplicateDefinitionVerifier.checkUnit; _checkForDeferredPrefixCollisions;
  _checkForIllegalLanguageOverride; GetterSetterTypesVerifier.checkStaticAccessors; super.visit…
visitClassDeclaration (:450-532)
  _checkAugmentations / _checkClassAugmentationModifiers / _checkAugmentationTypeParameters /
  _checkClassAugmentationTargetAlreadyHasExtendsClause            (só agem com `augment`)
  _isInNativeClass = node.nativeClause != null   (:478; zera no finally :529)
     -> _checkForFinalNotInitialized retorna cedo (:3577); constructorFieldsVerifier não recebe
        os construtores (:505-508)
  _enclosingClass = element.augmented.declaration   (:480-482; null no finally :530)
  built-in como nome (exceto a classe `Function` do core, :485-488)
  _checkForConflictingClassTypeVariableErrorCodes
  se há extends/with/implements (:495-497): moreChecks = _checkClassInheritance(...)
  … _checkForMixinClassErrorCodes (:519), _reportMacroDiagnostics (:520), super.visit…
_checkClassInheritance (:1871-1905)  -- a porta de hierarquia, com curto-circuito `&&`:
  se  !_checkForExtendsDisallowedClass(superclass)                 (não relata; só devolve bool)
   && !_checkForImplementsClauseErrorCodes(implements)             (relata IMPLEMENTS_DEFERRED_CLASS)
   && !_checkForAllMixinErrorCodes(with)                           (relata MIXIN_DEFERRED_CLASS,
                                         MIXIN_CLASS_DECLARES_CONSTRUCTOR, MIXIN_INHERITS_FROM_NOT_OBJECT…)
   && !_checkForNoGenerativeConstructorsInSuperclass(superclass)   (relata NO_GENERATIVE_CONSTRUCTORS…)
  então: EXTENDS_DEFERRED_CLASS; IMPLEMENTS_REPEATED; IMPLEMENTS_SUPER_CLASS; MIXINS_SUPER_CLASS;
         PRIVATE_COLLISION_IN_MIXIN_APPLICATION; CONFLICTING_GENERIC_INTERFACES; base/interface/final/
         sealed fora da biblioteca; CLASS_USED_AS_MIXIN; devolve true
  senão: devolve false (nada do bloco acima é checado)
_checkMixinInheritance (:6088-6115): porta = !_checkForOnClauseErrorCodes && !_checkForImplementsClauseErrorCodes;
  dentro: ON_REPEATED, IMPLEMENTS_REPEATED, …
contexto de executável: _withEnclosingExecutable (:6443-6456) troca _enclosingExecutable
  (EnclosingExecutableContext, :66-105: isAsynchronous = element.isAsynchronous)
acesso a `this`: _hasAccessToThis (:228) = _computeThisAccessForFunctionBody (:6255-6260):
  corpo de construtor não-factory -> true; corpo de método -> !isStatic; outro corpo -> herda;
  campo: !isStatic && isLate (:869)
```

Como o curto-circuito é por `&&`, um `implements` com tipo proibido/adiado **impede** a própria chamada de
`_checkForAllMixinErrorCodes` e de `_checkForNoGenerativeConstructorsInSuperclass` (e todos os relatos
deles). `_checkClassInheritance` é chamado: em classe só quando há alguma cláusula (:495-503); em
`ClassTypeAlias` sempre (:544-545); em enum quando há `with`/`implements`, com `superclass = null`
(:697-700). `_checkForExtendsOrImplementsDeferredClass` (:3337-3350) ignora `NamedType` sintético
e relata quando `namedType.isDeferred` — prefixo de import cujo `PrefixElement.imports` tem **exatamente
um** import e ele é `deferred` (`analyzer/lib/src/dart/ast/ast.dart:12695-12705`).

**Declarações de classe, mixin, enum, extension e extension type (17)**

| código | constante | emissor (arquivo:linha, método) | condição | posição | DartForge |
|---|---|---|---|---|---|
| `subtype_of_deferred_class` | `CompileTimeErrorCode.EXTENDS_DEFERRED_CLASS` | `analyzer/lib/src/generated/error_verifier.dart:3312` `ErrorVerifier._checkForExtendsDeferredClass` → `_checkForExtendsOrImplementsDeferredClass` (:3337-3350); chamado em `_checkClassInheritance` :1884 | depois da porta de hierarquia: superclasse não sintética com `namedType.isDeferred` | `atNode(superclass)` (o `NamedType` inteiro, com prefixo e argumentos de tipo) | **publicado** — `crates/analise/src/clausulas.rs:1434` (`verificar`, depois da porta; `Leitor::adiado` :200-210 usa `any` sobre os imports do prefixo, o analyzer exige exatamente um import) |
| `subtype_of_deferred_class` | `CompileTimeErrorCode.IMPLEMENTS_DEFERRED_CLASS` | `analyzer/lib/src/generated/error_verifier.dart:3785` `ErrorVerifier._checkForImplementsClauseErrorCodes` (classe, enum, mixin) e `:3433` `_checkForExtensionTypeImplementsDeferred` (extension type) | para cada tipo de `implements`: não é classe proibida (`else if`, :3781-3787) e `isDeferred`; em extension type, todo tipo de `implements` com `isDeferred` | `atNode(namedType)` | **publicado** — `crates/analise/src/clausulas.rs:673` (`implements`) |
| `implements_repeated` | `CompileTimeErrorCode.IMPLEMENTS_REPEATED` | `analyzer/lib/src/generated/error_verifier.dart:5187-5191` `ErrorVerifier._checkForRepeatedType`; chamado em `visitExtensionTypeDeclaration` :819-823, `_checkClassInheritance` :1885-1889, `_checkMixinInheritance` :6103-6107 | o elemento do `InterfaceType` já está no conjunto `libraryContext.setOfImplements(declaração)` (conjunto por declaração, compartilhado entre a declaração e as suas augmentations, :6657-6659); em classe/mixin só depois da porta | `atNode(namedType)` da segunda ocorrência em diante; `{0}` = `element.name` | **publicado** — `crates/analise/src/clausulas.rs:1215`, `:1403`, `:1408` (`repetidos`, :1167) |
| `implements_super_class` | `CompileTimeErrorCode.IMPLEMENTS_SUPER_CLASS` | `analyzer/lib/src/generated/error_verifier.dart:6032` `ErrorVerifier._checkImplementsSuperClass` (:6017-6037); chamado em `_checkClassInheritance` :1890 | depois da porta: tipo de `implements` é `InterfaceType` cujo elemento é o de `_enclosingClass.supertype` | `atNode(interfaceNode)`; `{0}` é o **Element** da superclasse, convertido por `getDisplayString()` (`analyzer/lib/error/listener.dart:338-346`) | **publicado** — `crates/analise/src/clausulas.rs:1418` (`exibir_classe` :405 monta o `{0}`) |
| `implements_super_class` | `CompileTimeErrorCode.MIXINS_SUPER_CLASS` | `analyzer/lib/src/generated/error_verifier.dart:6136` `ErrorVerifier._checkMixinsSuperClass` (:6121-6141); chamado em `_checkClassInheritance` :1891 | depois da porta: tipo do `with` é `InterfaceType` cujo elemento é o da superclasse | `atNode(mixinNode)`; `{0}` = Element da superclasse por `getDisplayString()` | **publicado** — `crates/analise/src/clausulas.rs:1418` (mesmo laço, com `cl.with`) |
| `mixin_class_declaration_extends_not_object` | `CompileTimeErrorCode.MIXIN_CLASS_DECLARATION_EXTENDS_NOT_OBJECT` | `analyzer/lib/src/generated/error_verifier.dart:4347` e `:4354` `ErrorVerifier._checkForMixinClassErrorCodes` (:4320-4359); chamado em `visitClassDeclaration` :519 e `visitClassTypeAlias` :547-548 | elemento é `mixin class`: (a) há `extends` e o tipo não é `Object`; senão (b) há `with` e não é (aplicação de mixin com menos de 2 mixins) | (a) `atNode(superclass)`; (b) `atNode(withClause)` (do `with` ao último mixin); `{0}` = nome da classe | **publicado** — `crates/analise/src/clausulas.rs:872`, `:887` (`classe_mixin`) |
| `mixin_class_declares_constructor` | `CompileTimeErrorCode.MIXIN_CLASS_DECLARES_CONSTRUCTOR` | `analyzer/lib/src/generated/error_verifier.dart:4306` `ErrorVerifier._checkForMixinClassDeclaresConstructor` (uso num `with`, via `_checkForAllMixinErrorCodes` :2009-2011) e `:4336` `_checkForMixinClassErrorCodes` (declaração) | uso: o elemento misturado não é `MixinElement` nem `mixin class` e tem construtor não sintético e não factory (para no primeiro); declaração: `mixin class` com construtor gerador não trivial (`isTrivial`: sem redirecionamento, sem parâmetros, sem inicializadores, corpo `;`, não `external` — `analyzer/lib/src/dart/ast/ast.dart:4307-4312`) | uso: `atNode(mixinName)` com `{0}` = nome do mixin; declaração: `atNode(member.returnType)` (o identificador do nome da classe no construtor) com `{0}` = nome da classe | **publicado** — `crates/analise/src/clausulas.rs:774` (`mixins`), `:851` (`classe_mixin`) |
| `subtype_of_deferred_class` | `CompileTimeErrorCode.MIXIN_DEFERRED_CLASS` | `analyzer/lib/src/generated/error_verifier.dart:1993` `ErrorVerifier._checkForAllMixinErrorCodes` (:1974-2022) | tipo do `with` é `InterfaceType`, não é classe proibida e `isDeferred`; os demais testes do mesmo mixin (restrições, construtor, herança) continuam depois do relato | `atNode(mixinName)` | **publicado** — `crates/analise/src/clausulas.rs:717` (`mixins`) |
| `mixin_inherits_from_not_object` | `CompileTimeErrorCode.MIXIN_INHERITS_FROM_NOT_OBJECT` | `analyzer/lib/src/generated/error_verifier.dart:4384` `ErrorVerifier._checkForMixinInheritsNotFromObject` (:4367-4388); chamado em `_checkForAllMixinErrorCodes` :2014 | o misturado é `ClassElement` (não `mixin`, não extension type) e não vale: (supertipo nulo ou `Object`) e (sem mixins, ou é aplicação de mixin com menos de 2 mixins) | `atNode(mixinName)`; `{0}` = nome da classe misturada | **publicado** — `crates/analise/src/clausulas.rs:799` (`mixins`) |
| `mixin_super_class_constraint_deferred_class` | `CompileTimeErrorCode.MIXIN_SUPER_CLASS_CONSTRAINT_DEFERRED_CLASS` | `analyzer/lib/src/generated/error_verifier.dart:4937` `ErrorVerifier._checkForOnClauseErrorCodes` (:4920-4944); chamado em `_checkMixinInheritance` :6095 | tipo do `on` é `InterfaceType`, não é classe proibida e `isDeferred` | `atNode(namedType)` | **publicado** — `crates/analise/src/clausulas.rs:570` (`avaliar`) |
| `no_generative_constructors_in_superclass` | `CompileTimeErrorCode.NO_GENERATIVE_CONSTRUCTORS_IN_SUPERCLASS` | `analyzer/lib/src/generated/error_verifier.dart:4702` `ErrorVerifier._checkForNoGenerativeConstructorsInSuperclass` (:4680-4708); 4º termo da porta :1883 | `_enclosingClass.supertype` não nulo; a classe tem algum construtor não factory; a superclasse tem construtores (lista não vazia) e **todos** são factory | `atNode(superclass)`; `{0}` = nome da classe, `{1}` = nome da superclasse | **publicado** — `crates/analise/src/clausulas.rs:637` (`avaliar`) |
| `on_repeated` | `CompileTimeErrorCode.ON_REPEATED` | `analyzer/lib/src/generated/error_verifier.dart:5187-5191` `ErrorVerifier._checkForRepeatedType`; chamado em `_checkMixinInheritance` :6098-6102 | depois da porta do mixin: elemento do tipo do `on` já está em `libraryContext.setOfOn(declaração)` | `atNode(namedType)` repetido; `{0}` = `element.name` | **publicado** — `crates/analise/src/clausulas.rs:1402` |
| `private_collision_in_mixin_application` | `CompileTimeErrorCode.PRIVATE_COLLISION_IN_MIXIN_APPLICATION` | `analyzer/lib/src/generated/error_verifier.dart:4517` e `:4537` `ErrorVerifier._checkForMixinWithConflictingPrivateMember` (:4491-4574); chamado em `_checkClassInheritance` :1892 | depois da porta: mixin de **outra** biblioteca declara membro de instância privado cujo nome já foi trazido por um mixin anterior da mesma biblioteca (:4510-4521) ou existe como membro concreto herdado da superclasse declarada (:4523-4545); só o primeiro conflito do `with` é relatado | `atNode(namedType)` do mixin; `{0}` nome sem o `=` final, `{1}` lexema do mixin, `{2}` lexema do mixin anterior ou nome da classe que declara o herdado | **não implementado** — especificação completa abaixo |
| `extension_declares_member_of_object` | `CompileTimeErrorCode.EXTENSION_DECLARES_MEMBER_OF_OBJECT` | `analyzer/lib/src/generated/error_verifier.dart:3379` `ErrorVerifier._checkForExtensionDeclaresMemberOfObject` (:3374-3392); chamado em `visitMethodDeclaration` :1178 | `_enclosingExtension != null` e o nome do método/acessor/operador está em {`==`, `hashCode`, `toString`, `runtimeType`, `noSuchMethod`} (`analyzer/lib/src/dart/ast/extensions.dart:290-298`); não há teste de `static` | `atToken(node.name)` | **publicado** — `crates/analise/src/membros.rs:943` |
| `extension_type_declares_instance_field` | `CompileTimeErrorCode.EXTENSION_TYPE_DECLARES_INSTANCE_FIELD` | `analyzer/lib/src/generated/error_verifier.dart:3417` `ErrorVerifier._checkForExtensionTypeDeclaresInstanceField` (:3405-3420); chamado em `visitFieldDeclaration` :870. A mensagem do parser (`_fe_analyzer_shared/lib/src/parser/parser_impl.dart:3825`) é descartada em `analyzer/lib/src/fasta/error_converter.dart:529-532` | `_enclosingClass` é extension type e a declaração de campo não é `static` nem `external` | `atToken(field.name)` — um relato **por variável** da lista | **publicado** — `crates/analise/src/membros.rs:919` |
| `extension_type_with_abstract_member` | `CompileTimeErrorCode.EXTENSION_TYPE_WITH_ABSTRACT_MEMBER` | `analyzer/lib/src/generated/error_verifier.dart:3523` `ErrorVerifier._checkForExtensionTypeWithAbstractMember` (:3515-3529); chamado em `visitExtensionTypeDeclaration` :838. A mensagem do parser (`_fe_analyzer_shared/lib/src/parser/parser_impl.dart:5088`) é descartada em `analyzer/lib/src/fasta/error_converter.dart:526-528` | membro `MethodDeclaration` não estático e `isAbstract` | `atNode(member)` — a declaração inteira, **incluindo comentário de documentação e anotações** (`AnnotatedNodeImpl.beginToken`, `analyzer/lib/src/dart/ast/ast.dart:148-163`); `{0}` lexema do nome do membro, `{1}` nome do extension type | **publicado** — `crates/analise/src/membros.rs:967` |
| `non_final_field_in_enum` | `CompileTimeErrorCode.NON_FINAL_FIELD_IN_ENUM` | `analyzer/lib/src/generated/error_verifier.dart:4795` `ErrorVerifier._checkForNonFinalFieldInEnum` (:4782-4797); chamado em `visitFieldDeclaration` :874 | campo não `static`, lista com `isFinal` falso e `_enclosingClass` é enum | `atToken(variables.first.name)` — só a **primeira** variável da lista | **publicado** — `crates/analise/src/membros.rs:930` |

**Augmentations (8)** — todas dependem de `augmentKeyword != null`, isto é, do experimento de augmentations/macros; sem ele o parser não produz o nó com `augment` (T1). `_checkAugmentations` (:1644-1678) é chamado para classe (:454), construtor (:608), constante de enum (:659), enum (:674), extension (:757), extension type (:799), campo (:879), função de topo (:967), typedef genérico (:1042), método (:1182), mixin (:1217) e variável de topo (:1565).

| código | constante | emissor (arquivo:linha, método) | condição | posição | DartForge |
|---|---|---|---|---|---|
| `augmentation_extends_clause_already_present` | `CompileTimeErrorCode.AUGMENTATION_EXTENDS_CLAUSE_ALREADY_PRESENT` | `analyzer/lib/src/generated/error_verifier.dart:1849` `ErrorVerifier._checkClassAugmentationTargetAlreadyHasExtendsClause` (:1836-1864); chamado em `visitClassDeclaration` :473-476 | a declaração tem `extends` e algum elemento da cadeia `augmentationTarget` tem `hasExtendsClause` (para no primeiro) | `atToken(extendsClause.extendsKeyword)`; `contextMessages`: um, no nome do alvo (`nameOffset`/`nameLength`), texto `The extends clause is included here.` | **não implementado** — experimento desligado no 3.6.2 (T1); nenhum emissor em `crates/` |
| `augmentation_modifier_extra` | `CompileTimeErrorCode.AUGMENTATION_MODIFIER_EXTRA` | `analyzer/lib/src/generated/error_verifier.dart:1789` `ErrorVerifier._checkClassAugmentationModifiers` (:1756-1834) e `:6072` `_checkMixinAugmentationModifiers` (:6039-6084) | augmentation com alvo: a declaração **não** tem o modificador e a augmentation tem; classe: `abstract` (pulado se a declaração é `sealed`), `base`, `final`, `interface`, `mixin`, `sealed`; mixin: só `base` | `atToken(augmentationModifier)`; `{0}` = nome do modificador | **não implementado** — idem |
| `augmentation_modifier_missing` | `CompileTimeErrorCode.AUGMENTATION_MODIFIER_MISSING` | `analyzer/lib/src/generated/error_verifier.dart:1781` `ErrorVerifier._checkClassAugmentationModifiers` e `:6064` `_checkMixinAugmentationModifiers` | augmentation com alvo: a declaração tem o modificador e a augmentation não | `atToken(augmentKeyword)`; `{0}` = nome do modificador (um relato por modificador faltante, todos no mesmo token) | **não implementado** — idem |
| `augmentation_of_different_declaration_kind` | `CompileTimeErrorCode.AUGMENTATION_OF_DIFFERENT_DECLARATION_KIND` | `analyzer/lib/src/generated/error_verifier.dart:1665` `ErrorVerifier._checkAugmentations` (:1644-1678) | há `augment`, o elemento é `AugmentableElement`, `augmentationTarget == null` e `augmentationTargetAny != null` | `atToken(augmentKeyword)`; `{0}` = `target.kind.displayName`, `{1}` = `element.kind.displayName` | **não implementado** — idem |
| `augmentation_type_parameter_bound` | `CompileTimeErrorCode.AUGMENTATION_TYPE_PARAMETER_BOUND` | `analyzer/lib/src/generated/error_verifier.dart:1731`, `:1736`, `:1746` `ErrorVerifier._checkAugmentationTypeParameters` (:1680-1754) | mesma contagem e mesmo nome no índice: declaração sem limite e augmentation com; declaração com e augmentation sem; ambos com e `!typeSystem.isEqualTo` | `atNode(augmentationBound)` nos casos 1 e 3; `atToken(ofAugmentation.name)` no caso 2 | **não implementado** — idem |
| `augmentation_type_parameter_count` | `CompileTimeErrorCode.AUGMENTATION_TYPE_PARAMETER_COUNT` | `analyzer/lib/src/generated/error_verifier.dart:1689`, `:1696`, `:1705`, `:1710` `ErrorVerifier._checkAugmentationTypeParameters`; chamado quando `element.augmentedIfReally != null` (:465-471, :679-685, :762-770, :804-810, :1228-1234) | declaração sem parâmetros de tipo e augmentation com lista; declaração com e augmentation sem lista; lista menor; lista maior | respectivamente `atToken(leftBracket)`; `atToken(nameToken)`; `atToken(rightBracket)`; `atToken(typeParameters[declarationCount].name)` (o primeiro excedente) | **não implementado** — idem |
| `augmentation_type_parameter_name` | `CompileTimeErrorCode.AUGMENTATION_TYPE_PARAMETER_NAME` | `analyzer/lib/src/generated/error_verifier.dart:1720` `ErrorVerifier._checkAugmentationTypeParameters` | mesma contagem e `ofAugmentation.name.lexeme != ofDeclaration.name` (aí o limite desse índice não é checado, `continue` :1722) | `atToken(ofAugmentation.name)` | **não implementado** — idem |
| `augmentation_without_declaration` | `CompileTimeErrorCode.AUGMENTATION_WITHOUT_DECLARATION` | `analyzer/lib/src/generated/error_verifier.dart:1676` `ErrorVerifier._checkAugmentations` | há `augment`, elemento aumentável, sem `augmentationTarget` e sem `augmentationTargetAny` | `atToken(augmentKeyword)` | **não implementado** — idem |

**Construtores e inicializadores (10)**

| código | constante | emissor (arquivo:linha, método) | condição | posição | DartForge |
|---|---|---|---|---|---|
| `abstract_field_initializer` | `CompileTimeErrorCode.ABSTRACT_FIELD_CONSTRUCTOR_INITIALIZER` | `analyzer/lib/src/generated/error_verifier.dart:1931` `ErrorVerifier._checkForAbstractOrExternalFieldConstructorInitializer` (:1926-1940); chamado em `visitConstructorFieldInitializer` :624-627 e `visitFieldFormalParameter` :900-907 | o campo inicializado (`x = …` na lista de inicializadores, ou `this.x` formal) tem `isAbstract` | `atToken(identifier)`: o nome do campo no inicializador, ou `node.name` do formal `this.x` | **publicado** — `crates/analise/src/externos.rs:80`, `:97` (`inicializadores`) |
| `external_with_initializer` | `CompileTimeErrorCode.EXTERNAL_FIELD_CONSTRUCTOR_INITIALIZER` | `analyzer/lib/src/generated/error_verifier.dart:1937` `ErrorVerifier._checkForAbstractOrExternalFieldConstructorInitializer` | idem com `fieldElement.isExternal` (os dois testes são independentes, não `else`) | `atToken(identifier)` | **publicado** — `crates/analise/src/externos.rs:74`, `:91` |
| `assert_in_redirecting_constructor` | `CompileTimeErrorCode.ASSERT_IN_REDIRECTING_CONSTRUCTOR` | `analyzer/lib/src/generated/error_verifier.dart:2772` `ErrorVerifier._checkForConflictingInitializerErrorCodes` (:2684-2796); chamado em `visitConstructorDeclaration` :601 | `_enclosingClass != null`, a lista de inicializadores tem pelo menos um `this(...)`/`this.n(...)` e o inicializador é `AssertInitializer` (um relato por assert) | `atNode(initializer)` | **publicado** — `crates/analise/src/membros.rs:326` (`inicializadores`) |
| `field_initializer_factory_constructor` | `CompileTimeErrorCode.FIELD_INITIALIZER_FACTORY_CONSTRUCTOR` | `analyzer/lib/src/generated/error_verifier.dart:3549` `ErrorVerifier._checkForFieldInitializingFormalRedirectingConstructor` (:3535-3569); chamado em `visitFieldFormalParameter` :898 | formal `this.x` cujo construtor (pai da lista de parâmetros, subindo um nível quando o pai do formal não é a lista) tem `factoryKeyword`; retorna antes do teste de redirecionamento | `atNode(parameter)` — o `FieldFormalParameter` | **publicado** — `crates/analise/src/membros.rs:376` (`construtor`) |
| `field_initializer_redirecting_constructor` | `CompileTimeErrorCode.FIELD_INITIALIZER_REDIRECTING_CONSTRUCTOR` | `analyzer/lib/src/generated/error_verifier.dart:2766` `ErrorVerifier._checkForConflictingInitializerErrorCodes` e `:3558` `_checkForFieldInitializingFormalRedirectingConstructor`. O caso de `analyzer/lib/src/fasta/error_converter.dart:144-151` só é alcançado se o parser emitir `RedirectingConstructorWithAnotherInitializer`; o dump não achou sítio no parser (não verificado) | (a) inicializador `x = e` num construtor que tem `this(...)`; (b) formal `this.x` num construtor não factory cuja lista tem `RedirectingConstructorInvocation` | (a) `atNode(initializer)` (o `ConstructorFieldInitializer` inteiro); (b) `atNode(parameter)` | **publicado** — `crates/analise/src/membros.rs:323` (a), `:378` (b) |
| `multiple_redirecting_constructor_invocations` | `CompileTimeErrorCode.MULTIPLE_REDIRECTING_CONSTRUCTOR_INVOCATIONS` | `analyzer/lib/src/generated/error_verifier.dart:2699` `ErrorVerifier._checkForConflictingInitializerErrorCodes` | segundo `this(...)` em diante na mesma lista de inicializadores (`redirectingInitializerCount > 0`) | `atNode(initializer)` de cada redirecionamento extra | **publicado** — `crates/analise/src/membros.rs:293` |
| `super_in_redirecting_constructor` | `CompileTimeErrorCode.SUPER_IN_REDIRECTING_CONSTRUCTOR` | `analyzer/lib/src/generated/error_verifier.dart:2759` `ErrorVerifier._checkForConflictingInitializerErrorCodes`; o caso de `analyzer/lib/src/fasta/error_converter.dart:428-434` não tem sítio do parser no dump (não verificado) | há redirecionamento `this(...)`, o inicializador é `super(...)` e a classe **não** é enum (em enum sai `super_in_enum_constructor`, :2735-2739) | `atNode(initializer)` | **publicado** — `crates/analise/src/membros.rs:320` |
| `instantiate_enum` | `CompileTimeErrorCode.INSTANTIATE_ENUM` | `analyzer/lib/src/generated/error_verifier.dart:3964` `ErrorVerifier._checkForInvalidGenerativeConstructorReference` (:3951-3968); chamado em `visitConstructorReference` :637 e `visitInstanceCreationExpression` :1107 | construtor resolvido, gerador, de enum, e a biblioteca **não** tem `Feature.enhanced_enums` (linguagem anterior à 2.17, `analyzer/lib/src/dart/analysis/experiments.g.dart:246-253`) | `atNode(node.type)` — só o `NamedType`, sem `.nome` | **não implementado** — só bibliotecas com linguagem menor que 2.17; nenhum emissor em `crates/` |
| `invalid_reference_to_generative_enum_constructor` | `CompileTimeErrorCode.INVALID_REFERENCE_TO_GENERATIVE_ENUM_CONSTRUCTOR` | `analyzer/lib/src/generated/error_verifier.dart:3959` `ErrorVerifier._checkForInvalidGenerativeConstructorReference` | mesmo teste, com `enhanced_enums` ligado: `ConstructorName` (em criação de instância ou em tear-off) cujo `staticElement` é construtor gerador de enum | `atNode(node)` — o `ConstructorName` inteiro | **publicado** — emitido por texto em `crates/types/src/inferencia/chamadas.rs:628`, `:686`, `:1533`, `crates/types/src/inferencia/expr.rs:1779`, `crates/types/src/inferencia/funcoes.rs:232`, `:237` (`inf.aviso` com o molde de `crates/types/src/codes.rs:100`, codificado pela ponte `crates/paridade/src/ponte.rs:83`) |
| `const_deferred_class` | `CompileTimeErrorCode.CONST_DEFERRED_CLASS` | `analyzer/lib/src/generated/error_verifier.dart:2927` `ErrorVerifier._checkForConstDeferredClass` (:2922-2930); chamado em `visitInstanceCreationExpression` :1116 | criação com `node.isConst`, tipo `InterfaceType` e `namedType.isDeferred` | `atNode(constructorName)` — `p.C` ou `p.C.named` inteiro | **não implementado** — especificação completa abaixo |

**Membros e modificadores (12)**

| código | constante | emissor (arquivo:linha, método) | condição | posição | DartForge |
|---|---|---|---|---|---|
| `abstract_field_initializer` | `CompileTimeErrorCode.ABSTRACT_FIELD_INITIALIZER` | `analyzer/lib/src/generated/error_verifier.dart:1950` `ErrorVerifier._checkForAbstractOrExternalVariableInitializer` (:1942-1968); chamado em `visitVariableDeclaration` :1606 | variável com inicializador cujo elemento é `FieldElement` com `isAbstract` | `atToken(node.name)` | **publicado** — `crates/analise/src/externos.rs:61` |
| `external_with_initializer` | `CompileTimeErrorCode.EXTERNAL_FIELD_INITIALIZER` | `analyzer/lib/src/generated/error_verifier.dart:1956` `ErrorVerifier._checkForAbstractOrExternalVariableInitializer` | idem com `FieldElement.isExternal` (independente do teste de `abstract`) | `atToken(node.name)` | **publicado** — `crates/analise/src/externos.rs:58` |
| `external_with_initializer` | `CompileTimeErrorCode.EXTERNAL_VARIABLE_INITIALIZER` | `analyzer/lib/src/generated/error_verifier.dart:1963` `ErrorVerifier._checkForAbstractOrExternalVariableInitializer` | variável de topo (`TopLevelVariableElement`) `external` com inicializador | `atToken(node.name)` | **publicado** — `crates/analise/src/externos.rs:27` |
| `const_instance_field` | `CompileTimeErrorCode.CONST_INSTANCE_FIELD` | `analyzer/lib/src/generated/error_verifier.dart:863` `ErrorVerifier.visitFieldDeclaration` (:854-892) | declaração de campo não `static` com `fields.isConst` | `atToken(fields.keyword)` — a palavra `const` | **publicado** — `crates/analise/src/membros.rs:912` |
| `non_void_return_for_operator` | `CompileTimeErrorCode.NON_VOID_RETURN_FOR_OPERATOR` | `analyzer/lib/src/generated/error_verifier.dart:4815` `ErrorVerifier._checkForNonVoidReturnTypeForOperator` (:4803-4819); chamado em `visitMethodDeclaration` :1176 (sempre, mesmo com aridade errada) | operador de nome `[]=` com tipo de retorno escrito cujo tipo resolvido não é `VoidType` | `atNode(returnType)` | **publicado** — `crates/analise/src/operadores.rs:83` (`aridade`) |
| `non_void_return_for_setter` | `CompileTimeErrorCode.NON_VOID_RETURN_FOR_SETTER` | `analyzer/lib/src/generated/error_verifier.dart:4831` `ErrorVerifier._checkForNonVoidReturnTypeForSetter` (:4825-4835); chamado em `visitFunctionDeclaration` :961 e `visitMethodDeclaration` :1167 | setter (de topo ou membro) com tipo de retorno escrito que não resolve para `VoidType` | `atNode(namedType)` (a anotação de retorno) | **publicado** — `crates/analise/src/membros.rs:176` (`setter`) |
| `optional_parameter_in_operator` | `CompileTimeErrorCode.OPTIONAL_PARAMETER_IN_OPERATOR` | `analyzer/lib/src/generated/error_verifier.dart:4964` `ErrorVerifier._checkForOptionalParameterInOperator` (:4953-4968); chamado em `visitMethodDeclaration` :1171-1175 | operador **sem** erro de aridade (`_checkForWrongNumberOfParametersForOperator` devolveu `false`) com parâmetro `isOptional`; um relato por parâmetro opcional | `atNode(formalParameter)` | **publicado** — `crates/analise/src/operadores.rs:74` |
| `wrong_number_of_parameters_for_operator` | `CompileTimeErrorCode.WRONG_NUMBER_OF_PARAMETERS_FOR_OPERATOR` | `analyzer/lib/src/generated/error_verifier.dart:5810` `ErrorVerifier._checkForWrongNumberOfParametersForOperator` (:5771-5823) | lista de parâmetros presente e contagem diferente do esperado: `[]=` 2; `<` `>` `<=` `>=` `==` `+` `/` `~/` `*` `%` `∣` `^` `&` `<<` `>>` `>>>` `[]` 1; `~` 0 | `atToken(declaration.name)`; `{0}` nome, `{1}` esperado, `{2}` contagem (inteiros) | **publicado** — `crates/analise/src/operadores.rs:50` |
| `wrong_number_of_parameters_for_operator` | `CompileTimeErrorCode.WRONG_NUMBER_OF_PARAMETERS_FOR_OPERATOR_MINUS` | `analyzer/lib/src/generated/error_verifier.dart:5817` `ErrorVerifier._checkForWrongNumberOfParametersForOperator` | operador `-` com mais de 1 parâmetro | `atToken(declaration.name)`; `{0}` = contagem | **publicado** — `crates/analise/src/operadores.rs:62` |
| `wrong_number_of_parameters_for_setter` | `CompileTimeErrorCode.WRONG_NUMBER_OF_PARAMETERS_FOR_SETTER` | `analyzer/lib/src/generated/error_verifier.dart:5843` `ErrorVerifier._checkForWrongNumberOfParametersForSetter` (:5833-5846); chamado em `visitFunctionDeclaration` :959-960 e `visitMethodDeclaration` :1166; o caso de `analyzer/lib/src/fasta/error_converter.dart:492-498` não tem sítio do parser no dump (não verificado) | setter com lista de parâmetros cuja contagem não é 1, ou cujo único parâmetro não é `isRequiredPositional` | `atToken(setterName)` | **publicado** — `crates/analise/src/membros.rs:171` (`setter`) |
| `native_clause_in_non_sdk_code` | `ParserErrorCode.NATIVE_CLAUSE_IN_NON_SDK_CODE` | `analyzer/lib/src/generated/error_verifier.dart:1287` `ErrorVerifier.visitNativeClause` (:1281-1291) | `NativeClause` em biblioteca cujo URI não é `dart:` (`!_isInSystemLibrary`) | `atNode(node)` — o `NativeClause` inteiro | **publicado** — `crates/analise/src/nativos.rs:107` (`fora_do_sdk`); é `SyntacticError` na tabela (`crates/diagnostics/src/codigos_g.rs:976`), publicado por `crates/analise/src/publicacao.rs:29` sem passar por `verificados.txt` |
| `native_function_body_in_non_sdk_code` | `ParserErrorCode.NATIVE_FUNCTION_BODY_IN_NON_SDK_CODE` | `analyzer/lib/src/generated/error_verifier.dart:4583` `ErrorVerifier._checkForNativeFunctionBodyInNonSdkCode` (:4579-4586); chamado em `visitNativeFunctionBody` :1295 | `NativeFunctionBody` fora de biblioteca `dart:` | `atNode(body)` — o `NativeFunctionBody` inteiro | **publicado** — `crates/analise/src/nativos.rs:117`; `SyntacticError` (`crates/diagnostics/src/codigos_g.rs:978`), mesma regra de publicação |

#### Constantes do lote `S1` ainda sem leitura do emissor (28 de 75)

As linhas abaixo foram **geradas por script** (mesmo método dos lotes automáticos: "condição" = mensagem oficial; "posição" não verificada).

##### `analyzer/lib/src/generated/error_verifier.dart` (27)

| código | constante | emissor (arquivo:linha, método) | condição (mensagem oficial) | posição | DartForge |
|---|---|---|---|---|---|
| `built_in_identifier_in_declaration` | `CompileTimeErrorCode.BUILT_IN_IDENTIFIER_AS_EXTENSION_NAME` | `analyzer/lib/src/generated/error_verifier.dart:784` `ErrorVerifier.visitExtensionDeclaration` | The built-in identifier '{0}' can't be used as an extension name. | não verificado | **publicado** — `crates/analise/src/membros.rs:661` |
| `built_in_identifier_in_declaration` | `CompileTimeErrorCode.BUILT_IN_IDENTIFIER_AS_EXTENSION_TYPE_NAME` | `analyzer/lib/src/generated/error_verifier.dart:815` `ErrorVerifier.visitExtensionTypeDeclaration` | The built-in identifier '{0}' can't be used as an extension type name. | não verificado | **publicado** — `crates/analise/src/membros.rs:662` |
| `built_in_identifier_in_declaration` | `CompileTimeErrorCode.BUILT_IN_IDENTIFIER_AS_PREFIX_NAME` | `analyzer/lib/src/generated/error_verifier.dart:1069` `ErrorVerifier.visitImportDirective` | The built-in identifier '{0}' can't be used as a prefix name. | não verificado | **publicado** — `crates/analise/src/membros.rs:649` |
| `built_in_identifier_in_declaration` | `CompileTimeErrorCode.BUILT_IN_IDENTIFIER_AS_TYPEDEF_NAME` | `analyzer/lib/src/generated/error_verifier.dart:541` `ErrorVerifier.visitClassTypeAlias`; `analyzer/lib/src/generated/error_verifier.dart:1018` `ErrorVerifier.visitFunctionTypeAlias`; `analyzer/lib/src/generated/error_verifier.dart:1048` `ErrorVerifier.visitGenericTypeAlias` | The built-in identifier '{0}' can't be used as a typedef name. | não verificado | **publicado** — `crates/analise/src/membros.rs:656`, `crates/analise/src/membros.rs:660` |
| `built_in_identifier_in_declaration` | `CompileTimeErrorCode.BUILT_IN_IDENTIFIER_AS_TYPE_NAME` | `analyzer/lib/src/generated/error_verifier.dart:487` `ErrorVerifier.visitClassDeclaration`; `analyzer/lib/src/generated/error_verifier.dart:692` `ErrorVerifier.visitEnumDeclaration`; `analyzer/lib/src/generated/error_verifier.dart:1242` `ErrorVerifier.visitMixinDeclaration` | The built-in identifier '{0}' can't be used as a type name. | não verificado | **publicado** — `crates/analise/src/membros.rs:657`, `crates/analise/src/membros.rs:658` |
| `built_in_identifier_in_declaration` | `CompileTimeErrorCode.BUILT_IN_IDENTIFIER_AS_TYPE_PARAMETER_NAME` | `analyzer/lib/src/generated/error_verifier.dart:1587` `ErrorVerifier.visitTypeParameter` | The built-in identifier '{0}' can't be used as a type parameter name. | não verificado | **publicado** — `crates/analise/src/membros.rs:681` |
| `conflicting_type_variable_and_container` | `CompileTimeErrorCode.CONFLICTING_TYPE_VARIABLE_AND_CLASS` | `analyzer/lib/src/generated/error_verifier.dart:2543` `ErrorVerifier._checkForConflictingClassTypeVariableErrorCodes` | '{0}' can't be used to name both a type parameter and the class in which the type parameter is defined. | não verificado | **publicado** — `crates/analise/src/membros.rs:861` |
| `conflicting_type_variable_and_container` | `CompileTimeErrorCode.CONFLICTING_TYPE_VARIABLE_AND_ENUM` | `analyzer/lib/src/generated/error_verifier.dart:2576` `ErrorVerifier._checkForConflictingEnumTypeVariableErrorCodes` | '{0}' can't be used to name both a type parameter and the enum in which the type parameter is defined. | não verificado | **publicado** — `crates/analise/src/membros.rs:869` |
| `conflicting_type_variable_and_container` | `CompileTimeErrorCode.CONFLICTING_TYPE_VARIABLE_AND_EXTENSION` | `analyzer/lib/src/generated/error_verifier.dart:2635` `ErrorVerifier._checkForConflictingExtensionTypeVariableErrorCodes` | '{0}' can't be used to name both a type parameter and the extension in which the type parameter is defined. | não verificado | **publicado** — `crates/analise/src/membros.rs:873` |
| `conflicting_type_variable_and_container` | `CompileTimeErrorCode.CONFLICTING_TYPE_VARIABLE_AND_EXTENSION_TYPE` | `analyzer/lib/src/generated/error_verifier.dart:2604` `ErrorVerifier._checkForConflictingExtensionTypeTypeVariableErrorCodes` | '{0}' can't be used to name both a type parameter and the extension type in which the type parameter is defined. | não verificado | **publicado** — `crates/analise/src/membros.rs:877` |
| `conflicting_type_variable_and_container` | `CompileTimeErrorCode.CONFLICTING_TYPE_VARIABLE_AND_MIXIN` | `analyzer/lib/src/generated/error_verifier.dart:2542` `ErrorVerifier._checkForConflictingClassTypeVariableErrorCodes` | '{0}' can't be used to name both a type parameter and the mixin in which the type parameter is defined. | não verificado | **publicado** — `crates/analise/src/membros.rs:865` |
| `generic_function_type_cannot_be_bound` | `CompileTimeErrorCode.GENERIC_FUNCTION_TYPE_CANNOT_BE_BOUND` | `analyzer/lib/src/generated/error_verifier.dart:3744` `ErrorVerifier._checkForGenericFunctionType` | Generic function types can't be used as type parameter bounds. | não verificado | **não implementado** |
| `illegal_language_version_override` | `CompileTimeErrorCode.ILLEGAL_LANGUAGE_VERSION_OVERRIDE` | `analyzer/lib/src/generated/error_verifier.dart:3764` `ErrorVerifier._checkForIllegalLanguageOverride` | The language version must be {0}. | não verificado | **publicado** — `crates/elements/src/load.rs:960` |
| `invalid_annotation_from_deferred_library` | `CompileTimeErrorCode.INVALID_ANNOTATION_FROM_DEFERRED_LIBRARY` | `analyzer/lib/src/generated/error_verifier.dart:3916` `ErrorVerifier._checkForInvalidAnnotationFromDeferredLibrary` | Constant values from a deferred library can't be used as annotations. | não verificado | **não implementado** |
| `invalid_macro_application_target` | `CompileTimeErrorCode.INVALID_MACRO_APPLICATION_TARGET` | `analyzer/lib/src/generated/error_verifier.dart:6883` `_MacroDiagnosticsReporter._reportInvalidTarget` | The macro can be applied only to a {0}. | não verificado | **não implementado** |
| `invalid_reference_to_this` | `CompileTimeErrorCode.INVALID_REFERENCE_TO_THIS` | `analyzer/lib/src/generated/error_verifier.dart:4063` `ErrorVerifier._checkForInvalidReferenceToThis` | Invalid reference to 'this' expression. | não verificado | **publicado** — `crates/analise/src/membros.rs:622` |
| `macro_application_argument_error` | `CompileTimeErrorCode.MACRO_APPLICATION_ARGUMENT_ERROR` | `analyzer/lib/src/generated/error_verifier.dart:6755` `_MacroDiagnosticsReporter._reportArgument` | {0} | não verificado | **não implementado** |
| `macro_declarations_phase_introspection_cycle` | `CompileTimeErrorCode.MACRO_DECLARATIONS_PHASE_INTROSPECTION_CYCLE` | `analyzer/lib/src/generated/error_verifier.dart:6874` `_MacroDiagnosticsReporter._reportIntrospectionCycle` | The declaration '{0}' can't be introspected because there is a cycle of macro applications. | não verificado | **não implementado** |
| `macro_definition_application_same_library_cycle` | `CompileTimeErrorCode.MACRO_DEFINITION_APPLICATION_SAME_LIBRARY_CYCLE` | `analyzer/lib/src/generated/error_verifier.dart:6742` `_MacroDiagnosticsReporter._reportApplicationFromSameLibraryCycle` | The macro '{0}' can't be applied in the same library cycle where it is defined. | não verificado | **não implementado** |
| `macro_error` | `CompileTimeErrorCode.MACRO_ERROR` | `analyzer/lib/src/generated/error_verifier.dart:6764` `_MacroDiagnosticsReporter._reportCustom` | {0} | não verificado | **não implementado** |
| `macro_info` | `HintCode.MACRO_INFO` | `analyzer/lib/src/generated/error_verifier.dart:6762` `_MacroDiagnosticsReporter._reportCustom` | {0} | não verificado | **não implementado** |
| `macro_internal_exception` | `CompileTimeErrorCode.MACRO_INTERNAL_EXCEPTION` | `analyzer/lib/src/generated/error_verifier.dart:6841` `_MacroDiagnosticsReporter._reportException` | {0} {1} | não verificado | **não implementado** |
| `macro_not_allowed_declaration` | `CompileTimeErrorCode.MACRO_NOT_ALLOWED_DECLARATION` | `analyzer/lib/src/generated/error_verifier.dart:6895` `_MacroDiagnosticsReporter._reportNotAllowedDeclaration` | The macro attempted to add declaration(s) not allowed during the {0} phase.\nLocations: {1}\n---\n{2}\n--- | não verificado | **não implementado** |
| `macro_warning` | `WarningCode.MACRO_WARNING` | `analyzer/lib/src/generated/error_verifier.dart:6763` `_MacroDiagnosticsReporter._reportCustom` | {0} | não verificado | **não implementado** |
| `missing_enum_constant_in_switch` | `StaticWarningCode.MISSING_ENUM_CONSTANT_IN_SWITCH` | `analyzer/lib/src/generated/error_verifier.dart:4276` `ErrorVerifier._checkForMissingEnumConstantInSwitch`; `analyzer/lib/src/generated/error_verifier.dart:4287` `ErrorVerifier._checkForMissingEnumConstantInSwitch` | Missing case clause for '{0}'. | não verificado | **não implementado** |
| `non_const_map_as_expression_statement` | `CompileTimeErrorCode.NON_CONST_MAP_AS_EXPRESSION_STATEMENT` | `analyzer/lib/src/generated/error_verifier.dart:4749` `ErrorVerifier._checkForNonConstMapAsExpressionStatement3` | A non-constant map or set literal without type arguments can't be used as an expression statement. | não verificado | **não implementado** |
| `unnecessary_non_null_assertion` | `StaticWarningCode.UNNECESSARY_NON_NULL_ASSERTION` | `analyzer/lib/src/generated/error_verifier.dart:5576` `ErrorVerifier._checkForUnnecessaryNullAware` | The '!' will have no effect because the receiver can't be null. | não verificado | **publicado** — `crates/types/src/inferencia/expr.rs:2520` |

##### `analyzer/lib/src/fasta/error_converter.dart` (1)

| código | constante | emissor (arquivo:linha, método) | condição (mensagem oficial) | posição | DartForge |
|---|---|---|---|---|---|
| `await_in_wrong_context` | `CompileTimeErrorCode.AWAIT_IN_WRONG_CONTEXT` | `analyzer/lib/src/fasta/error_converter.dart:52` `FastaErrorReporter.reportByCode`; `analyzer/lib/src/generated/error_verifier.dart:374` `ErrorVerifier.visitAwaitExpression`; fasta `AwaitNotAsync`: `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:8750` `Parser.parseAwaitExpression` | The await expression can only be used in an async function. | não verificado | **publicado** — `crates/frontend/src/parser/expressions.rs:849` |

### II.6 — Constantes, herança e sobrescrita

Lote `S2`: **71 constantes** (40 publicadas, 16 emitidas e não publicadas, 15 não implementadas no DartForge).
Esta tabela foi **gerada por script** (`E:\dftempnalise\spec-infrainal\gera2.py`) a partir do levantamento automático (`catalogo.json`: definição, sítios de emissão por grep com o método que os contém, referências em `crates/`). A coluna "condição" traz a **mensagem oficial** do código (o texto do `messages.yaml`), não a condição lida no método emissor; a coluna "posição" está **não verificada** em todas as linhas. Antes de implementar um código deste lote, abra o emissor citado e escreva os seis campos (o molde está em `docs/ANALYZER-ESPECIFICACAO.md`).

#### `analyzer/lib/src/dart/constant/constant_verifier.dart` (30)

| código | constante | emissor (arquivo:linha, método) | condição (mensagem oficial) | posição | DartForge |
|---|---|---|---|---|---|
| `const_eval_extension_method` | `CompileTimeErrorCode.CONST_EVAL_EXTENSION_METHOD` | `analyzer/lib/src/dart/constant/constant_verifier.dart:646` `ConstantVerifier._reportError`; `analyzer/lib/src/dart/constant/evaluation.dart:643` `ConstantVisitor.visitBinaryExpression`; `analyzer/lib/src/dart/constant/evaluation.dart:1149` `ConstantVisitor.visitPrefixExpression` | Extension methods can't be used in constant expressions. | não verificado | **publicado** — `crates/types/src/constantes/avaliador.rs:928`, `crates/types/src/constantes/avaliador.rs:1321` |
| `const_eval_extension_type_method` | `CompileTimeErrorCode.CONST_EVAL_EXTENSION_TYPE_METHOD` | `analyzer/lib/src/dart/constant/constant_verifier.dart:648` `ConstantVerifier._reportError`; `analyzer/lib/src/dart/constant/evaluation.dart:648` `ConstantVisitor.visitBinaryExpression`; `analyzer/lib/src/dart/constant/evaluation.dart:1154` `ConstantVisitor.visitPrefixExpression` | Extension type methods can't be used in constant expressions. | não verificado | **emitido, não publicado** — `crates/types/src/constantes/avaliador.rs:930`, `crates/types/src/constantes/avaliador.rs:1323` |
| `const_eval_for_element` | `CompileTimeErrorCode.CONST_EVAL_FOR_ELEMENT` | `analyzer/lib/src/dart/constant/constant_verifier.dart:649` `ConstantVerifier._reportError`; `analyzer/lib/src/dart/constant/constant_verifier.dart:1098` `_ConstLiteralVerifier.verify`; `analyzer/lib/src/dart/constant/evaluation.dart:1379` `ConstantVisitor._buildListConstant` | Constant expressions don't support 'for' elements. | não verificado | **publicado** — `crates/types/src/constantes/avaliador.rs:672`, `crates/types/src/constantes/avaliador.rs:750` |
| `const_eval_throws_idbze` | `CompileTimeErrorCode.CONST_EVAL_THROWS_IDBZE` | `analyzer/lib/src/dart/constant/constant_verifier.dart:655` `ConstantVerifier._reportError`; `analyzer/lib/src/dart/constant/value.dart:2148` `IntState.integerDivide`; `analyzer/lib/src/lint/linter.dart:394` `_ConstantAnalysisErrorListener.onError` | Evaluation of this constant expression throws an IntegerDivisionByZeroException. | não verificado | **publicado** — `crates/types/src/constantes/valor.rs:309`, `crates/types/src/constantes/verificador.rs:29` |
| `const_eval_type_bool_int` | `CompileTimeErrorCode.CONST_EVAL_TYPE_BOOL_INT` | `analyzer/lib/src/dart/constant/constant_verifier.dart:659` `ConstantVerifier._reportError`; `analyzer/lib/src/dart/constant/value.dart:429` `DartObjectImpl.eagerAnd`; `analyzer/lib/src/dart/constant/value.dart:452` `DartObjectImpl.eagerOr` | In constant expressions, operands of this operator must be of type 'bool' or 'int'. | não verificado | **publicado** — `crates/types/src/constantes/avaliador.rs:1038`, `crates/types/src/constantes/verificador.rs:32` |
| `const_eval_type_bool_num_string` | `CompileTimeErrorCode.CONST_EVAL_TYPE_BOOL_NUM_STRING` | `analyzer/lib/src/dart/constant/constant_verifier.dart:657` `ConstantVerifier._reportError`; `analyzer/lib/src/dart/constant/evaluation.dart:973` `ConstantVisitor.visitInterpolationExpression`; `analyzer/lib/src/dart/constant/value.dart:515` `DartObjectImpl.equalEqual` | In constant expressions, operands of this operator must be of type 'bool', 'num', 'String' or 'null'. | não verificado | **publicado** — `crates/types/src/constantes/avaliador.rs:617`, `crates/types/src/constantes/avaliador.rs:1093` |
| `const_eval_type_int` | `CompileTimeErrorCode.CONST_EVAL_TYPE_INT` | `analyzer/lib/src/dart/constant/constant_verifier.dart:660` `ConstantVerifier._reportError`; `analyzer/lib/src/dart/constant/value.dart:1605` `InstanceState.assertIntOrNull`; `analyzer/lib/src/lint/linter.dart:389` `_ConstantAnalysisErrorListener.onError` | In constant expressions, operands of this operator must be of type 'int'. | não verificado | **publicado** — `crates/types/src/constantes/valor.rs:124`, `crates/types/src/constantes/verificador.rs:33` |
| `const_eval_type_num` | `CompileTimeErrorCode.CONST_EVAL_TYPE_NUM` | `analyzer/lib/src/dart/constant/constant_verifier.dart:661` `ConstantVerifier._reportError`; `analyzer/lib/src/dart/constant/value.dart:1613` `InstanceState.assertNumOrNull`; `analyzer/lib/src/lint/linter.dart:390` `_ConstantAnalysisErrorListener.onError` | In constant expressions, operands of this operator must be of type 'num'. | não verificado | **publicado** — `crates/types/src/constantes/valor.rs:128`, `crates/types/src/constantes/verificador.rs:34` |
| `const_eval_type_num_string` | `CompileTimeErrorCode.CONST_EVAL_TYPE_NUM_STRING` | `analyzer/lib/src/dart/constant/constant_verifier.dart:662` `ConstantVerifier._reportError`; `analyzer/lib/src/dart/constant/value.dart:1622` `InstanceState.assertNumStringOrNull`; `analyzer/lib/src/lint/linter.dart:391` `_ConstantAnalysisErrorListener.onError` | In constant expressions, operands of this operator must be of type 'num' or 'String'. | não verificado | **publicado** — `crates/types/src/constantes/valor.rs:135`, `crates/types/src/constantes/verificador.rs:35` |
| `const_map_key_not_primitive_equality` | `CompileTimeErrorCode.CONST_MAP_KEY_NOT_PRIMITIVE_EQUALITY` | `analyzer/lib/src/dart/constant/constant_verifier.dart:1299` `_ConstLiteralVerifier._validateMapLiteralEntry`; `analyzer/lib/src/lint/linter.dart:396` `_ConstantAnalysisErrorListener.onError` | The type of a key in a constant map can't override the '==' operator, or 'hashCode', but the class '{0}' does. | não verificado | **publicado** — `crates/types/src/constantes/verificador.rs:972` |
| `const_set_element_not_primitive_equality` | `CompileTimeErrorCode.CONST_SET_ELEMENT_NOT_PRIMITIVE_EQUALITY` | `analyzer/lib/src/dart/constant/constant_verifier.dart:1248` `_ConstLiteralVerifier._validateListOrSetSpread`; `analyzer/lib/src/dart/constant/constant_verifier.dart:1374` `_ConstLiteralVerifier._validateSetExpression`; `analyzer/lib/src/lint/linter.dart:397` `_ConstantAnalysisErrorListener.onError` | An element in a constant set can't override the '==' operator, or 'hashCode', but the type '{0}' does. | não verificado | **publicado** — `crates/types/src/constantes/verificador.rs:920`, `crates/types/src/constantes/verificador.rs:1005` |
| `const_spread_expected_list_or_set` | `CompileTimeErrorCode.CONST_SPREAD_EXPECTED_LIST_OR_SET` | `analyzer/lib/src/dart/constant/constant_verifier.dart:674` `ConstantVerifier._reportError`; `analyzer/lib/src/dart/constant/constant_verifier.dart:1233` `_ConstLiteralVerifier._validateListOrSetSpread`; `analyzer/lib/src/dart/constant/evaluation.dart:1436` `ConstantVisitor._buildListConstant` | A list or a set is expected in this spread. | não verificado | **emitido, não publicado** — `crates/types/src/constantes/avaliador.rs:717`, `crates/types/src/constantes/avaliador.rs:792` |
| `const_spread_expected_map` | `CompileTimeErrorCode.CONST_SPREAD_EXPECTED_MAP` | `analyzer/lib/src/dart/constant/constant_verifier.dart:675` `ConstantVerifier._reportError`; `analyzer/lib/src/dart/constant/constant_verifier.dart:1351` `_ConstLiteralVerifier._validateMapSpread`; `analyzer/lib/src/dart/constant/evaluation.dart:1544` `ConstantVisitor._buildMapConstant` | A map is expected in this spread. | não verificado | **emitido, não publicado** — `crates/types/src/constantes/avaliador.rs:877`, `crates/types/src/constantes/verificador.rs:43` |
| `if_element_condition_from_deferred_library` | `CompileTimeErrorCode.IF_ELEMENT_CONDITION_FROM_DEFERRED_LIBRARY` | `analyzer/lib/src/dart/constant/constant_verifier.dart:702` `ConstantVerifier._reportError`; `analyzer/lib/src/dart/constant/evaluation.dart:1894` `ConstantVisitor._getDeferredLibraryError` | Constant values from a deferred library can't be used as values in an if condition inside a const collection literal. | não verificado | **emitido, não publicado** — `crates/types/src/constantes/verificador.rs:54` |
| `invalid_annotation_constant_value_from_deferred_library` | `CompileTimeErrorCode.INVALID_ANNOTATION_CONSTANT_VALUE_FROM_DEFERRED_LIBRARY` | `analyzer/lib/src/dart/constant/constant_verifier.dart:700` `ConstantVerifier._reportError`; `analyzer/lib/src/dart/constant/evaluation.dart:1885` `ConstantVisitor._getDeferredLibraryError` | Constant values from a deferred library can't be used in annotations. | não verificado | **emitido, não publicado** — `crates/types/src/constantes/verificador.rs:53` |
| `non_bool_condition` | `CompileTimeErrorCode.NON_BOOL_CONDITION` | `analyzer/lib/src/dart/constant/constant_verifier.dart:678` `ConstantVerifier._reportError`; `analyzer/lib/src/dart/constant/evaluation.dart:1398` `ConstantVisitor._buildListConstant`; `analyzer/lib/src/dart/constant/evaluation.dart:1505` `ConstantVisitor._buildMapConstant` | Conditions must have a static type of 'bool'. | não verificado | **publicado** — `crates/types/src/constantes/avaliador.rs:687`, `crates/types/src/constantes/avaliador.rs:764` |
| `non_constant_case_expression_from_deferred_library` | `CompileTimeErrorCode.NON_CONSTANT_CASE_EXPRESSION_FROM_DEFERRED_LIBRARY` | `analyzer/lib/src/dart/constant/constant_verifier.dart:696` `ConstantVerifier._reportError`; `analyzer/lib/src/dart/constant/evaluation.dart:1918` `ConstantVisitor._getDeferredLibraryError` | Constant values from a deferred library can't be used as a case expression. | não verificado | **emitido, não publicado** — `crates/types/src/constantes/verificador.rs:52` |
| `non_constant_default_value_from_deferred_library` | `CompileTimeErrorCode.NON_CONSTANT_DEFAULT_VALUE_FROM_DEFERRED_LIBRARY` | `analyzer/lib/src/dart/constant/constant_verifier.dart:682` `ConstantVerifier._reportError`; `analyzer/lib/src/dart/constant/evaluation.dart:1891` `ConstantVisitor._getDeferredLibraryError` | Constant values from a deferred library can't be used as a default parameter value. | não verificado | **emitido, não publicado** — `crates/types/src/constantes/verificador.rs:47` |
| `non_constant_map_element` | `CompileTimeErrorCode.NON_CONSTANT_MAP_ELEMENT` | `analyzer/lib/src/dart/constant/constant_verifier.dart:422` `ConstantVerifier.visitSetOrMapLiteral`; `analyzer/lib/src/dart/constant/constant_verifier.dart:1178` `_ConstLiteralVerifier._reportNotPotentialConstants`; `analyzer/lib/src/lint/linter.dart:410` `_ConstantAnalysisErrorListener.onError` | The elements in a const map literal must be constant. | não verificado | **publicado** — `crates/types/src/constantes/verificador.rs:892`, `crates/types/src/constantes/verificador.rs:1068` |
| `non_constant_map_key` | `CompileTimeErrorCode.NON_CONSTANT_MAP_KEY` | `analyzer/lib/src/dart/constant/constant_verifier.dart:1184` `_ConstLiteralVerifier._reportNotPotentialConstants`; `analyzer/lib/src/dart/constant/constant_verifier.dart:1277` `_ConstLiteralVerifier._validateMapLiteralEntry`; `analyzer/lib/src/lint/linter.dart:411` `_ConstantAnalysisErrorListener.onError` | The keys in a const map literal must be constant. | não verificado | **publicado** — `crates/types/src/constantes/verificador.rs:962`, `crates/types/src/constantes/verificador.rs:1079` |
| `non_constant_map_pattern_key` | `CompileTimeErrorCode.NON_CONSTANT_MAP_PATTERN_KEY` | `analyzer/lib/src/dart/constant/constant_verifier.dart:335` `ConstantVerifier.visitMapPattern` | Key expressions in map patterns must be constants. | não verificado | **publicado** — `crates/types/src/constantes/verificador.rs:574` |
| `non_constant_map_value` | `CompileTimeErrorCode.NON_CONSTANT_MAP_VALUE` | `analyzer/lib/src/dart/constant/constant_verifier.dart:1186` `_ConstLiteralVerifier._reportNotPotentialConstants`; `analyzer/lib/src/dart/constant/constant_verifier.dart:1281` `_ConstLiteralVerifier._validateMapLiteralEntry`; `analyzer/lib/src/lint/linter.dart:412` `_ConstantAnalysisErrorListener.onError` | The values in a const map literal must be constant. | não verificado | **publicado** — `crates/types/src/constantes/verificador.rs:963`, `crates/types/src/constantes/verificador.rs:1079` |
| `non_constant_record_field` | `CompileTimeErrorCode.NON_CONSTANT_RECORD_FIELD` | `analyzer/lib/src/dart/constant/constant_verifier.dart:374` `ConstantVerifier.visitRecordLiteral`; `analyzer/lib/src/lint/linter.dart:413` `_ConstantAnalysisErrorListener.onError` | The fields in a const record literal must be constants. | não verificado | **publicado** — `crates/types/src/constantes/verificador.rs:667` |
| `non_constant_record_field_from_deferred_library` | `CompileTimeErrorCode.NON_CONSTANT_RECORD_FIELD_FROM_DEFERRED_LIBRARY` | `analyzer/lib/src/dart/constant/constant_verifier.dart:714` `ConstantVerifier._reportError`; `analyzer/lib/src/dart/constant/evaluation.dart:1911` `ConstantVisitor._getDeferredLibraryError` | Constant values from a deferred library can't be used as fields in a 'const' record literal. | não verificado | **emitido, não publicado** — `crates/types/src/constantes/verificador.rs:57` |
| `non_constant_set_element` | `CompileTimeErrorCode.NON_CONSTANT_SET_ELEMENT` | `analyzer/lib/src/dart/constant/constant_verifier.dart:400` `ConstantVerifier.visitSetOrMapLiteral`; `analyzer/lib/src/dart/constant/constant_verifier.dart:1192` `_ConstLiteralVerifier._reportNotPotentialConstants`; `analyzer/lib/src/lint/linter.dart:414` `_ConstantAnalysisErrorListener.onError` | The values in a const set literal must be constants. | não verificado | **publicado** — `crates/types/src/constantes/verificador.rs:891` |
| `non_exhaustive_switch_statement` | `CompileTimeErrorCode.NON_EXHAUSTIVE_SWITCH_STATEMENT` | `analyzer/lib/src/dart/constant/constant_verifier.dart:970` `ConstantVerifier._validateSwitchExhaustiveness` | The type '{0}' is not exhaustively matched by the switch cases since it doesn't match '{1}'. | não verificado | **publicado** — `crates/types/src/constantes/verificador.rs:773` |
| `pattern_constant_from_deferred_library` | `CompileTimeErrorCode.PATTERN_CONSTANT_FROM_DEFERRED_LIBRARY` | `analyzer/lib/src/dart/constant/constant_verifier.dart:720` `ConstantVerifier._reportError`; `analyzer/lib/src/dart/constant/evaluation.dart:1920` `ConstantVisitor._getDeferredLibraryError` | Constant values from a deferred library can't be used in patterns. | não verificado | **emitido, não publicado** — `crates/types/src/constantes/verificador.rs:58` |
| `spread_expression_from_deferred_library` | `CompileTimeErrorCode.SPREAD_EXPRESSION_FROM_DEFERRED_LIBRARY` | `analyzer/lib/src/dart/constant/constant_verifier.dart:692` `ConstantVerifier._reportError`; `analyzer/lib/src/dart/constant/evaluation.dart:1915` `ConstantVisitor._getDeferredLibraryError` | Constant values from a deferred library can't be spread into a const literal. | não verificado | **emitido, não publicado** — `crates/types/src/constantes/verificador.rs:51` |
| `unreachable_switch_case` | `WarningCode.UNREACHABLE_SWITCH_CASE` | `analyzer/lib/src/dart/constant/constant_verifier.dart:946` `ConstantVerifier._validateSwitchExhaustiveness` | This case is covered by the previous cases. | não verificado | **publicado** — `crates/types/src/constantes/verificador.rs:767` |
| `unreachable_switch_default` | `WarningCode.UNREACHABLE_SWITCH_DEFAULT` | `analyzer/lib/src/dart/constant/constant_verifier.dart:984` `ConstantVerifier._validateSwitchExhaustiveness` | This default clause is covered by the previous cases. | não verificado | **publicado** — `crates/types/src/constantes/verificador.rs:781` |

#### `analyzer/lib/src/dart/analysis/library_analyzer.dart` (13)

| código | constante | emissor (arquivo:linha, método) | condição (mensagem oficial) | posição | DartForge |
|---|---|---|---|---|---|
| `deprecated_export_use` | `WarningCode.DEPRECATED_EXPORT_USE` | `analyzer/lib/src/dart/analysis/library_analyzer.dart:608` `LibraryAnalyzer._hasDiagnosticReportedThatPreventsImportWarnings`; `analyzer/lib/src/generated/scope_helpers.dart:67` `ScopeHelpers._reportDeprecatedExportUse` | The ability to import '{0}' indirectly is deprecated. | não verificado | **emitido, não publicado** — `crates/analise/src/importacoes.rs:43` |
| `duplicate_part` | `CompileTimeErrorCode.DUPLICATE_PART` | `analyzer/lib/src/dart/analysis/library_analyzer.dart:1040` `LibraryAnalyzer._resolvePartDirective` | The library already contains a part with the URI '{0}'. | não verificado | **não implementado** |
| `export_of_non_library` | `CompileTimeErrorCode.EXPORT_OF_NON_LIBRARY` | `analyzer/lib/src/dart/analysis/library_analyzer.dart:934` `LibraryAnalyzer._resolveLibraryExportDirective` | The exported library '{0}' can't have a part-of directive. | não verificado | **não implementado** |
| `extends_non_class` | `CompileTimeErrorCode.EXTENDS_NON_CLASS` | `analyzer/lib/src/dart/analysis/library_analyzer.dart:597` `LibraryAnalyzer._hasDiagnosticReportedThatPreventsImportWarnings`; `analyzer/lib/src/dart/resolver/resolution_visitor.dart:1698` `ResolutionVisitor._resolveType` | Classes can only extend other classes. | não verificado | **publicado** — `crates/analise/src/clausulas.rs:1231`, `crates/analise/src/importacoes.rs:32` |
| `implements_non_class` | `CompileTimeErrorCode.IMPLEMENTS_NON_CLASS` | `analyzer/lib/src/dart/analysis/library_analyzer.dart:598` `LibraryAnalyzer._hasDiagnosticReportedThatPreventsImportWarnings`; `analyzer/lib/src/dart/resolver/resolution_visitor.dart:1702` `ResolutionVisitor._resolveType` | Classes and mixins can only implement other classes and mixins. | não verificado | **publicado** — `crates/analise/src/clausulas.rs:1236`, `crates/analise/src/clausulas.rs:1246` |
| `import_of_non_library` | `CompileTimeErrorCode.IMPORT_OF_NON_LIBRARY` | `analyzer/lib/src/dart/analysis/library_analyzer.dart:706` `LibraryAnalyzer._reportImportDirectiveErrors`; `analyzer/lib/src/fasta/error_converter.dart:212` `FastaErrorReporter.reportByCode` | The imported library '{0}' can't have a part-of directive. | não verificado | **não implementado** |
| `inconsistent_language_version_override` | `CompileTimeErrorCode.INCONSISTENT_LANGUAGE_VERSION_OVERRIDE` | `analyzer/lib/src/dart/analysis/library_analyzer.dart:263` `LibraryAnalyzer._checkForInconsistentLanguageVersionOverride` | Parts must have exactly the same language version override as the library. | não verificado | **não implementado** |
| `invalid_uri` | `CompileTimeErrorCode.INVALID_URI` | `analyzer/lib/src/dart/analysis/library_analyzer.dart:713` `LibraryAnalyzer._reportImportDirectiveErrors`; `analyzer/lib/src/dart/analysis/library_analyzer.dart:941` `LibraryAnalyzer._resolveLibraryExportDirective`; `analyzer/lib/src/dart/analysis/library_analyzer.dart:1002` `LibraryAnalyzer._resolvePartDirective` | Invalid URI syntax: '{0}'. | não verificado | **não implementado** |
| `mixin_of_non_class` | `CompileTimeErrorCode.MIXIN_OF_NON_CLASS` | `analyzer/lib/src/dart/analysis/library_analyzer.dart:599` `LibraryAnalyzer._hasDiagnosticReportedThatPreventsImportWarnings`; `analyzer/lib/src/dart/resolver/resolution_visitor.dart:1707` `ResolutionVisitor._resolveType` | Classes can only mix in mixins and classes. | não verificado | **publicado** — `crates/analise/src/clausulas.rs:1235`, `crates/analise/src/clausulas.rs:1248` |
| `part_of_different_library` | `CompileTimeErrorCode.PART_OF_DIFFERENT_LIBRARY` | `analyzer/lib/src/dart/analysis/library_analyzer.dart:1060` `LibraryAnalyzer._resolvePartDirective`; `analyzer/lib/src/dart/analysis/library_analyzer.dart:1067` `LibraryAnalyzer._resolvePartDirective` | Expected this library to be part of '{0}', not '{1}'. | não verificado | **não implementado** |
| `part_of_unnamed_library` | `CompileTimeErrorCode.PART_OF_UNNAMED_LIBRARY` | `analyzer/lib/src/dart/analysis/library_analyzer.dart:1055` `LibraryAnalyzer._resolvePartDirective` | The library is unnamed. A URI is expected, not a library name '{0}', in the part-of directive. | não verificado | **não implementado** |
| `uri_does_not_exist` | `CompileTimeErrorCode.URI_DOES_NOT_EXIST` | `analyzer/lib/src/dart/analysis/library_analyzer.dart:686` `LibraryAnalyzer._reportImportDirectiveErrors`; `analyzer/lib/src/dart/analysis/library_analyzer.dart:697` `LibraryAnalyzer._reportImportDirectiveErrors`; `analyzer/lib/src/dart/analysis/library_analyzer.dart:919` `LibraryAnalyzer._resolveLibraryExportDirective` | Target of URI doesn't exist: '{0}'. | não verificado | **não implementado** |
| `uri_has_not_been_generated` | `CompileTimeErrorCode.URI_HAS_NOT_BEEN_GENERATED` | `analyzer/lib/src/dart/analysis/library_analyzer.dart:696` `LibraryAnalyzer._reportImportDirectiveErrors`; `analyzer/lib/src/dart/analysis/library_analyzer.dart:924` `LibraryAnalyzer._resolveLibraryExportDirective`; `analyzer/lib/src/dart/analysis/library_analyzer.dart:1024` `LibraryAnalyzer._resolvePartDirective` | Target of URI hasn't been generated: '{0}'. | não verificado | **não implementado** |

#### `analyzer/lib/src/error/inheritance_override.dart` (11)

| código | constante | emissor (arquivo:linha, método) | condição (mensagem oficial) | posição | DartForge |
|---|---|---|---|---|---|
| `illegal_concrete_enum_member` | `CompileTimeErrorCode.ILLEGAL_CONCRETE_ENUM_MEMBER_DECLARATION` | `analyzer/lib/src/error/inheritance_override.dart:670` `_ClassVerifier._checkIllegalConcreteEnumMemberDeclaration` | A concrete instance member named '{0}' can't be declared in a class that implements 'Enum'. | não verificado | **publicado** — `crates/types/src/sobrescritas.rs:1528`, `crates/types/src/sobrescritas.rs:1539` |
| `illegal_concrete_enum_member` | `CompileTimeErrorCode.ILLEGAL_CONCRETE_ENUM_MEMBER_INHERITANCE` | `analyzer/lib/src/error/inheritance_override.dart:699` `_ClassVerifier._checkIllegalConcreteEnumMemberInheritance` | A concrete instance member named '{0}' can't be inherited from '{1}' in a class that implements 'Enum'. | não verificado | **publicado** — `crates/types/src/sobrescritas.rs:1571` |
| `illegal_enum_values` | `CompileTimeErrorCode.ILLEGAL_ENUM_VALUES_DECLARATION` | `analyzer/lib/src/error/inheritance_override.dart:716` `_ClassVerifier._checkIllegalEnumValuesDeclaration` | An instance member named 'values' can't be declared in a class that implements 'Enum'. | não verificado | **publicado** — `crates/types/src/sobrescritas.rs:1525`, `crates/types/src/sobrescritas.rs:1542` |
| `illegal_enum_values` | `CompileTimeErrorCode.ILLEGAL_ENUM_VALUES_INHERITANCE` | `analyzer/lib/src/error/inheritance_override.dart:735` `_ClassVerifier._checkIllegalEnumValuesInheritance` | An instance member named 'values' can't be inherited from '{0}' in a class that implements 'Enum'. | não verificado | **publicado** — `crates/types/src/sobrescritas.rs:1583` |
| `missing_override_of_must_be_overridden` | `WarningCode.MISSING_OVERRIDE_OF_MUST_BE_OVERRIDDEN_ONE` | `analyzer/lib/src/error/inheritance_override.dart:1053` `_ClassVerifier._verifyMustBeOverridden` | Missing concrete implementation of '{0}'. | não verificado | **não implementado** |
| `missing_override_of_must_be_overridden` | `WarningCode.MISSING_OVERRIDE_OF_MUST_BE_OVERRIDDEN_THREE_PLUS` | `analyzer/lib/src/error/inheritance_override.dart:1065` `_ClassVerifier._verifyMustBeOverridden` | Missing concrete implementations of '{0}', '{1}', and {2} more. | não verificado | **não implementado** |
| `missing_override_of_must_be_overridden` | `WarningCode.MISSING_OVERRIDE_OF_MUST_BE_OVERRIDDEN_TWO` | `analyzer/lib/src/error/inheritance_override.dart:1059` `_ClassVerifier._verifyMustBeOverridden` | Missing concrete implementations of '{0}' and '{1}'. | não verificado | **não implementado** |
| `subtype_of_disallowed_type` | `CompileTimeErrorCode.EXTENDS_DISALLOWED_CLASS` | `analyzer/lib/src/error/inheritance_override.dart:511` `_ClassVerifier._checkDirectSuperTypes`; `analyzer/lib/src/generated/error_verifier.dart:3324` `ErrorVerifier._checkForExtendsDisallowedClass` | Classes can't extend '{0}'. | não verificado | **publicado** — `crates/analise/src/clausulas.rs:1288` |
| `subtype_of_disallowed_type` | `CompileTimeErrorCode.IMPLEMENTS_DISALLOWED_CLASS` | `analyzer/lib/src/error/inheritance_override.dart:492` `_ClassVerifier._checkDirectSuperTypes`; `analyzer/lib/src/generated/error_verifier.dart:3782` `ErrorVerifier._checkForImplementsClauseErrorCodes` | Classes and mixins can't implement '{0}'. | não verificado | **publicado** — `crates/analise/src/clausulas.rs:1286` |
| `subtype_of_disallowed_type` | `CompileTimeErrorCode.MIXIN_OF_DISALLOWED_CLASS` | `analyzer/lib/src/error/inheritance_override.dart:520` `_ClassVerifier._checkDirectSuperTypes`; `analyzer/lib/src/generated/error_verifier.dart:1988` `ErrorVerifier._checkForAllMixinErrorCodes` | Classes can't mixin '{0}'. | não verificado | **publicado** — `crates/analise/src/clausulas.rs:1289` |
| `subtype_of_disallowed_type` | `CompileTimeErrorCode.MIXIN_SUPER_CLASS_CONSTRAINT_DISALLOWED_CLASS` | `analyzer/lib/src/error/inheritance_override.dart:502` `_ClassVerifier._checkDirectSuperTypes`; `analyzer/lib/src/generated/error_verifier.dart:4931` `ErrorVerifier._checkForOnClauseErrorCodes` | '{0}' can't be used as a superclass constraint. | não verificado | **publicado** — `crates/analise/src/clausulas.rs:1287` |

#### `analyzer/lib/src/dart/constant/evaluation.dart` (5)

| código | constante | emissor (arquivo:linha, método) | condição (mensagem oficial) | posição | DartForge |
|---|---|---|---|---|---|
| `const_eval_assertion_failure` | `CompileTimeErrorCode.CONST_EVAL_ASSERTION_FAILURE` | `analyzer/lib/src/dart/constant/evaluation.dart:2826` `_InstanceCreationEvaluator._checkInitializers` | The assertion in this constant expression failed. | não verificado | **emitido, não publicado** — `crates/types/src/constantes/avaliador.rs:2123` |
| `const_eval_assertion_failure_with_message` | `CompileTimeErrorCode.CONST_EVAL_ASSERTION_FAILURE_WITH_MESSAGE` | `analyzer/lib/src/dart/constant/evaluation.dart:2818` `_InstanceCreationEvaluator._checkInitializers` | An assertion failed with message '{0}'. | não verificado | **emitido, não publicado** — `crates/types/src/constantes/avaliador.rs:2116` |
| `missing_const_in_list_literal` | `CompileTimeErrorCode.MISSING_CONST_IN_LIST_LITERAL` | `analyzer/lib/src/dart/constant/evaluation.dart:1004` `ConstantVisitor.visitListLiteral`; `analyzer/lib/src/lint/linter.dart:405` `_ConstantAnalysisErrorListener.onError` | Seeing this message constitutes a bug. Please report it. | não verificado | **emitido, não publicado** — `crates/types/src/constantes/avaliador.rs:331` |
| `missing_const_in_map_literal` | `CompileTimeErrorCode.MISSING_CONST_IN_MAP_LITERAL` | `analyzer/lib/src/dart/constant/evaluation.dart:1266` `ConstantVisitor.visitSetOrMapLiteral`; `analyzer/lib/src/lint/linter.dart:406` `_ConstantAnalysisErrorListener.onError` | Seeing this message constitutes a bug. Please report it. | não verificado | **emitido, não publicado** — `crates/types/src/constantes/avaliador.rs:359` |
| `missing_const_in_set_literal` | `CompileTimeErrorCode.MISSING_CONST_IN_SET_LITERAL` | `analyzer/lib/src/dart/constant/evaluation.dart:1292` `ConstantVisitor.visitSetOrMapLiteral`; `analyzer/lib/src/lint/linter.dart:407` `_ConstantAnalysisErrorListener.onError` | Seeing this message constitutes a bug. Please report it. | não verificado | **emitido, não publicado** — `crates/types/src/constantes/avaliador.rs:376` |

#### `analyzer/lib/src/error/duplicate_definition_verifier.dart` (5)

| código | constante | emissor (arquivo:linha, método) | condição (mensagem oficial) | posição | DartForge |
|---|---|---|---|---|---|
| `enum_constant_same_name_as_enclosing` | `CompileTimeErrorCode.ENUM_CONSTANT_SAME_NAME_AS_ENCLOSING` | `analyzer/lib/src/error/duplicate_definition_verifier.dart:585` `MemberDuplicateDefinitionVerifier._checkEnum` | The name of the enum value can't be the same as the enum's name. | não verificado | **publicado** — `crates/analise/src/duplicatas.rs:627` |
| `enum_with_name_values` | `CompileTimeErrorCode.ENUM_WITH_NAME_VALUES` | `analyzer/lib/src/error/duplicate_definition_verifier.dart:598` `MemberDuplicateDefinitionVerifier._checkEnum` | The name 'values' is not a valid name for an enum. | não verificado | **publicado** — `crates/analise/src/duplicatas.rs:643` |
| `extension_conflicting_static_and_instance` | `CompileTimeErrorCode.EXTENSION_CONFLICTING_STATIC_AND_INSTANCE` | `analyzer/lib/src/error/duplicate_definition_verifier.dart:711` `MemberDuplicateDefinitionVerifier._checkExtensionStatic`; `analyzer/lib/src/error/duplicate_definition_verifier.dart:725` `MemberDuplicateDefinitionVerifier._checkExtensionStatic` | An extension can't define static member '{0}' and an instance member with the same name. | não verificado | **publicado** — `crates/analise/src/duplicatas.rs:759` |
| `prefix_collides_with_top_level_member` | `CompileTimeErrorCode.PREFIX_COLLIDES_WITH_TOP_LEVEL_MEMBER` | `analyzer/lib/src/error/duplicate_definition_verifier.dart:169` `DuplicateDefinitionVerifier.checkUnit` | The name '{0}' is already used as an import prefix and can't be used to name a top-level element. | não verificado | **publicado** — `crates/analise/src/duplicatas.rs:284` |
| `values_declaration_in_enum` | `CompileTimeErrorCode.VALUES_DECLARATION_IN_ENUM` | `analyzer/lib/src/error/duplicate_definition_verifier.dart:803` `MemberDuplicateDefinitionVerifier._checkValuesDeclarationInEnum` | A member named 'values' can't be declared in an enum. | não verificado | **publicado** — `crates/analise/src/duplicatas.rs:632`, `crates/analise/src/duplicatas.rs:690` |

#### `analyzer/lib/src/error/base_or_final_type_verifier.dart` (4)

| código | constante | emissor (arquivo:linha, método) | condição (mensagem oficial) | posição | DartForge |
|---|---|---|---|---|---|
| `subtype_of_base_or_final_is_not_base_final_or_sealed` | `CompileTimeErrorCode.MIXIN_SUBTYPE_OF_BASE_IS_NOT_BASE` | `analyzer/lib/src/error/base_or_final_type_verifier.dart:256` `BaseOrFinalTypeVerifier._reportRestrictionError` | The mixin '{0}' must be 'base' because the supertype '{1}' is 'base'. | não verificado | **publicado** — `crates/analise/src/modificadores.rs:293` |
| `subtype_of_base_or_final_is_not_base_final_or_sealed` | `CompileTimeErrorCode.MIXIN_SUBTYPE_OF_FINAL_IS_NOT_BASE` | `analyzer/lib/src/error/base_or_final_type_verifier.dart:241` `BaseOrFinalTypeVerifier._reportRestrictionError` | The mixin '{0}' must be 'base' because the supertype '{1}' is 'final'. | não verificado | **publicado** — `crates/analise/src/modificadores.rs:289` |
| `subtype_of_base_or_final_is_not_base_final_or_sealed` | `CompileTimeErrorCode.SUBTYPE_OF_BASE_IS_NOT_BASE_FINAL_OR_SEALED` | `analyzer/lib/src/error/base_or_final_type_verifier.dart:258` `BaseOrFinalTypeVerifier._reportRestrictionError` | The type '{0}' must be 'base', 'final' or 'sealed' because the supertype '{1}' is 'base'. | não verificado | **publicado** — `crates/analise/src/modificadores.rs:293` |
| `subtype_of_base_or_final_is_not_base_final_or_sealed` | `CompileTimeErrorCode.SUBTYPE_OF_FINAL_IS_NOT_BASE_FINAL_OR_SEALED` | `analyzer/lib/src/error/base_or_final_type_verifier.dart:243` `BaseOrFinalTypeVerifier._reportRestrictionError` | The type '{0}' must be 'base', 'final' or 'sealed' because the supertype '{1}' is 'final'. | não verificado | **publicado** — `crates/analise/src/modificadores.rs:289` |

#### `analyzer/lib/src/dart/constant/value.dart` (1)

| código | constante | emissor (arquivo:linha, método) | condição (mensagem oficial) | posição | DartForge |
|---|---|---|---|---|---|
| `const_eval_type_type` | `CompileTimeErrorCode.CONST_EVAL_TYPE_TYPE` | `analyzer/lib/src/dart/constant/value.dart:1008` `DartObjectImpl._assertType` | In constant expressions, operands of this operator must be of type 'Type'. | não verificado | **não implementado** |

#### `analyzer/lib/src/error/const_argument_verifier.dart` (1)

| código | constante | emissor (arquivo:linha, método) | condição (mensagem oficial) | posição | DartForge |
|---|---|---|---|---|---|
| `non_const_argument_for_const_parameter` | `WarningCode.NON_CONST_ARGUMENT_FOR_CONST_PARAMETER` | `analyzer/lib/src/error/const_argument_verifier.dart:38` `ConstArgumentsVerifier.visitAssignmentExpression`; `analyzer/lib/src/error/const_argument_verifier.dart:128` `ConstArgumentsVerifier._check` | Argument '{0}' must be a constant. | não verificado | **não implementado** |

#### `analyzer/lib/src/dart/analysis/driver.dart` (1)

| código | constante | emissor (arquivo:linha, método) | condição (mensagem oficial) | posição | DartForge |
|---|---|---|---|---|---|
| `missing_dart_library` | `CompileTimeErrorCode.MISSING_DART_LIBRARY` | `analyzer/lib/src/dart/analysis/driver.dart:1940` `AnalysisDriver._newMissingDartLibraryResult` | Required library '{0}' is missing. | não verificado | **não implementado** |

### II.7 — `BestPracticesVerifier`, anotações e documentação

Lote `S3a`: **34 constantes** (0 publicadas, 0 emitidas e não publicadas, 34 não implementadas no DartForge).
Esta tabela foi **gerada por script** (`E:\dftempnalise\spec-infrainal\gera2.py`) a partir do levantamento automático (`catalogo.json`: definição, sítios de emissão por grep com o método que os contém, referências em `crates/`). A coluna "condição" traz a **mensagem oficial** do código (o texto do `messages.yaml`), não a condição lida no método emissor; a coluna "posição" está **não verificada** em todas as linhas. Antes de implementar um código deste lote, abra o emissor citado e escreva os seis campos (o molde está em `docs/ANALYZER-ESPECIFICACAO.md`).

#### `analyzer/lib/src/error/best_practices_verifier.dart` (23)

| código | constante | emissor (arquivo:linha, método) | condição (mensagem oficial) | posição | DartForge |
|---|---|---|---|---|---|
| `assignment_of_do_not_store` | `WarningCode.ASSIGNMENT_OF_DO_NOT_STORE` | `analyzer/lib/src/error/best_practices_verifier.dart:845` `BestPracticesVerifier._checkForAssignmentOfDoNotStore` | '{0}' is marked 'doNotStore' and shouldn't be assigned to a field or top-level variable. | não verificado | **não implementado** |
| `deprecated_colon_for_default_value` | `HintCode.DEPRECATED_COLON_FOR_DEFAULT_VALUE` | `analyzer/lib/src/error/best_practices_verifier.dart:298` `BestPracticesVerifier.visitDefaultFormalParameter` | Using a colon as the separator before a default value is deprecated and will not be supported in language version 3.0 and later. | não verificado | escrito em 2026-10-05, não compilado: `crates/analise/src/fases.rs` `dois_pontos_no_padrao` com a versão da biblioteca |
| `import_deferred_library_with_load_function` | `HintCode.IMPORT_DEFERRED_LIBRARY_WITH_LOAD_FUNCTION` | `analyzer/lib/src/error/best_practices_verifier.dart:1144` `BestPracticesVerifier._checkForLoadLibraryFunction` | The imported library defines a top-level function named 'loadLibrary' that is hidden by deferring this library. | não verificado | **não implementado** |
| `inference_failure_on_function_return_type` | `WarningCode.INFERENCE_FAILURE_ON_FUNCTION_RETURN_TYPE` | `analyzer/lib/src/error/best_practices_verifier.dart:1419` `BestPracticesVerifier._checkStrictInferenceReturnType`; `analyzer/lib/src/error/best_practices_verifier.dart:1425` `BestPracticesVerifier._checkStrictInferenceReturnType`; `analyzer/lib/src/error/best_practices_verifier.dart:1431` `BestPracticesVerifier._checkStrictInferenceReturnType` | The return type of '{0}' cannot be inferred. | não verificado | **não implementado** |
| `inference_failure_on_untyped_parameter` | `WarningCode.INFERENCE_FAILURE_ON_UNTYPED_PARAMETER` | `analyzer/lib/src/error/best_practices_verifier.dart:1388` `BestPracticesVerifier._checkStrictInferenceInParameters` | The type of {0} can't be inferred; a type must be explicitly provided. | não verificado | **não implementado** |
| `invalid_export_of_internal_element` | `WarningCode.INVALID_EXPORT_OF_INTERNAL_ELEMENT` | `analyzer/lib/src/error/best_practices_verifier.dart:960` `BestPracticesVerifier._checkForInternalExport`; `analyzer/lib/src/error/best_practices_verifier.dart:970` `BestPracticesVerifier._checkForInternalExport` | The member '{0}' can't be exported as a part of a package's public API. | não verificado | **não implementado** |
| `invalid_export_of_internal_element_indirectly` | `WarningCode.INVALID_EXPORT_OF_INTERNAL_ELEMENT_INDIRECTLY` | `analyzer/lib/src/error/best_practices_verifier.dart:984` `BestPracticesVerifier._checkForInternalExport` | The member '{0}' can't be exported as a part of a package's public API, but is indirectly exported as part of the signature of '{1}'. | não verificado | **não implementado** |
| `invalid_override_of_non_virtual_member` | `WarningCode.INVALID_OVERRIDE_OF_NON_VIRTUAL_MEMBER` | `analyzer/lib/src/error/best_practices_verifier.dart:414` `BestPracticesVerifier.visitFieldDeclaration`; `analyzer/lib/src/error/best_practices_verifier.dart:612` `BestPracticesVerifier.visitMethodDeclaration` | The member '{0}' is declared non-virtual in '{1}' and can't be overridden in subclasses. | não verificado | **não implementado** |
| `invalid_required_named_param` | `WarningCode.INVALID_REQUIRED_NAMED_PARAM` | `analyzer/lib/src/error/best_practices_verifier.dart:1346` `BestPracticesVerifier._checkRequiredParameter` | The type parameter '{0}' is annotated with @required but only named parameters without a default value can be annotated with it. | não verificado | **não implementado** |
| `invalid_required_optional_positional_param` | `WarningCode.INVALID_REQUIRED_OPTIONAL_POSITIONAL_PARAM` | `analyzer/lib/src/error/best_practices_verifier.dart:1332` `BestPracticesVerifier._checkRequiredParameter` | Incorrect use of the annotation @required on the optional positional parameter '{0}'. Optional positional parameters cannot be required. | não verificado | **não implementado** |
| `invalid_required_positional_param` | `WarningCode.INVALID_REQUIRED_POSITIONAL_PARAM` | `analyzer/lib/src/error/best_practices_verifier.dart:1339` `BestPracticesVerifier._checkRequiredParameter` | Redundant use of the annotation @required on the required positional parameter '{0}'. | não verificado | **não implementado** |
| `invalid_use_of_internal_member` | `WarningCode.INVALID_USE_OF_INTERNAL_MEMBER` | `analyzer/lib/src/error/best_practices_verifier.dart:1648` `_InvalidAccessVerifier.verifyImport`; `analyzer/lib/src/error/best_practices_verifier.dart:1701` `_InvalidAccessVerifier.verifyPatternField`; `analyzer/lib/src/error/best_practices_verifier.dart:1720` `_InvalidAccessVerifier.verifySuperConstructorInvocation` | The member '{0}' can only be used within its package. | não verificado | **não implementado** |
| `invalid_use_of_protected_member` | `WarningCode.INVALID_USE_OF_PROTECTED_MEMBER` | `analyzer/lib/src/error/best_practices_verifier.dart:1893` `_InvalidAccessVerifier._checkForOtherInvalidAccess` | The member '{0}' can only be used within instance members of subclasses of '{1}'. | não verificado | **não implementado** |
| `invalid_use_of_visible_for_overriding_member` | `WarningCode.INVALID_USE_OF_VISIBLE_FOR_OVERRIDING_MEMBER` | `analyzer/lib/src/error/best_practices_verifier.dart:1631` `_InvalidAccessVerifier.verifyBinary`; `analyzer/lib/src/error/best_practices_verifier.dart:1917` `_InvalidAccessVerifier._checkForOtherInvalidAccess` | The member '{0}' can only be used for overriding. | não verificado | **não implementado** |
| `invalid_use_of_visible_for_template_member` | `WarningCode.INVALID_USE_OF_VISIBLE_FOR_TEMPLATE_MEMBER` | `analyzer/lib/src/error/best_practices_verifier.dart:1901` `_InvalidAccessVerifier._checkForOtherInvalidAccess` | The member '{0}' can only be used within '{1}' or a template library. | não verificado | **não implementado** |
| `invalid_use_of_visible_for_testing_member` | `WarningCode.INVALID_USE_OF_VISIBLE_FOR_TESTING_MEMBER` | `analyzer/lib/src/error/best_practices_verifier.dart:1909` `_InvalidAccessVerifier._checkForOtherInvalidAccess` | The member '{0}' can only be used within '{1}' or a test. | não verificado | **não implementado** |
| `mixin_on_sealed_class` | `WarningCode.MIXIN_ON_SEALED_CLASS` | `analyzer/lib/src/error/best_practices_verifier.dart:1015` `BestPracticesVerifier._checkForInvalidSealedSuperclass` | The class '{0}' shouldn't be used as a mixin constraint because it is sealed, and any class mixing in this mixin must have '{0}' as a superclass. | não verificado | **não implementado** |
| `must_be_immutable` | `WarningCode.MUST_BE_IMMUTABLE` | `analyzer/lib/src/error/best_practices_verifier.dart:943` `BestPracticesVerifier._checkForImmutable` | This class (or a class that this class inherits from) is marked as '@immutable', but one or more of its instance fields aren't final: {0} | não verificado | **não implementado** |
| `non_const_call_to_literal_constructor` | `WarningCode.NON_CONST_CALL_TO_LITERAL_CONSTRUCTOR` | `analyzer/lib/src/error/best_practices_verifier.dart:1114` `BestPracticesVerifier._checkForLiteralConstructorUse` | This instance creation must be 'const', because the {0} constructor is marked as '@literal'. | não verificado | **não implementado** |
| `non_const_call_to_literal_constructor` | `WarningCode.NON_CONST_CALL_TO_LITERAL_CONSTRUCTOR_USING_NEW` | `analyzer/lib/src/error/best_practices_verifier.dart:1113` `BestPracticesVerifier._checkForLiteralConstructorUse` | This instance creation must be 'const', because the {0} constructor is marked as '@literal'. | não verificado | **não implementado** |
| `return_of_do_not_store` | `WarningCode.RETURN_OF_DO_NOT_STORE` | `analyzer/lib/src/error/best_practices_verifier.dart:1223` `BestPracticesVerifier._checkForReturnOfDoNotStore` | '{0}' is annotated with 'doNotStore' and shouldn't be returned unless '{1}' is also annotated. | não verificado | **não implementado** |
| `subtype_of_sealed_class` | `WarningCode.SUBTYPE_OF_SEALED_CLASS` | `analyzer/lib/src/error/best_practices_verifier.dart:1022` `BestPracticesVerifier._checkForInvalidSealedSuperclass` | The class '{0}' shouldn't be extended, mixed in, or implemented because it's sealed. | não verificado | **não implementado** |
| `unnecessary_cast` | `WarningCode.UNNECESSARY_CAST` | `analyzer/lib/src/error/best_practices_verifier.dart:139` `BestPracticesVerifier.visitAsExpression` | Unnecessary cast. | não verificado | **não implementado** |

#### `analyzer/lib/src/error/annotation_verifier.dart` (11)

| código | constante | emissor (arquivo:linha, método) | condição (mensagem oficial) | posição | DartForge |
|---|---|---|---|---|---|
| `invalid_annotation_target` | `WarningCode.INVALID_ANNOTATION_TARGET` | `analyzer/lib/src/error/annotation_verifier.dart:180` `AnnotationVerifier._checkKinds`; `analyzer/lib/src/error/annotation_verifier.dart:237` `AnnotationVerifier._checkRedeclare` | The annotation '{0}' can only be used on {1}. | não verificado | **não implementado** |
| `invalid_factory_method_decl` | `WarningCode.INVALID_FACTORY_METHOD_DECL` | `analyzer/lib/src/error/annotation_verifier.dart:79` `AnnotationVerifier._checkFactory` | Factory method '{0}' must have a return type. | não verificado | **não implementado** |
| `invalid_factory_method_impl` | `WarningCode.INVALID_FACTORY_METHOD_IMPL` | `analyzer/lib/src/error/annotation_verifier.dart:109` `AnnotationVerifier._checkFactory` | Factory method '{0}' doesn't return a newly allocated object. | não verificado | **não implementado** |
| `invalid_internal_annotation` | `WarningCode.INVALID_INTERNAL_ANNOTATION` | `analyzer/lib/src/error/annotation_verifier.dart:126` `AnnotationVerifier._checkInternal`; `analyzer/lib/src/error/annotation_verifier.dart:136` `AnnotationVerifier._checkInternal`; `analyzer/lib/src/error/annotation_verifier.dart:145` `AnnotationVerifier._checkInternal` | Only public elements in a package's private API can be annotated as being internal. | não verificado | **não implementado** |
| `invalid_literal_annotation` | `WarningCode.INVALID_LITERAL_ANNOTATION` | `analyzer/lib/src/error/annotation_verifier.dart:195` `AnnotationVerifier._checkLiteral` | Only const constructors can have the `@literal` annotation. | não verificado | **não implementado** |
| `invalid_non_virtual_annotation` | `WarningCode.INVALID_NON_VIRTUAL_ANNOTATION` | `analyzer/lib/src/error/annotation_verifier.dart:208` `AnnotationVerifier._checkNonVirtual`; `analyzer/lib/src/error/annotation_verifier.dart:218` `AnnotationVerifier._checkNonVirtual`; `analyzer/lib/src/error/annotation_verifier.dart:224` `AnnotationVerifier._checkNonVirtual` | The annotation '@nonVirtual' can only be applied to a concrete instance member. | não verificado | **não implementado** |
| `invalid_reopen_annotation` | `WarningCode.INVALID_REOPEN_ANNOTATION` | `analyzer/lib/src/error/annotation_verifier.dart:273` `AnnotationVerifier._checkReopen`; `analyzer/lib/src/error/annotation_verifier.dart:280` `AnnotationVerifier._checkReopen`; `analyzer/lib/src/error/annotation_verifier.dart:288` `AnnotationVerifier._checkReopen` | The annotation '@reopen' can only be applied to a class that opens capabilities that the supertype intentionally disallows. | não verificado | **não implementado** |
| `invalid_visibility_annotation` | `WarningCode.INVALID_VISIBILITY_ANNOTATION` | `analyzer/lib/src/error/annotation_verifier.dart:343` `AnnotationVerifier._checkVisibility` | The member '{0}' is annotated with '{1}', but this annotation is only meaningful on declarations of public members. | não verificado | **não implementado** |
| `invalid_visible_for_overriding_annotation` | `WarningCode.INVALID_VISIBLE_FOR_OVERRIDING_ANNOTATION` | `analyzer/lib/src/error/annotation_verifier.dart:351` `AnnotationVerifier._checkVisibility` | The annotation 'visibleForOverriding' can only be applied to a public instance member that can be overridden. | não verificado | **não implementado** |
| `invalid_visible_outside_template_annotation` | `WarningCode.INVALID_VISIBLE_OUTSIDE_TEMPLATE_ANNOTATION` | `analyzer/lib/src/error/annotation_verifier.dart:409` `AnnotationVerifier._checkVisibleOutsideTemplate` | The annotation 'visibleOutsideTemplate' can only be applied to a member of a class, enum, or mixin that is annotated with 'visibleForTemplate'. | não verificado | **não implementado** |
| `undefined_referenced_parameter` | `WarningCode.UNDEFINED_REFERENCED_PARAMETER` | `analyzer/lib/src/error/annotation_verifier.dart:325` `AnnotationVerifier._checkUseResult` | The parameter '{0}' isn't defined by '{1}'. | não verificado | **não implementado** |

### II.8 — Verificadores de aviso: imports, deprecated, resultado não usado, must_call_super, redeclare, código morto, Unicode, restrição de SDK, ignores e TODOs

Lote de **31 constantes** (27 nomes emitidos distintos) dos verificadores menores. Duas fases diferentes do `LibraryAnalyzer` as emitem, e a fase decide as supressões:

- **Fase de erros** (`_computeVerifyErrors`, `analyzer/lib/src/dart/analysis/library_analyzer.dart:420-457`, roda sempre): o `ErrorVerifier` chama `UseResultVerifier` (`unused_result`), `RequiredParametersVerifier` (`missing_required_param`), `TypeArgumentsVerifier` (`strict_raw_type`), `ReturnTypeVerifier` (`return_in_generative_constructor`); o `AssignmentVerifier` (`assignment_to_*`) roda ainda antes, na resolução (`analyzer/lib/src/dart/resolver/property_element_resolver.dart:282`, `:519`, `:727`). Estes avisos NÃO dependem de `_analysisOptions.warning`.
- **Fase de avisos** (`_computeWarnings`, `library_analyzer.dart:459-536`, só se `_analysisOptions.warning`, `:315`), por arquivo, nesta ordem: `UnicodeTextVerifier` (`:466`) → `DeadCodeVerifier` (`:468`) → `BestPracticesVerifier` (`:470-481`; hospeda `DeprecatedMemberUseVerifier` e `MustCallSuperVerifier`) → `OverrideVerifier` (`:483`) → `RedeclareVerifier` (`:489`) → `TodoFinder` (`:495`) → `LanguageVersionOverrideVerifier` (`:496`) → `ImportsVerifier` (`:499-510`, atrás da porta `_hasDiagnosticReportedThatPreventsImportWarnings`, `:588-613`) → `UnusedLocalElementsVerifier` (`:513-520`) → `SdkConstraintVerifier` (`:526-535`, só com `sdkVersionConstraint` do pubspec).
- **Depois de tudo** (avisos e lints), ainda em `_computeDiagnostics`: `IgnoreValidator(...).reportErrors()` por arquivo (`library_analyzer.dart:341-349`), antes do filtro dos ignorados (`_filterIgnoredErrors`, chamado em `:115`).

Achados transversais do lote: (1) os relatos vão para um `RecordingErrorListener` que guarda um **`Set<AnalysisError>`** (`analyzer/lib/error/listener.dart:431-453`), com igualdade por código + offset + length + mensagem + fonte (`analyzer/lib/error/error.dart:232-254`): dois caminhos que relatam o mesmo código na mesma posição dão **um** diagnóstico (vale para `assignment_to_*`, `sdk_version_since`, `deprecated_member_use*`); (2) a ordem de emissão não aparece na saída do `dart analyze`, que ordena por severidade, arquivo, offset e mensagem (`dartdev/lib/src/analysis_server.dart:386-402`, usado em `dartdev/lib/src/commands/analyze.dart:241-242`); (3) os `TodoCode` têm severidade INFO e tipo TODO e o `dart analyze` os descarta (`dartdev/lib/src/commands/analyze.dart:189-192`); (4) nenhum verificador deste lote tem porta de erro de sintaxe (T5), só o `ImportsVerifier` tem a porta dos nomes não resolvidos.

#### ImportsVerifier (`analyzer/lib/src/error/imports_verifier.dart`)

Um `ImportsVerifier` por arquivo (`library_analyzer.dart:500-502`), alimentado só com as diretivas daquela unidade (`addImports(unit)`, `:503`). Sequência fixa dos relatos (`library_analyzer.dart:504-509`): `duplicate_export` → `duplicate_import` → `duplicate_hidden_name` → `duplicate_shown_name` → `unused_import` → `unused_shown_name` → `unnecessary_import`. A ordem importa porque `generateUnusedImportHints` preenche `_unusedImports`, que `generateUnusedShownNameHints` e `generateUnnecessaryImportHints` consultam.

```text
addImports(unit)                                         // imports_verifier.dart:60-104
  para cada diretiva, na ordem do arquivo:
    import: lib = directive.element?.importedLibrary; nula → pula; lib.isSynthetic → pula   // :65-71
            _allImports += directive; importsWithLibraries += (directive, lib)
    export: lib = directive.element?.exportedLibrary; nula → pula                           // :80-83 (sintética NÃO é pulada)
            exportsWithLibraries += (directive, lib)
    import ou export: _addDuplicateShownHiddenNames(directive)                               // :92-94, mesmo com lib nula
  _duplicateImports = _duplicates(importsWithLibraries)
  _duplicateExports = _duplicates(exportsWithLibraries)

_duplicates(lista)                                       // :368-397
  se len <= 1: vazio
  ordena por libraryUriStr = '${library.source.uri}' (URI absoluta)                          // :373-375, :411
  atual = lista[0]
  para i = 1..: prox = lista[i]
    se atual.uri == prox.uri E areSyntacticallyIdenticalExceptUri(atual.node, prox.node):
        duplicata = a de MAIOR offset entre as duas                                          // :387-391
    atual = prox                                         // só pares consecutivos da lista ordenada

areSyntacticallyIdenticalExceptUri(n1, n2)               // analyzer/lib/src/dart/ast/ast.dart:10005-10055
  imports com prefix?.name diferente → false             // `deferred` e as configurações `if (...)` não entram
  número de combinadores diferente → false
  par a par, na ordem: mesmo tipo (show/show, hide/hide) e mesma lista de nomes NA MESMA ORDEM; senão false
```

| código | constante | emissor (arquivo:linha, método) | condição | posição | DartForge |
|---|---|---|---|---|---|
| `duplicate_export` | `WarningCode.DUPLICATE_EXPORT` | `analyzer/lib/src/error/imports_verifier.dart:113-116` `ImportsVerifier.generateDuplicateExportWarnings` | cada diretiva em `_duplicateExports`: export cuja biblioteca (URI absoluta) e combinadores coincidem com os de um export anterior do mesmo arquivo | `atNode(directive.uri)`: o literal de string do URI, com aspas | **não implementado** — nenhum emissor em `crates/` |
| `duplicate_hidden_name` | `WarningCode.DUPLICATE_HIDDEN_NAME` | `analyzer/lib/src/error/imports_verifier.dart:146-149` `ImportsVerifier.generateDuplicateShownHiddenNameWarnings` (coleta em `_addDuplicateShownHiddenNames`, `:339-350`) | nome de um `hide` cujo `staticElement` (não nulo) já apareceu no MESMO combinador | `atNode(identifier)`: a segunda (terceira…) ocorrência | **não implementado** — nenhum emissor em `crates/` |
| `duplicate_import` | `WarningCode.DUPLICATE_IMPORT` | `analyzer/lib/src/error/imports_verifier.dart:127-130` `ImportsVerifier.generateDuplicateImportWarnings` | cada diretiva em `_duplicateImports`: import de biblioteca resolvida e não sintética, com mesma URI absoluta, mesmo prefixo e mesmos combinadores de um import anterior do mesmo arquivo | `atNode(directive.uri)`: o literal do URI da diretiva de maior offset | **não implementado** — nenhum emissor em `crates/` |
| `duplicate_shown_name` | `WarningCode.DUPLICATE_SHOWN_NAME` | `analyzer/lib/src/error/imports_verifier.dart:157-160` `ImportsVerifier.generateDuplicateShownHiddenNameWarnings` (coleta em `:351-362`) | nome de um `show` cujo `staticElement` (não nulo) já apareceu no MESMO combinador | `atNode(identifier)`: a ocorrência repetida | **não implementado** — nenhum emissor em `crates/` |
| `unnecessary_import` | `HintCode.UNNECESSARY_IMPORT` | `analyzer/lib/src/error/imports_verifier.dart:222-229` `ImportsVerifier.generateUnnecessaryImportHints` | import usado, não `dart:core`, para o qual existe outro import usado do mesmo prefixo cujo conjunto de elementos usados contém o dele e é estritamente maior | `atNode(firstDirective.uri)`; argumentos `[relativeUriString do primeiro, relativeUriString do segundo]` | **não implementado** — nenhum emissor em `crates/` |
| `unused_shown_name` | `WarningCode.UNUSED_SHOWN_NAME` | `analyzer/lib/src/error/imports_verifier.dart:320-324` `ImportsVerifier.generateUnusedShownNameHints` | import usado (fora de `_unusedImports`), resolvido, não `dart:core`; nome de `show` com elemento não nulo que não está no conjunto de elementos usados do import (para variável/campo: nem o getter nem o setter) | `atNode(identifier)`; argumento `identifier.name` | **emitido, não publicado** — `crates/analise/src/importacoes.rs:184` (uso aproximado por nome citado; fora de `verificados.txt`) |

#### DeprecatedMemberUseVerifier (`analyzer/lib/src/error/deprecated_member_use_verifier.dart`)

Instanciado pelo `BestPracticesVerifier` (`analyzer/lib/src/error/best_practices_verifier.dart:107-109`) com o `workspacePackage` da biblioteca analisada (`library_analyzer.dart:480`). O `BestPracticesVerifier` é um `RecursiveAstVisitor` que, em cada `visitX`, chama o método homônimo do verificador (`assignmentExpression` `:156`, `binaryExpression` `:162`, `constructorName` `:283`, `exportDirective` `:331`, `extensionOverride` `:358`, `functionExpressionInvocation` `:484`, `importDirective` `:537`, `indexExpression` `:549`, `instanceCreationExpression` `:555`, `methodInvocation` `:629`, `namedType` `:660`, `patternField` `:682`, `postfixExpression` `:689`, `prefixExpression` `:702`, `redirectingConstructorInvocation` `:709`, `simpleIdentifier` `:729`, `superConstructorInvocation` `:736`) e mantém a pilha `_inDeprecatedMemberStack` em volta das declarações. As quatro constantes saem do único `reportError` (`deprecated_member_use_verifier.dart:346-373`); `DEPRECATED_MEMBER_USE` e `DEPRECATED_MEMBER_USE_WITH_MESSAGE` (fora deste lote) são as do ramo "outro pacote" e estão em `docs/ANALYZER-ESPECIFICACAO.md` (§F, `deprecated_member_use`).

| código | constante | emissor (arquivo:linha, método) | condição | posição | DartForge |
|---|---|---|---|---|---|
| `deprecated_member_use_from_same_package` | `HintCode.DEPRECATED_MEMBER_USE_FROM_SAME_PACKAGE` | `analyzer/lib/src/error/deprecated_member_use_verifier.dart:352-358` `DeprecatedMemberUseVerifier.reportError` | uso de elemento depreciado (ver especificação), biblioteca do elemento dentro do pacote do workspace (`_isLibraryInWorkspacePackage`, `:375-382`) e mensagem do `@Deprecated` nula, vazia ou `.` depois do `trim` | `atEntity(errorEntity)`: depende da forma da referência (tabela na especificação); argumento `displayName` | **não implementado** — nenhum emissor em `crates/` |
| `deprecated_member_use_from_same_package` | `HintCode.DEPRECATED_MEMBER_USE_FROM_SAME_PACKAGE_WITH_MESSAGE` | `analyzer/lib/src/error/deprecated_member_use_verifier.dart:365-371` `DeprecatedMemberUseVerifier.reportError` | idem, com mensagem não vazia; a mensagem recebe `.` no fim se não termina em `.`, `?` ou `!` (`:360-364`) | idem; argumentos `[displayName, mensagem]` | **não implementado** — nenhum emissor em `crates/` |

#### UseResultVerifier (`analyzer/lib/src/error/use_result_verifier.dart`)

Campo `_checkUseVerifier` do `ErrorVerifier` (`analyzer/lib/src/generated/error_verifier.dart:252`, `:269`), chamado em `visitFunctionExpressionInvocation` (`:1005`), `visitInstanceCreationExpression` (`:1111`, só se o tipo criado é `InterfaceType`), `visitMethodInvocation` (`:1207`), `visitPropertyAccess` (`:1356`) e `visitSimpleIdentifier` (`:1428`). Apesar de `WarningCode`, sai na fase de erros.

| código | constante | emissor (arquivo:linha, método) | condição | posição | DartForge |
|---|---|---|---|---|---|
| `unused_result` | `WarningCode.UNUSED_RESULT` | `analyzer/lib/src/error/use_result_verifier.dart:111-115` `UseResultVerifier._check` | elemento referenciado tem `@useResult`/`@UseResult(...)`, o nó não está "usado" (`_isUsed`, `:189-266`), não passa o parâmetro de `UseResult.unless`, e a anotação não traz mensagem | `atNode(_getNodeToAnnotate(node))`: `methodName`, `propertyName`, ou o nó; argumento `displayName` | **não implementado** — nenhum emissor em `crates/` |
| `unused_result` | `WarningCode.UNUSED_RESULT_WITH_MESSAGE` | `analyzer/lib/src/error/use_result_verifier.dart:117-121` `UseResultVerifier._check` | idem, com `UseResult('msg')` de mensagem não vazia | idem; argumentos `[displayName, message]` (o molde acrescenta `.` depois de `{1}`) | **não implementado** — nenhum emissor em `crates/` |

#### MustCallSuperVerifier (`analyzer/lib/src/error/must_call_super_verifier.dart`)

Chamado de `BestPracticesVerifier.visitMethodDeclaration` (`analyzer/lib/src/error/best_practices_verifier.dart:581`), primeira checagem do método.

| código | constante | emissor (arquivo:linha, método) | condição | posição | DartForge |
|---|---|---|---|---|---|
| `must_call_super` | `WarningCode.MUST_CALL_SUPER` | `analyzer/lib/src/error/must_call_super_verifier.dart:146-150` `MustCallSuperVerifier._verifySuperIsCalled` | método/getter/setter de instância não abstrato que sobrescreve (por superclasse, mixins ou restrições `on`) um membro com `@mustCallSuper`, tem implementação concreta herdada, e cujo corpo não contém `super.<mesmo nome>` (`invokesSuperSelf`) | `atToken(node.name)`; argumento = nome do elemento que declara o membro anotado | **não implementado** — nenhum emissor em `crates/` |

#### RedeclareVerifier (`analyzer/lib/src/error/redeclare_verifier.dart`)

`RecursiveAstVisitor` próprio (`library_analyzer.dart:489-493`), depois do `OverrideVerifier`. Só olha `MethodDeclaration` dentro de `ExtensionTypeDeclaration` (`redeclare_verifier.dart:33-42`).

| código | constante | emissor (arquivo:linha, método) | condição | posição | DartForge |
|---|---|---|---|---|---|
| `redeclare_on_non_redeclaring_member` | `WarningCode.REDECLARE_ON_NON_REDECLARING_MEMBER` | `analyzer/lib/src/error/redeclare_verifier.dart:51-57` (método), `:60-66` (getter), `:68-74` (setter) `RedeclareVerifier.visitMethodDeclaration` | membro não estático de extension type com `@redeclare` cujo `Name(libraryUri, element.name)` não é chave de `inheritance.getInterface(extensionType).redeclared` (`:81-91`) | `atToken(node.name)`; argumento `'method'`, `'getter'` ou `'setter'` (usado duas vezes no molde) | **não implementado** — nenhum emissor em `crates/` |

#### DeadCodeVerifier (`analyzer/lib/src/error/dead_code_verifier.dart`)

Só a constante de combinador pertence a este lote (o `dead_code` de fluxo é do `NullSafetyDeadCodeVerifier`, especificado em §B). `_checkCombinator` (`dead_code_verifier.dart:133-157`) é chamado de `visitImportDirective` (`:81-94`) e `visitExportDirective` (`:53-65`) quando a biblioteca alvo existe e não é sintética. Fora da porta dos imports.

| código | constante | emissor (arquivo:linha, método) | condição | posição | DartForge |
|---|---|---|---|---|---|
| `undefined_hidden_name` | `WarningCode.UNDEFINED_HIDDEN_NAME` | `analyzer/lib/src/error/dead_code_verifier.dart:150-154` `DeadCodeVerifier._checkCombinator` (código escolhido em `:140`) | nome de `hide` (import ou export) para o qual o namespace de exportação da biblioteca alvo não tem `nome` nem `nome=` (`:147-149`) | `atNode(name)`; argumentos `[library.identifier, nome]` (`identifier` = `'${source.uri}'`, `analyzer/lib/src/dart/element/element.dart:5903`) | **não implementado** — nenhum emissor em `crates/` (`crates/analise/src/importacoes.rs:152` só usa `hide` como filtro de visibilidade) |

#### UnusedLocalElementsVerifier (`analyzer/lib/src/error/unused_local_elements_verifier.dart`)

Coleta de uso por `GatherUsedLocalElementsVisitor` de todos os arquivos da biblioteca (`library_analyzer.dart:316-324`), verificação por arquivo (`:513-520`). O resto do verificador (`unused_local_variable`, `unused_element`, `unused_field`) está em `docs/ANALYZER-ESPECIFICACAO.md` §F.

| código | constante | emissor (arquivo:linha, método) | condição | posição | DartForge |
|---|---|---|---|---|---|
| `unused_catch_clause` | `WarningCode.UNUSED_CATCH_CLAUSE` | `analyzer/lib/src/error/unused_local_elements_verifier.dart:1049` (relato em `:1055`) `UnusedLocalElementsVerifier._visitLocalVariableElement` | variável de exceção de `on T catch (e)` sem pilha, nunca lida e que não é só `_`; com pilha ou sem `on` a exceção é marcada usada na coleta (`:57-66`) | `_reportErrorForElement` (`:993-1004`): `element.nameOffset`, `element.nameLength`; argumento `element.displayName` | **publicado** — `crates/analise/src/locais.rs:876` (declaração em `:671`) |
| `unused_catch_stack` | `WarningCode.UNUSED_CATCH_STACK` | `analyzer/lib/src/error/unused_local_elements_verifier.dart:1051` (relato em `:1055`) `UnusedLocalElementsVerifier._visitLocalVariableElement` | variável de pilha de `catch (e, s)` (`addCatchStackTrace`, `:67-70`) nunca lida e que não é só `_` (`_isNamedWildcard`, `:775`) | idem: nome da variável de pilha; argumento `element.displayName` | **publicado** — `crates/analise/src/locais.rs:877` |

#### UnicodeTextVerifier (`analyzer/lib/src/error/unicode_text_verifier.dart`)

Primeiro verificador de `_computeWarnings` (`library_analyzer.dart:466`); varre o **texto** do arquivo unidade de código UTF-16 a unidade (`unicode_text_verifier.dart:17-18`) e usa a AST só para classificar.

| código | constante | emissor (arquivo:linha, método) | condição | posição | DartForge |
|---|---|---|---|---|---|
| `text_direction_code_point_in_comment` | `WarningCode.TEXT_DIRECTION_CODE_POINT_IN_COMMENT` | `analyzer/lib/src/error/unicode_text_verifier.dart:33-38` `UnicodeTextVerifier.verify` (código escolhido em `:28-31`) | unidade de código em U+202A..U+202E ou U+2066..U+2069 (`:20-22`) e o nó mais interno que cobre o offset (`NodeLocator(offset).searchWithin(unit)`, `:24`) NÃO é `SimpleStringLiteral` nem `InterpolationString` | `atOffset(offset: offset, length: 1)`; argumento = código em hexadecimal maiúsculo sem prefixo (`202E`) | **não implementado** — nenhum emissor em `crates/` |
| `text_direction_code_point_in_literal` | `WarningCode.TEXT_DIRECTION_CODE_POINT_IN_LITERAL` | `analyzer/lib/src/error/unicode_text_verifier.dart:33-38` `UnicodeTextVerifier.verify` (código escolhido em `:28-31`) | mesma faixa, e o nó mais interno É `SimpleStringLiteral` ou `InterpolationString` | idem | **não implementado** — nenhum emissor em `crates/` |

#### SdkConstraintVerifier (`analyzer/lib/src/hint/sdk_constraint_verifier.dart`)

Último passo de `_computeWarnings` (`library_analyzer.dart:526-535`): só roda quando `fileAnalysis.file.workspacePackage` é `PubPackage` e `package.sdkVersionConstraint` não é nulo, isto é, o `pubspec.yaml` tem `environment: sdk:` que o `VersionConstraint.parse` aceita (`analyzer/lib/src/workspace/pub.dart:448-462`). A restrição passa por `withoutPreRelease` (`analyzer/lib/src/utilities/extensions/version.dart:12-26`).

| código | constante | emissor (arquivo:linha, método) | condição | posição | DartForge |
|---|---|---|---|---|---|
| `sdk_version_gt_gt_gt_operator` | `WarningCode.SDK_VERSION_GT_GT_GT_OPERATOR` | `analyzer/lib/src/hint/sdk_constraint_verifier.dart:82-85` `SdkConstraintVerifier.visitBinaryExpression` e `:117-120` `visitMethodDeclaration` | `checkTripleShift` (a restrição intersecta `<2.14.0`, `:53-54`) e: operador binário `>>>`, ou declaração de operador de nome `>>>` | `atToken(node.operator)` no uso; `atToken(node.name)` na declaração | **não implementado** — nenhum emissor em `crates/` |
| `sdk_version_since` | `WarningCode.SDK_VERSION_SINCE` | `analyzer/lib/src/hint/sdk_constraint_verifier.dart:200-207` `SdkConstraintVerifier._checkSinceSdkVersion` | elemento referenciado com `sinceSdkVersion` não nulo (`@Since` em biblioteca `dart:`) e `!_versionConstraint.requiresAtLeast(since)` (`:167-170`) | `atEntity(errorEntity)`: conforme o nó (`:175-198`); argumentos `[since.toString(), constraint.toString()]` | **não implementado** — nenhum emissor em `crates/` |

#### IgnoreValidator (`analyzer/lib/src/error/ignore_validator.dart`)

Roda por arquivo no fim de `_computeDiagnostics` (`library_analyzer.dart:341-349`), sem depender de `_analysisOptions.warning`, sobre o `IgnoreInfo` do arquivo (`analyzer/lib/src/dart/analysis/file_analysis.dart:28`). No 3.6.2 só `duplicate_ignore` é relatado: o trecho de `UNIGNORABLE_IGNORE` está comentado (`ignore_validator.dart:118-125`) e o corpo inteiro de `_reportUnnecessaryOrRemovedOrDeprecatedIgnores` (`UNNECESSARY_IGNORE`, `REMOVED_LINT_USE`, `REPLACED_LINT_USE`) também (`:149-183`).

| código | constante | emissor (arquivo:linha, método) | condição | posição | DartForge |
|---|---|---|---|---|---|
| `duplicate_ignore` | `WarningCode.DUPLICATE_IGNORE` | `analyzer/lib/src/error/ignore_validator.dart:129-134` (nome) e `:137-142` (`type=`) `IgnoreValidator._reportUnignorableAndDuplicateIgnores` | nome (ou `type=x`) repetido nos `ignore_for_file` do arquivo, ou num `ignore` de linha quando já está nos `ignore_for_file` ou já apareceu entre os `ignore` que valem para a mesma linha; nomes de `unignorableNames` ficam fora | `atOffset(offset: ignoredElement.offset, length: name.length)` para nome; para tipo, `length` de `type` até o fim da palavra do tipo; argumento = nome ou tipo em minúsculas | **não implementado** — nenhum emissor em `crates/` (`crates/paridade/src/filtros.rs:168` só interpreta os ignores para filtrar) |

#### RequiredParametersVerifier (`analyzer/lib/src/error/required_parameters_verifier.dart`)

Campo do `ErrorVerifier` (`analyzer/lib/src/generated/error_verifier.dart:270`), chamado em `visitAnnotation` (`:336`), `visitEnumConstantDeclaration` (`:664`), `visitFunctionExpressionInvocation` (`:1003`), `visitInstanceCreationExpression` (`:1109`), `visitMethodInvocation` (`:1205`), `visitRedirectingConstructorInvocation` (`:1363`), `visitSuperConstructorInvocation` (`:1442`). O mesmo `_check` emite `MISSING_REQUIRED_ARGUMENT` (`required_parameters_verifier.dart:122-131`, §A do outro documento) e, para parâmetro nomeado **opcional** com `@required` do `package:meta`, as duas constantes abaixo.

| código | constante | emissor (arquivo:linha, método) | condição | posição | DartForge |
|---|---|---|---|---|---|
| `missing_required_param` | `WarningCode.MISSING_REQUIRED_PARAM` | `analyzer/lib/src/error/required_parameters_verifier.dart:147-151` `RequiredParametersVerifier._check` | parâmetro `isOptionalNamed` com anotação `isRequired` (`@required` ou `Required(...)`, `:189-194`), sem argumento nomeado correspondente nem `super.nome` no construtor envolvente (`_containsNamedExpression`, `:159-179`), e `reason` nula ou vazia | `atEntity(errorEntity)`: a mesma entidade do `missing_required_argument` do chamador (`methodName`, `constructorName`, nó da invocação…); argumento = nome do parâmetro | **não implementado** — nenhum emissor em `crates/` |
| `missing_required_param` | `WarningCode.MISSING_REQUIRED_PARAM_WITH_DETAILS` | `analyzer/lib/src/error/required_parameters_verifier.dart:141-145` `RequiredParametersVerifier._check` | idem, com `Required('motivo')` de `reason` não vazia (`getReason`, `:211-219`) | idem; argumentos `[nome, reason]` (o molde acrescenta `.` depois de `{1}`) | **não implementado** — nenhum emissor em `crates/` |

#### TypeArgumentsVerifier (`analyzer/lib/src/error/type_arguments_verifier.dart`)

Só `strict_raw_type` pertence ao lote. `checkNamedType` (`type_arguments_verifier.dart:199-206`), chamado de `ErrorVerifier.visitNamedType` (`analyzer/lib/src/generated/error_verifier.dart:1276`), chama `_checkForRawTypeName` para todo `NamedType` que não seja o tipo de um `ConstructorName` de `InstanceCreationExpression`.

| código | constante | emissor (arquivo:linha, método) | condição | posição | DartForge |
|---|---|---|---|---|---|
| `strict_raw_type` | `WarningCode.STRICT_RAW_TYPE` | `analyzer/lib/src/error/type_arguments_verifier.dart:258-262` `TypeArgumentsVerifier._checkForRawTypeName` | opção `strict-raw-types` ligada (`:243`; padrão `false`, `analyzer/lib/src/generated/engine.dart:234`), `NamedType` sem argumentos explícitos cujo tipo (ou alias) tem algum argumento `dynamic` e o elemento não tem `@optionalTypeArgs` (`_isMissingTypeArguments`, `:580-600`), fora de `as`, `is`, `CastPattern` e `ObjectPattern` | `atNode(node)`: o `NamedType`; argumento = o `DartType` (display, T7) | **não implementado** — nenhum emissor em `crates/` |

#### ReturnTypeVerifier (`analyzer/lib/src/error/return_type_verifier.dart`)

| código | constante | emissor (arquivo:linha, método) | condição | posição | DartForge |
|---|---|---|---|---|---|
| `return_in_generative_constructor` | `CompileTimeErrorCode.RETURN_IN_GENERATIVE_CONSTRUCTOR` | `analyzer/lib/src/error/return_type_verifier.dart:59-62` `ReturnTypeVerifier.verifyReturnStatement` (de `ErrorVerifier.visitReturnStatement`, `analyzer/lib/src/generated/error_verifier.dart:1386`); e `analyzer/lib/src/generated/error_verifier.dart:5225-5228` `ErrorVerifier._checkForReturnInGenerativeConstructor` (chamado em `:607`) | `return e;` com expressão dentro de construtor gerador (`enclosingExecutable.isGenerativeConstructor`); ou construtor não `factory` com corpo de expressão `=> e` | `atNode(expression)` no `return`; `atNode(body)` (o `ExpressionFunctionBody` inteiro) no `=>` | **publicado** — `crates/analise/src/membros.rs:224` (`return e;`) e `:362` (`=> e`) |

#### AssignmentVerifier (`analyzer/lib/src/error/assignment_verifier.dart`)

Chamado pelo `PropertyElementResolver` durante a resolução, quando o identificador é alvo de escrita e não há elemento gravável: identificador simples (`analyzer/lib/src/dart/resolver/property_element_resolver.dart:282-287`, com `requested`/`recovery` da busca léxica), propriedade de instância/extensão com `needsSetterError` (`:519-524`) e `Tipo.nome` estático (`:727-732`). Os mesmos três códigos saem de novo do `ErrorVerifier._checkForAssignmentToFinal` (`analyzer/lib/src/generated/error_verifier.dart:2138-2203`, chamado em `:363`, `:929`, `:1316`, `:1339`) sobre a expressão alvo inteira; quando o alvo é um identificador simples as duas posições coincidem e o `Set` do listener deixa um só.

| código | constante | emissor (arquivo:linha, método) | condição | posição | DartForge |
|---|---|---|---|---|---|
| `assignment_to_function` | `CompileTimeErrorCode.ASSIGNMENT_TO_FUNCTION` | `analyzer/lib/src/error/assignment_verifier.dart:57-60` `AssignmentVerifier.verify`; `analyzer/lib/src/generated/error_verifier.dart:2186-2189` `ErrorVerifier._checkForAssignmentToFinal` | escrita cujo elemento de recuperação é `FunctionElement` (função de topo ou local), sem setter | `atNode(node)`: o `SimpleIdentifier`; no `ErrorVerifier`, `atNode(expression)`: o alvo inteiro | **publicado** — `crates/types/src/inferencia/expr.rs:2624`, `:2761`, `:2889` (molde de texto de `crates/types/src/codes.rs:130`; código ligado em `crates/paridade/src/ponte.rs:89`) |
| `assignment_to_method` | `CompileTimeErrorCode.ASSIGNMENT_TO_METHOD` | `analyzer/lib/src/error/assignment_verifier.dart:62-65` `AssignmentVerifier.verify`; `analyzer/lib/src/generated/error_verifier.dart:2191-2194` `ErrorVerifier._checkForAssignmentToFinal` | escrita cujo elemento de recuperação é `MethodElement` | `atNode(node)`: o nome (`propertyName` em `o.m = …`); no `ErrorVerifier`, o alvo inteiro | **publicado** — `crates/types/src/inferencia/expr.rs:2638` (molde de `crates/types/src/codes.rs:50`; código ligado em `crates/paridade/src/ponte.rs:73`) |
| `assignment_to_type` | `CompileTimeErrorCode.ASSIGNMENT_TO_TYPE` | `analyzer/lib/src/error/assignment_verifier.dart:52-55` `AssignmentVerifier.verify`; `analyzer/lib/src/generated/error_verifier.dart:2198-2201` `ErrorVerifier._checkForAssignmentToFinal` | escrita cujo elemento de recuperação é `dynamic`, classe/mixin/enum/extension type (`InterfaceElement`), typedef (`TypeAliasElement`, só no `AssignmentVerifier`) ou parâmetro de tipo | `atNode(node)`: o identificador; no `ErrorVerifier`, o alvo inteiro | **publicado** — `crates/types/src/inferencia/expr.rs:2768`, `:2775` (molde de `crates/types/src/codes.rs:125`; código ligado em `crates/paridade/src/ponte.rs:88`) |

#### TodoFinder (`analyzer/lib/src/error/todo_finder.dart`, `analyzer/lib/src/dart/error/todo_codes.dart`)

`TodoFinder(errorReporter).findIn(unit)` (`library_analyzer.dart:495`). As quatro constantes saem do mesmo `atOffset` (`todo_finder.dart:131-136`), com o código escolhido por `Todo.forKind(kind)` (`todo_codes.dart:53`, mapa em `:13-18`). `TodoCode` tem molde `{0}`, severidade INFO (`todo_codes.dart:92`) e tipo `ErrorType.TODO` (`:95`).

| código | constante | emissor (arquivo:linha, método) | condição | posição | DartForge |
|---|---|---|---|---|---|
| `fixme` | `TodoCode.FIXME` | `analyzer/lib/src/error/todo_finder.dart:131-136` `TodoFinder._scrapeTodoComment` (código via `analyzer/lib/src/dart/error/todo_codes.dart:15`, `:53`) | casamento de `Todo.TODO_REGEX` num comentário, com a palavra `FIXME` | `atOffset(offset: início da palavra, length: end - offset)` (ver especificação de `todo`) | **não implementado** — nenhum emissor em `crates/` |
| `hack` | `TodoCode.HACK` | `analyzer/lib/src/error/todo_finder.dart:131-136` `TodoFinder._scrapeTodoComment` (código via `analyzer/lib/src/dart/error/todo_codes.dart:16`, `:53`) | idem, com a palavra `HACK` | idem | **não implementado** — nenhum emissor em `crates/` |
| `todo` | `TodoCode.TODO` | `analyzer/lib/src/error/todo_finder.dart:131-136` `TodoFinder._scrapeTodoComment` (código via `analyzer/lib/src/dart/error/todo_codes.dart:14`, `:53`) | idem, com a palavra `TODO` (também é o valor de recuo de `forKind`) | idem | **não implementado** — nenhum emissor em `crates/` |
| `undone` | `TodoCode.UNDONE` | `analyzer/lib/src/error/todo_finder.dart:131-136` `TodoFinder._scrapeTodoComment` (código via `analyzer/lib/src/dart/error/todo_codes.dart:17`, `:53`) | idem, com a palavra `UNDONE` | idem | **não implementado** — nenhum emissor em `crates/` |

### II.9 — Resolução, inferência, FFI e demais emissores

Lote `S4`: **36 constantes** (12 publicadas, 0 emitidas e não publicadas, 24 não implementadas no DartForge).
Esta tabela foi **gerada por script** (`E:\dftempnalise\spec-infrainal\gera2.py`) a partir do levantamento automático (`catalogo.json`: definição, sítios de emissão por grep com o método que os contém, referências em `crates/`). A coluna "condição" traz a **mensagem oficial** do código (o texto do `messages.yaml`), não a condição lida no método emissor; a coluna "posição" está **não verificada** em todas as linhas. Antes de implementar um código deste lote, abra o emissor citado e escreva os seis campos (o molde está em `docs/ANALYZER-ESPECIFICACAO.md`).

#### `analyzer/lib/src/dart/resolver/named_type_resolver.dart` (5)

| código | constante | emissor (arquivo:linha, método) | condição (mensagem oficial) | posição | DartForge |
|---|---|---|---|---|---|
| `cast_to_non_type` | `CompileTimeErrorCode.CAST_TO_NON_TYPE` | `analyzer/lib/src/dart/resolver/named_type_resolver.dart:550` `_ErrorHelper.reportNullOrNonTypeElement` | The name '{0}' isn't a type, so it can't be used in an 'as' expression. | não verificado | **publicado** — `crates/types/src/resolve.rs:144` |
| `non_type_in_catch_clause` | `CompileTimeErrorCode.NON_TYPE_IN_CATCH_CLAUSE` | `analyzer/lib/src/dart/resolver/named_type_resolver.dart:539` `_ErrorHelper.reportNullOrNonTypeElement` | The name '{0}' isn't a type and can't be used in an on-catch clause. | não verificado | **publicado** — `crates/types/src/resolve.rs:143` |
| `nullable_type_in_extends_clause` | `CompileTimeErrorCode.NULLABLE_TYPE_IN_EXTENDS_CLAUSE` | `analyzer/lib/src/dart/resolver/named_type_resolver.dart:395` `NamedTypeResolver._verifyNullability` | A class can't extend a nullable type. | não verificado | **publicado** — `crates/analise/src/clausulas.rs:1253` |
| `nullable_type_in_on_clause` | `CompileTimeErrorCode.NULLABLE_TYPE_IN_ON_CLAUSE` | `analyzer/lib/src/dart/resolver/named_type_resolver.dart:405` `NamedTypeResolver._verifyNullability` | A mixin can't have a nullable type as a superclass constraint. | não verificado | **publicado** — `crates/analise/src/clausulas.rs:1251` |
| `nullable_type_in_with_clause` | `CompileTimeErrorCode.NULLABLE_TYPE_IN_WITH_CLAUSE` | `analyzer/lib/src/dart/resolver/named_type_resolver.dart:410` `NamedTypeResolver._verifyNullability` | A class or mixin can't mix in a nullable type. | não verificado | **publicado** — `crates/analise/src/clausulas.rs:1249` |

#### `analyzer/lib/src/generated/ffi_verifier.dart` (5)

| código | constante | emissor (arquivo:linha, método) | condição (mensagem oficial) | posição | DartForge |
|---|---|---|---|---|---|
| `abi_specific_integer_invalid` | `FfiCode.ABI_SPECIFIC_INTEGER_INVALID` | `analyzer/lib/src/generated/ffi_verifier.dart:962` `FfiVerifier._validateAbiSpecificIntegerAnnotation` | Classes extending 'AbiSpecificInteger' must have exactly one const constructor, no other members, and no type parameters. | não verificado | **não implementado** |
| `address_receiver` | `FfiCode.ADDRESS_RECEIVER` | `analyzer/lib/src/generated/ffi_verifier.dart:1123` `FfiVerifier._validateAddressReceiver` | The receiver of '.address' must be a concrete 'TypedData', a concrete 'TypedData' '[]', an 'Array', an 'Array' '[]', a Struct field, or a Union field. | não verificado | **não implementado** |
| `compound_implements_finalizable` | `FfiCode.COMPOUND_IMPLEMENTS_FINALIZABLE` | `analyzer/lib/src/generated/ffi_verifier.dart:202` `FfiVerifier.visitClassDeclaration` | The class '{0}' can't implement Finalizable. | não verificado | **não implementado** |
| `ffi_native_only_classes_extending_nativefieldwrapperclass1_can_be_pointer` | `FfiCode.FFI_NATIVE_ONLY_CLASSES_EXTENDING_NATIVEFIELDWRAPPERCLASS1_CAN_BE_POINTER` | `analyzer/lib/src/generated/ffi_verifier.dart:637` `FfiVerifier._checkFfiNativeFunction`; `analyzer/lib/src/generated/ffi_verifier.dart:669` `FfiVerifier._checkFfiNativeFunction` | Only classes extending NativeFieldWrapperClass1 can be passed as Pointer. | não verificado | **não implementado** |
| `variable_length_array_not_last` | `FfiCode.VARIABLE_LENGTH_ARRAY_NOT_LAST` | `analyzer/lib/src/generated/ffi_verifier.dart:1990` `FfiVerifier._validateSizeOfAnnotation` | Variable length 'Array's must only occur as the last field of Structs. | não verificado | **não implementado** |

#### `analyzer/lib/src/generated/resolver.dart` (4)

| código | constante | emissor (arquivo:linha, método) | condição (mensagem oficial) | posição | DartForge |
|---|---|---|---|---|---|
| `augmented_expression_is_not_setter` | `CompileTimeErrorCode.AUGMENTED_EXPRESSION_IS_NOT_SETTER` | `analyzer/lib/src/generated/resolver.dart:1419` `ResolverVisitor.resolveForWrite` | The augmented declaration is not a setter, it can't be used to write a value. | não verificado | **não implementado** |
| `missing_named_pattern_field_name` | `CompileTimeErrorCode.MISSING_NAMED_PATTERN_FIELD_NAME` | `analyzer/lib/src/generated/resolver.dart:490` `ResolverVisitor.buildSharedPatternFields` | The getter name is not specified explicitly, and the pattern is not a variable. | não verificado | **não implementado** |
| `non_bool_expression` | `CompileTimeErrorCode.NON_BOOL_EXPRESSION` | `analyzer/lib/src/generated/resolver.dart:1899` `ResolverVisitor.visitAssertInitializer`; `analyzer/lib/src/generated/resolver.dart:1917` `ResolverVisitor.visitAssertStatement` | The expression in an assert must be of type 'bool'. | não verificado | **publicado** — `crates/types/src/inferencia/expr.rs:3421` |
| `positional_field_in_object_pattern` | `CompileTimeErrorCode.POSITIONAL_FIELD_IN_OBJECT_PATTERN` | `analyzer/lib/src/generated/resolver.dart:497` `ResolverVisitor.buildSharedPatternFields` | Object patterns can only use named fields. | não verificado | **não implementado** |

#### `analyzer/lib/src/dart/resolver/property_element_resolver.dart` (3)

| código | constante | emissor (arquivo:linha, método) | condição (mensagem oficial) | posição | DartForge |
|---|---|---|---|---|---|
| `private_setter` | `CompileTimeErrorCode.PRIVATE_SETTER` | `analyzer/lib/src/dart/resolver/property_element_resolver.dart:716` `PropertyElementResolver._resolveTargetInterfaceElement` | The setter '{0}' is private and can't be accessed outside the library that declares it. | não verificado | **não implementado** |
| `undefined_extension_getter` | `CompileTimeErrorCode.UNDEFINED_EXTENSION_GETTER` | `analyzer/lib/src/dart/resolver/property_element_resolver.dart:560` `PropertyElementResolver._resolveTargetExtensionElement`; `analyzer/lib/src/dart/resolver/property_element_resolver.dart:629` `PropertyElementResolver._resolveTargetExtensionOverride` | The getter '{0}' isn't defined for the extension '{1}'. | não verificado | **não implementado** |
| `undefined_extension_setter` | `CompileTimeErrorCode.UNDEFINED_EXTENSION_SETTER` | `analyzer/lib/src/dart/resolver/property_element_resolver.dart:580` `PropertyElementResolver._resolveTargetExtensionElement`; `analyzer/lib/src/dart/resolver/property_element_resolver.dart:647` `PropertyElementResolver._resolveTargetExtensionOverride` | The setter '{0}' isn't defined for the extension '{1}'. | não verificado | **não implementado** |

#### `analyzer/lib/src/dart/resolver/binary_expression_resolver.dart` (3)

| código | constante | emissor (arquivo:linha, método) | condição (mensagem oficial) | posição | DartForge |
|---|---|---|---|---|---|
| `augmented_expression_is_setter` | `CompileTimeErrorCode.AUGMENTED_EXPRESSION_IS_SETTER` | `analyzer/lib/src/dart/resolver/binary_expression_resolver.dart:384` `BinaryExpressionResolver._resolveUserDefinableAugmented`; `analyzer/lib/src/dart/resolver/prefix_expression_resolver.dart:288` `PrefixExpressionResolver._resolveAugmented`; `analyzer/lib/src/generated/resolver.dart:2000` `ResolverVisitor.visitAugmentedInvocation` | The augmented declaration is a setter, it can't be used to read a value. | não verificado | **não implementado** |
| `augmented_expression_not_operator` | `CompileTimeErrorCode.AUGMENTED_EXPRESSION_NOT_OPERATOR` | `analyzer/lib/src/dart/resolver/binary_expression_resolver.dart:370` `BinaryExpressionResolver._resolveUserDefinableAugmented`; `analyzer/lib/src/dart/resolver/prefix_expression_resolver.dart:272` `PrefixExpressionResolver._resolveAugmented` | The enclosing augmentation doesn't augment the operator '{0}'. | não verificado | **não implementado** |
| `non_bool_operand` | `CompileTimeErrorCode.NON_BOOL_OPERAND` | `analyzer/lib/src/dart/resolver/binary_expression_resolver.dart:86` `BinaryExpressionResolver._checkNonBoolOperand` | The operands of the operator '{0}' must be assignable to 'bool'. | não verificado | **publicado** — `crates/types/src/inferencia/expr.rs:3422` |

#### `analyzer/lib/src/dart/element/generic_inferrer.dart` (3)

| código | constante | emissor (arquivo:linha, método) | condição (mensagem oficial) | posição | DartForge |
|---|---|---|---|---|---|
| `inference_failure_on_function_invocation` | `WarningCode.INFERENCE_FAILURE_ON_FUNCTION_INVOCATION` | `analyzer/lib/src/dart/element/generic_inferrer.dart:855` `GenericInferrer._reportInferenceFailure` | The type argument(s) of the function '{0}' can't be inferred. | não verificado | **não implementado** |
| `inference_failure_on_generic_invocation` | `WarningCode.INFERENCE_FAILURE_ON_GENERIC_INVOCATION` | `analyzer/lib/src/dart/element/generic_inferrer.dart:867` `GenericInferrer._reportInferenceFailure` | The type argument(s) of the generic function type '{0}' can't be inferred. | não verificado | **não implementado** |
| `inference_failure_on_instance_creation` | `WarningCode.INFERENCE_FAILURE_ON_INSTANCE_CREATION` | `analyzer/lib/src/dart/element/generic_inferrer.dart:818` `GenericInferrer._reportInferenceFailure`; `analyzer/lib/src/dart/element/generic_inferrer.dart:831` `GenericInferrer._reportInferenceFailure` | The type argument(s) of the constructor '{0}' can't be inferred. | não verificado | **não implementado** |

#### `analyzer/lib/src/dart/resolver/method_invocation_resolver.dart` (2)

| código | constante | emissor (arquivo:linha, método) | condição (mensagem oficial) | posição | DartForge |
|---|---|---|---|---|---|
| `abstract_super_member_reference` | `CompileTimeErrorCode.ABSTRACT_SUPER_MEMBER_REFERENCE` | `analyzer/lib/src/dart/resolver/method_invocation_resolver.dart:774` `MethodInvocationResolver._resolveReceiverSuper`; `analyzer/lib/src/dart/resolver/property_element_resolver.dart:821` `PropertyElementResolver._resolveTargetSuperExpression`; `analyzer/lib/src/dart/resolver/property_element_resolver.dart:867` `PropertyElementResolver._resolveTargetSuperExpression` | The {0} '{1}' is always abstract in the supertype. | não verificado | **publicado** — `crates/types/src/inferencia/expr.rs:2066` |
| `undefined_extension_method` | `CompileTimeErrorCode.UNDEFINED_EXTENSION_METHOD` | `analyzer/lib/src/dart/resolver/method_invocation_resolver.dart:390` `MethodInvocationResolver._resolveExtensionMember`; `analyzer/lib/src/dart/resolver/method_invocation_resolver.dart:418` `MethodInvocationResolver._resolveExtensionOverride` | The method '{0}' isn't defined for the extension '{1}'. | não verificado | **não implementado** |

#### `analyzer/lib/src/diagnostic/diagnostic_factory.dart` (2)

| código | constante | emissor (arquivo:linha, método) | condição (mensagem oficial) | posição | DartForge |
|---|---|---|---|---|---|
| `equal_elements_in_const_set` | `CompileTimeErrorCode.EQUAL_ELEMENTS_IN_CONST_SET` | `analyzer/lib/src/diagnostic/diagnostic_factory.dart:211` `DiagnosticFactory.equalElementsInConstSet` | Two elements in a constant set literal can't be equal. | não verificado | **publicado** — `crates/types/src/constantes/verificador.rs:653` |
| `equal_keys_in_const_map` | `CompileTimeErrorCode.EQUAL_KEYS_IN_CONST_MAP` | `analyzer/lib/src/diagnostic/diagnostic_factory.dart:232` `DiagnosticFactory.equalKeysInConstMap` | Two keys in a constant map literal can't be equal. | não verificado | **publicado** — `crates/types/src/constantes/verificador.rs:653` |

#### `analyzer/lib/src/dart/resolver/function_expression_invocation_resolver.dart` (2)

| código | constante | emissor (arquivo:linha, método) | condição (mensagem oficial) | posição | DartForge |
|---|---|---|---|---|---|
| `extension_override_access_to_static_member` | `CompileTimeErrorCode.EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER` | `analyzer/lib/src/dart/resolver/function_expression_invocation_resolver.dart:176` `FunctionExpressionInvocationResolver._resolveReceiverExtensionOverride`; `analyzer/lib/src/dart/resolver/function_reference_resolver.dart:365` `FunctionReferenceResolver._resolveExtensionOverride`; `analyzer/lib/src/dart/resolver/method_invocation_resolver.dart:427` `MethodInvocationResolver._resolveExtensionOverride` | An extension override can't be used to access a static member from an extension. | não verificado | **não implementado** |
| `invocation_of_extension_without_call` | `CompileTimeErrorCode.INVOCATION_OF_EXTENSION_WITHOUT_CALL` | `analyzer/lib/src/dart/resolver/function_expression_invocation_resolver.dart:166` `FunctionExpressionInvocationResolver._resolveReceiverExtensionOverride` | The extension '{0}' doesn't define a 'call' method so the override can't be used in an invocation. | não verificado | **não implementado** |

#### `analyzer/lib/src/dart/resolver/function_reference_resolver.dart` (1)

| código | constante | emissor (arquivo:linha, método) | condição (mensagem oficial) | posição | DartForge |
|---|---|---|---|---|---|
| `generic_method_type_instantiation_on_dynamic` | `CompileTimeErrorCode.GENERIC_METHOD_TYPE_INSTANTIATION_ON_DYNAMIC` | `analyzer/lib/src/dart/resolver/function_reference_resolver.dart:107` `FunctionReferenceResolver._checkDynamicTypeInstantiation`; `analyzer/lib/src/dart/resolver/function_reference_resolver.dart:540` `FunctionReferenceResolver._resolvePropertyAccessFunction` | A method tear-off on a receiver whose type is 'dynamic' can't have type arguments. | não verificado | **não implementado** |

#### `analyzer/lib/src/dart/resolver/shared_type_analyzer.dart` (1)

| código | constante | emissor (arquivo:linha, método) | condição (mensagem oficial) | posição | DartForge |
|---|---|---|---|---|---|
| `inconsistent_pattern_variable_logical_or` | `CompileTimeErrorCode.INCONSISTENT_PATTERN_VARIABLE_LOGICAL_OR` | `analyzer/lib/src/dart/resolver/shared_type_analyzer.dart:115` `SharedTypeAnalyzerErrors.inconsistentJoinedPatternVariable` | The variable '{0}' has a different type and/or finality in this branch of the logical-or pattern. | não verificado | **não implementado** |

#### `analyzer/lib/src/dart/resolver/resolution_visitor.dart` (1)

| código | constante | emissor (arquivo:linha, método) | condição (mensagem oficial) | posição | DartForge |
|---|---|---|---|---|---|
| `mixin_with_non_class_superclass` | `CompileTimeErrorCode.MIXIN_WITH_NON_CLASS_SUPERCLASS` | `analyzer/lib/src/dart/resolver/resolution_visitor.dart:1693` `ResolutionVisitor._resolveType`; `analyzer/lib/src/dart/resolver/resolution_visitor.dart:1699` `ResolutionVisitor._resolveType` | Mixin can only be applied to class. | não verificado | **publicado** — `crates/analise/src/clausulas.rs:1229` |

#### `analyzer/lib/src/dart/resolver/typed_literal_resolver.dart` (1)

| código | constante | emissor (arquivo:linha, método) | condição (mensagem oficial) | posição | DartForge |
|---|---|---|---|---|---|
| `inference_failure_on_collection_literal` | `WarningCode.INFERENCE_FAILURE_ON_COLLECTION_LITERAL` | `analyzer/lib/src/dart/resolver/typed_literal_resolver.dart:479` `TypedLiteralResolver._inferListTypeUpwards`; `analyzer/lib/src/dart/resolver/typed_literal_resolver.dart:720` `TypedLiteralResolver._resolveSetOrMapLiteral2` | The type argument(s) of '{0}' can't be inferred. | não verificado | **não implementado** |

#### `analyzer/lib/src/dart/resolver/variable_declaration_resolver.dart` (1)

| código | constante | emissor (arquivo:linha, método) | condição (mensagem oficial) | posição | DartForge |
|---|---|---|---|---|---|
| `inference_failure_on_uninitialized_variable` | `WarningCode.INFERENCE_FAILURE_ON_UNINITIALIZED_VARIABLE` | `analyzer/lib/src/dart/resolver/variable_declaration_resolver.dart:34` `VariableDeclarationResolver.resolve` | The type of {0} can't be inferred without either a type or initializer. | não verificado | **não implementado** |

#### `analyzer/lib/src/dart/resolver/constructor_reference_resolver.dart` (1)

| código | constante | emissor (arquivo:linha, método) | condição (mensagem oficial) | posição | DartForge |
|---|---|---|---|---|---|
| `sdk_version_constructor_tearoffs` | `WarningCode.SDK_VERSION_CONSTRUCTOR_TEAROFFS` | `analyzer/lib/src/dart/resolver/constructor_reference_resolver.dart:27` `ConstructorReferenceResolver.resolve`; `analyzer/lib/src/dart/resolver/resolution_visitor.dart:971` `ResolutionVisitor.visitInstanceCreationExpression` | Tearing off a constructor requires the 'constructor-tearoffs' language feature. | não verificado | **não implementado** |

#### `analyzer/lib/src/dart/resolver/postfix_expression_resolver.dart` (1)

| código | constante | emissor (arquivo:linha, método) | condição (mensagem oficial) | posição | DartForge |
|---|---|---|---|---|---|
| `missing_assignable_selector` | `ParserErrorCode.MISSING_ASSIGNABLE_SELECTOR` | `analyzer/lib/src/dart/resolver/postfix_expression_resolver.dart:196` `PostfixExpressionResolver._resolveNullCheck`; fasta `MissingAssignableSelector`: `analyzer/lib/src/fasta/ast_builder.dart:3752` `AstBuilder.handleAssignmentExpression`; fasta `MissingAssignableSelector`: `analyzer/lib/src/fasta/ast_builder.dart:5688` `AstBuilder.handleUnaryPrefixAssignmentExpression`; fasta `MissingAssignableSelector`: `analyzer/lib/src/fasta/ast_builder.dart:5843` `AstBuilder.reportErrorIfSuper` | Missing selector such as '.identifier' or '[0]'. | não verificado | **publicado** — `crates/frontend/src/parser/expressions.rs:861`, `crates/frontend/src/parser/expressions.rs:881` |

### II.10 — Arquivos não-Dart: pubspec.yaml, analysis_options.yaml e AndroidManifest.xml

Cobre as **53 constantes** de `dump/N.txt`: 24 `PubspecWarningCode`
(`analyzer/lib/src/pubspec/`), 22 de `analysis_options.yaml` (17
`AnalysisOptionsWarningCode`, 3 `AnalysisOptionsHintCode`, 2
`AnalysisOptionsErrorCode`; `analyzer/lib/src/task/options.dart` e
`analyzer/lib/src/lint/options_rule_validator.dart`) e 7 `ManifestWarningCode`
(`analyzer/lib/src/manifest/manifest_validator.dart`). Nenhuma passa pelo
`LibraryAnalyzer`: são validadores de arquivo inteiro que o **servidor de
análise** chama uma vez por arquivo ao montar os contextos
(`analysis_server/lib/src/context_manager.dart:598-610`), fora do ciclo dos
drivers. Achados do lote: (1) 2 constantes **não têm emissor no 3.6.2**
(`ANALYSIS_OPTION_DEPRECATED`, `ANALYSIS_OPTION_DEPRECATED_WITH_REPLACEMENT`) e
1 só é emitida pelo `dart fix` (`MISSING_DEPENDENCY`); (2) com o registro de
lints do 3.6.2, `deprecated_lint`, `deprecated_lint_with_replacement` e
`replaced_lint` são inalcançáveis (nenhuma regra com `DeprecatedState` nem com
`replacedBy`); (3) uma exceção em qualquer validador zera **todos** os
diagnósticos do arquivo (o `try/catch` de `context_manager.dart:372-392`,
`:400-415`, `:463-484`); (4) `# ignore:` só vale no pubspec; `analyzer: errors:`
vale nos três; (5) o DartForge não tem nada disso: os 53 estão **não
implementados** e nem constam da tabela `crates/diagnostics/src/codigos_g.rs`.

#### Mecanismo 1 — quando o servidor valida cada arquivo não-Dart

Ponto único: `ContextManagerImpl._createAnalysisContexts`
(`analysis_server/lib/src/context_manager.dart:533-711`), dentro de
`performContextRebuild` (`:544-631`), para cada contexto da
`AnalysisContextCollectionImpl`:

```text
para cada analysisContext em collection.contexts:                       (:577)
  driver = analysisContext.driver
  para cada file em analysisContext.contextRoot.analyzedFiles():        (:598)
    se basename(file) == 'analysis_options.yaml':                       (:599, file_paths.dart:60-62)
        package = contextRoot.workspace.findPackageFor(file)
        _analyzeAnalysisOptionsYaml(driver, package, file)              (:602)
    senão se basename(file) == 'AndroidManifest.xml':                   (:603, file_paths.dart:65-67)
        _analyzeAndroidManifestXml(driver, file)                        (:604)
    senão se extension(file) == '.dart':  driver.addFile(file)          (:605-606)
    senão se basename(file) == 'pubspec.yaml':                          (:607, file_paths.dart:122-124)
        _analyzePubspecYaml(driver, file)                               (:608)
  packageName = rootFolder.shortName                                    (:612)
  se <raiz>/lib/fix_data.yaml existe: _analyzeFixDataYaml(...)          (:613-618)
  se <raiz>/lib/fix_data/ existe: _analyzeFixDataFolder(...)            (:620-625; recursivo, só *.yaml, :421-432)
```

(`file_paths.dart` = `analyzer/lib/src/util/file_paths.dart`.)

- **Quais arquivos:** o teste é só pelo **nome base**, em qualquer diretório
  que `ContextRootImpl.analyzedFiles()` percorra
  (`analyzer/lib/src/dart/analysis/context_root.dart:63-76`, `:98-132`): todos
  os arquivos sob cada caminho incluído, recursivamente, menos os excluídos por
  `_isExcluded` (`:139-173`): qualquer componente do caminho (até a raiz do
  contexto) começando por `.` (`:142-147`; logo `.dart_tool/`, `.git/`…), os
  `excludedPaths` e os globs de `analyzer: exclude:` (`:166-170`). Portanto um
  `pubspec.yaml`/`analysis_options.yaml`/`AndroidManifest.xml` em subpasta
  (`example/`, `android/app/src/main/`) também é validado, e um arquivo
  coberto por `exclude:` não é.
- **Alvo é arquivo ou subpasta:** se o caminho incluído é um arquivo,
  `analyzedFiles()` devolve só ele (`context_root.dart:67-68`):
  `dart analyze lib/a.dart` não valida pubspec nem opções; `dart analyze lib/`
  não alcança o `pubspec.yaml`/`analysis_options.yaml` da raiz do pacote (ficam
  fora do caminho incluído). O `dart analyze` manda os alvos como raízes
  (`dartdev/lib/src/analysis_server.dart:178-201`, `analysis.setAnalysisRoots`
  com `included` = alvos, `excluded` = `[]`).
- **Condições por arquivo:**
  - `analysis_options.yaml`: sempre (`_analyzeAnalysisOptionsYaml`,
    `context_manager.dart:369-394`): lê o texto do disco e chama
    `analyzeAnalysisOptions(FileSource(file), content, driver.sourceFactory,
    <caminho da raiz do contexto>, sdkVersionConstraint)` (`:379-385`), onde
    `sdkVersionConstraint` é o `environment: sdk:` do pubspec do pacote que
    contém o arquivo, ou `null` se o pacote não é `PubPackage` ou a restrição
    não parseia (`:377-378`, `analyzer/lib/src/workspace/pub.dart:448-462`).
  - `pubspec.yaml`: sempre (`_analyzePubspecYaml`, `:461-486`):
    `loadYamlNode(content, sourceUrl: file.toUri())`; se o documento **não é
    mapa** (vazio, escalar, lista), é trocado por `YamlMap()` vazio (`:467-469`)
    — daí sai só `missing_name` em 0/0; se o YAML **não parseia**, a exceção é
    engolida e o arquivo fica **sem diagnóstico algum** (não existe código de
    erro de parse para pubspec). Passa o `analysisOptions` do arquivo
    (`driver.getAnalysisOptionsForFile`, `:470`), o que liga também os lints de
    pubspec (`getPubspecVisitor`, `analyzer/lib/src/pubspec/pubspec_validator.dart:59-74`;
    são `LintCode`, fora deste lote).
  - `AndroidManifest.xml`: o método é chamado sempre, mas
    `ManifestValidator.validate(content, analysisOptions.chromeOsManifestChecks)`
    devolve `[]` se a opção é falsa (`analyzer/lib/src/manifest/manifest_validator.dart:405-408`).
    A opção só liga com `analyzer: optional-checks: chrome-os-manifest-checks`
    (escalar) ou `optional-checks: {chrome-os-manifest-checks: true}`
    (`analyzer/lib/src/analysis_options/apply_options.dart:83-99`, `:206-207`)
    no `analysis_options.yaml` que rege o arquivo. Sem ela, zero diagnósticos.
  - `fix_data.yaml`: só `<raiz do contexto>/lib/fix_data.yaml` e
    `<raiz>/lib/fix_data/**.yaml`, pelo `TransformSetParser`
    (`context_manager.dart:436-457`); os códigos são `TransformSetErrorCode`
    (do `analysis_server`, fora deste lote).
- **Mudanças em disco** (`_handleWatchEventImpl`, `:786-846`): alteração de
  `analysis_options.yaml`, `pubspec.yaml`, `BUILD` ou
  `.dart_tool/package_config.json` **recria todos os contextos** (`:797-812`)
  e, com isso, revalida tudo; `AndroidManifest.xml` e `fix_data*.yaml` são
  revalidados sozinhos em ADD/MODIFY (`:838-842`, `:488-525`). Irrelevante para
  o `dart analyze` (uma passada), relevante para o LSP.
- **Falha = silêncio:** os quatro métodos envolvem tudo em
  `try { … } catch (exception) { }` e publicam `convertedErrors`, que nasce
  `const []` (`:371`, `:399`, `:438`, `:462`). Qualquer exceção ou `TypeError`
  dentro do validador (há vários `valueOrThrow` = `value as Object`,
  `analyzer/lib/src/util/yaml.dart:147`, e um `as bool` em
  `analyzer/lib/src/lint/options_rule_validator.dart:151`) faz o arquivo sair
  com **lista vazia**, inclusive os diagnósticos já acumulados antes da exceção.
- **Como vira `analysis.errors`:**

```text
errors (List<AnalysisError> do analyzer)
  -> AnalyzerConverter().convertAnalysisErrors(errors,
         lineInfo: LineInfo.fromContent(content), options: analysisOptions)   (context_manager.dart:386-388, :409-411, :477-480)
       para cada erro:                                                         (analyzer_plugin/lib/utilities/analyzer_converter.dart:66-86)
         p = ErrorProcessor.getProcessor(options, erro)                        (analyzer/lib/source/error_processor.dart:91-108)
         se p != null e p.severity == null: descarta        (errors: <código>: ignore)
         se p != null: severidade = p.severity              (errors: <código>: info/warning/error)
         senão: severidade = errorCode.errorSeverity
         -> protocol.AnalysisError(severity, type,
              Location(source.fullName, offset, length, startLine, startColumn, endLine, endColumn),
              message, code = errorCode.name.toLowerCase(), correction, hasFix: true)   (analyzer_converter.dart:24-59)
  -> callbacks.recordAnalysisErrors(path, convertedErrors)                     (context_manager.dart:393, :416, :485)
  -> notificationManager.recordAnalysisErrors(serverId, path, errors)          (analysis_server/lib/src/analysis_server.dart:1153-1157)
       só se o caminho está dentro de uma raiz incluída e fora das excluídas   (analysis_server/lib/src/plugin/notification_manager.dart:120-128, :283-306)
  -> channel.sendNotification(AnalysisErrorsParams(filePath, mergedErrors))    (notification_manager.dart:321-324)
```

  `analyzer_plugin` não está na extração `E:\references\dart-sdk-3.6.2\pkg`;
  as linhas de `analyzer_converter.dart` foram lidas da tag 3.6.2 com
  `git show 3.6.2:pkg/analyzer_plugin/lib/utilities/analyzer_converter.dart`.
  O nome impresso é `errorCode.name` em minúsculas — **não** o `uniqueName`:
  `ANALYSIS_OPTION_DEPRECATED_WITH_REPLACEMENT` imprimiria
  `analysis_option_deprecated`
  (`analyzer/lib/src/analysis_options/error/option_codes.g.dart:140-145`).
  Linha/coluna vêm do `LineInfo` do texto lido do disco (base 1).
- **O que o `dart analyze` faz com eles**
  (`dartdev/lib/src/commands/analyze.dart:185-199`): recebe cada
  `analysis.errors`; descarta `type == 'TODO' && severity == 'INFO'`; se o
  nome base do arquivo é `analysis_options.yaml` ou `pubspec.yaml` **e** a
  severidade é `ERROR`, o diagnóstico vai para `priorityErrors`; o resto vai
  para `errors`, junto com os dos `.dart`. Saída (`:241-281`):
  - formato padrão: se há `priorityErrors`, imprime linha vazia, o parágrafo
    `Errors were found in 'pubspec.yaml' and/or 'analysis_options.yaml' which
    might result in either invalid diagnostics being produced or valid
    diagnostics being missed.`, a lista deles, e `Errors in remaining files.`
    se houver outros (`:263-273`); depois a lista normal; o total soma os dois
    (`:279-280`);
  - `--format=machine` e `--format=json`: emitem **só** `errors`
    (`:244-247`) — os `priorityErrors` não aparecem na saída, mas contam para
    o código de saída (`:287-297`).

  Na prática `priorityErrors` recebe `parse_error` e
  `included_file_parse_error` (os únicos de severidade `ERROR` do lote) e
  qualquer código promovido a `error` em `analyzer: errors:`. Ordenação:
  severidade, caminho, offset, mensagem
  (`dartdev/lib/src/analysis_server.dart:385-402`); os avisos de
  pubspec/opções (WARNING) se intercalam com os dos `.dart` pela ordem de
  caminho.
- **`analyzer_cli` (`dartanalyzer`, não o `dart analyze`):** mesmo trio, pelo
  nome base, mas só para os arquivos passados na linha de comando
  (`analyzer_cli/lib/src/driver.dart:246-350`); o pubspec só é validado se o
  documento é `YamlMap` (`:296-303`).

#### Mecanismo 2 — posição (offset/length) e supressões

- **YAML:** todo relato usa o `SourceSpan` do nó: `ErrorReporter.atSourceSpan`
  → `atOffset(offset: span.start.offset, length: span.length)`
  (`analyzer/lib/error/listener.dart:189-204`, `:159-185`); no pubspec,
  `PubspecValidationContext.reportErrorForNode` faz o mesmo
  (`analyzer/lib/src/pubspec/pubspec_validator.dart:160-176`). Offsets em
  unidades UTF-16 do texto do arquivo. O span de cada nó vem do `package:yaml`
  (fixado em `yaml_rev e773005a…` no `DEPS` da tag 3.6.2; lido aqui na cópia
  `yaml-3.1.3` do pub cache — equivalência das revisões **não verificada**):
  - escalar: o span do token inteiro, **com as aspas** quando há
    (`YamlScalar.internal(value, scalar)` → `scalar.span`,
    `yaml/lib/src/yaml_node.dart:173-175`); `span.text` de `"a"` traz as aspas,
    `value` é `a`;
  - valor vazio (`flutter:` sem nada, item `-` vazio): escalar de valor `null`
    e span **de comprimento 0** posto no **início do token indicador** — o
    `:` da chave, em mapa de bloco (`yaml/lib/src/parser.dart:476-482`), o `-`
    do item, em lista de bloco (`:367-369`) — e não depois dele
    (`_processEmptyScalar(location)` → `location.pointSpan()`, `:664-665`):
    em `name:\n` o valor nulo está em 4/0;
  - mapa/lista: do início do primeiro evento até o span do evento de fim
    (`firstEvent.span.expand(event.span)`, `yaml/lib/src/loader.dart:144`,
    `:172`); em coleções de bloco o evento de fim é o token `BLOCK-END`, cuja
    posição exata (pela leitura, o início do próximo token depois do recuo,
    pulando linhas em branco e comentários) **não foi verificada** — importa
    para `invalid_section_format`, `asset_missing_path`,
    `dependencies_field_not_map`, `invalid_platforms_field`,
    `flutter_field_not_map`, `asset_field_not_list`,
    `workspace_field_not_list`, `platform_value_disallowed` quando o nó
    relatado é um mapa/lista de bloco;
  - chave de mapa: é um `YamlNode` próprio (`map.nodes.keys`), span só da
    chave, sem o `:`.
- **XML (manifest):** parser próprio; atributo =
  `sourceFile.span(attributeNamePos, _pos)` com `_pos` na aspa de fechamento
  (`analyzer/lib/src/manifest/manifest_validator.dart:230-234`): cobre
  `android:name="valor` — **sem a aspa final**; elemento =
  `sourceFile.span(startPos, _pos + 1)` (`:384-387`; onde `_pos` está nesse
  ponto para elementos com filhos **não verificado**). Nomes de elemento e de
  atributo são postos em minúsculas pelo parser (`:204`, `:316-325`), por isso
  as chaves `android:screenorientation` e `android:resizeableactivity`
  (`analyzer/lib/src/manifest/manifest_values.dart:20`, `:25`).
- **`# ignore:` / `# ignore_for_file:`** — só no **pubspec**: `validatePubspec`
  termina com `IgnoreInfo.forYaml(conteúdo, lineInfo)` e filtra
  `!ignoreInfo.ignored(error)` (`pubspec_validator.dart:75-78`). Regras
  (`analyzer/lib/src/ignore_comments/ignore_info.dart:91-98`, `:141-161`,
  `:181-199`): `#+[ ]*ignore:<nomes>` vale para a **própria linha** se há texto
  não branco antes do `#` na linha, senão para a **linha seguinte**;
  `#[ ]*ignore_for_file:<nomes>` vale para o arquivo; nomes separados por
  vírgula, comparados em minúsculas com `errorCode.name` ou com o `uniqueName`
  sem o prefixo da classe (`:25-45`); a linha do erro é a do `offset`
  (`missing_name`, em 0/0, é a linha 1). Diferente do Dart, aqui **não há**
  checagem de `isIgnorable`/`cannot-ignore`.
  `analyzeAnalysisOptions` e `ManifestValidator.validate` **não** consultam
  `IgnoreInfo` (`analyzer/lib/src/task/options.dart:25-193`,
  `manifest_validator.dart:405-417`): comentário de ignore não cala nada em
  `analysis_options.yaml` nem no manifest.
- **`analyzer: errors:`** — vale nos **três** tipos de arquivo, pelo
  `ErrorProcessor` aplicado na conversão (pseudocódigo acima). O
  `analysisOptions` é o que rege o próprio arquivo
  (`driver.getAnalysisOptionsForFile(file)`, `context_manager.dart:374`,
  `:406`, `:470`); `appliesTo` compara o código da regra (posto em maiúsculas,
  `error_processor.dart:34`) com `errorCode.name` (`:85-87`). Ex.:
  `errors: {asset_does_not_exist: ignore}` remove; `undefined_lint: error`
  promove (e o `dart analyze` passa a tratá-lo como prioritário).
- Sem portas de "arquivo gerado", versão de linguagem ou erro de sintaxe: os
  validadores não dependem dos `.dart`. T1–T8 não se aplicam a este lote.

#### Mecanismo 3 — `include:` e a lista de lints

`analyzeAnalysisOptions` (`analyzer/lib/src/task/options.dart:25-193`):

```text
errors = []; initialSource = source; initialIncludeSpan = null
includeChain = {}   (Source -> span do include que o trouxe); firstPluginName = null

validate(source, options):                                                    (:75-175)
  direto = (initialIncludeSpan == null)         # true só para o arquivo inicial   (:76)
  errosDoArquivo = OptionsFileValidator(source, sdkVersionConstraint,
                     sourceIsOptionsForContextRoot: direto).validate(options)  (:77-82)
  acrescenta(errosDoArquivo, source, direto)                                  (:83-84)
  includeNode = options.valueAt('include')                                    (:86)
  se includeNode == null:
      acrescenta(PluginsOptionValidator(null).validate(options), source, direto)   (:87-94)
      retorna
  includeSpan = includeNode.span
  initialIncludeSpan ??= includeSpan            # o include do arquivo INICIAL  (:96)
  includeUri = includeSpan.text                 # texto cru do span (com aspas, se houver)  (:97)
  includedSource = sourceFactory.resolveUri(source, includeUri)               (:98)
  se includedSource == initialSource:
      RECURSIVE_INCLUDE_FILE em initialIncludeSpan, [includeUri, source.fullName]; retorna      (:99-110)
  se includedSource == null ou !includedSource.exists():
      INCLUDE_FILE_NOT_FOUND em initialIncludeSpan, [includeUri, source.fullName, contextRoot]; retorna   (:111-122)
  se includeChain[includedSource] existe (spanInChain):
      INCLUDED_FILE_WARNING em initialIncludeSpan,
         [includedSource, spanInChain.start.offset, spanInChain.length,
          'The file includes itself recursively.']; retorna                   (:123-140)
  includeChain[includedSource] = includeSpan                                  (:141)
  tenta:
      includedOptions = getOptionsFromString(includedSource.contents.data)    (:144-145)
      validate(includedSource, includedOptions)           # recursão          (:146)
      firstPluginName ??= _firstPluginName(includedOptions)                   (:147)
      acrescenta(PluginsOptionValidator(firstPluginName).validate(options), source, direto)   (:150-155)
  captura OptionsFormatException e:
      INCLUDED_FILE_PARSE_ERROR em initialIncludeSpan,
         [includedSource.fullName, e.span.start.offset, e.span.end.offset, e.message]   (:156-174)

acrescenta(lista, source, direto):                                            (:44-72)
  se direto: errors += lista
  senão: para cada erro e da lista:
      INCLUDED_FILE_WARNING em initialIncludeSpan,
         [source.fullName, e.offset, e.offset + e.length - 1, e.message]

corpo:                                                                        (:177-192)
  tenta: options = getOptionsFromString(content); validate(source, options)
  captura OptionsFormatException e: PARSE_ERROR em e.span, [e.message]
```

- `getOptionsFromString`
  (`analyzer/lib/src/analysis_options/analysis_options_provider.dart:91-103`):
  `loadYamlNode`; documento que não é mapa vira `YamlMap()` vazio (sem
  diagnóstico); `YamlException` vira `OptionsFormatException(message, span)`;
  qualquer outra exceção vira `OptionsFormatException('Unable to parse YAML
  document.')` **sem span** — e aí `e.span!` (`options.dart:181`, `:159`)
  estoura e cai no silêncio do `context_manager`.
- **Resolução do URI** (`SourceFactoryImpl.resolveUri`,
  `analyzer/lib/src/context/source.dart:116-141`, `_internalResolveUri`
  `:154-176`): string vazia → o próprio arquivo (→ `recursive_include_file`);
  `FormatException` do parse → `null`; URI relativo → resolvido contra o
  `file:` do arquivo que contém o `include`; URI absoluto → primeiro
  `UriResolver` do `sourceFactory` do driver que o resolver (`package:` pelo
  `package_config.json` do contexto, `file:`, `dart:`). `null` ou arquivo
  inexistente → `include_file_not_found`. Como `includeUri` é
  `includeSpan.text`, um `include: "package:lints/recommended.yaml"` **entre
  aspas** é resolvido com as aspas no texto: pela leitura cai em
  `include_file_not_found` com `{0}` contendo as aspas (comportamento
  **não verificado** em execução — conferir com o `dart` 3.6.2 antes de
  implementar). Um `include:` que é lista ou mapa usa o texto do span inteiro
  como URI (o 3.6.2 não trata lista de includes).
- **Recursão:** A inclui A → `recursive_include_file` (a comparação é só com
  o arquivo **inicial**); A → B → C → B → `included_file_warning` com a
  mensagem fixa `The file includes itself recursively.` (o `{0}` aí é o
  **objeto `Source`**, formatado pelo `toString()` dele — o caminho, para
  `FileSource`, `analyzer/lib/source/file_source.dart:95-97`; `{1}`/`{2}` são
  offset e **length** do include que trouxe B, não início/fim). Em todos os casos a posição é o valor do
  `include:` do arquivo inicial.
- **Erros do incluído:** cada diagnóstico de um arquivo incluído vira um
  `included_file_warning` separado, todos na mesma posição; severidade WARNING
  mesmo que o original fosse hint. `deprecated_lint*`, `removed_lint` e
  `replaced_lint` nem são gerados em incluídos
  (`sourceIsOptionsForContextRoot == false`,
  `analyzer/lib/src/lint/options_rule_validator.dart:108`).
- **Lista de lints** — `LinterRuleOptionsValidator._validateRules`
  (`options_rule_validator.dart:64-154`), sobre `linter: rules:`:

```text
seenRules = {}
validateRule(node, enabled):                                                  (:76-143)
  value = node.value; se null: retorna
  rule = primeira regra registrada com rule.name == value                     (:42-43, :80)
  se rule == null: UNDEFINED_LINT em node.span, [value]; retorna              (:81-88)
  se enabled:
     inc = primeiro nome de rule.incompatibleRules que já está em seenRules   (:67-74, :91)
     se inc != null: INCOMPATIBLE_LINT em node.span, [value, inc]             (:92-97)
     senão se !seenRules.add(rule.name): DUPLICATE_RULE em node.span, [value] (:98-104)
  se sourceIsOptionsForContextRoot:                                           (:108)
     state = rule.state
     se state é DeprecatedState e currentSdkAllows(state.since):
        replacedBy != null -> DEPRECATED_LINT_WITH_REPLACEMENT [value, replacedBy]   (:110-117)
        senão              -> DEPRECATED_LINT [value]                         (:118-124)
     senão se state é RemovedState e currentSdkAllows(state.since):
        since = state.since.toString()
        replacedBy != null -> REPLACED_LINT [value, since, replacedBy]        (:125-133)
        senão              -> REMOVED_LINT [value, since]                     (:134-140)

rules é YamlList: para cada nó: validateRule(nó, true)                        (:145-148)
rules é YamlMap:  para cada (chave, valor): validateRule(chave, valor.value as bool)   (:149-153)

currentSdkAllows(since):                                                      (:35-40)
  since == null -> true
  sdkVersionConstraint == null -> false
  senão sdkVersionConstraint.allows(since)
```

  - O registro é `Registry.ruleRegistry` (`:33`), preenchido pelo servidor com
    `linter.registerLintRules()` (`analysis_server/lib/src/server/driver.dart:378`,
    `:486`): 240 chamadas `..register(` em `linter/lib/src/rules.dart`
    (contagem por grep), **incluindo** as regras removidas (que por isso não
    dão `undefined_lint`).
  - `incompatibleRules` no 3.6.2 (`linter/lib/src/rules/*.dart`):
    `always_specify_types` × {`avoid_types_on_closure_parameters`,
    `omit_local_variable_types`, `omit_obvious_local_variable_types`};
    `always_use_package_imports` × `prefer_relative_imports`;
    `avoid_final_parameters` × `prefer_final_parameters`;
    `omit_local_variable_types` × `specify_nonobvious_local_variable_types`;
    `prefer_double_quotes` × `prefer_single_quotes`;
    `prefer_final_locals` × `unnecessary_final`;
    `prefer_final_parameters` × {`unnecessary_final`, `avoid_final_parameters`}
    (e os simétricos, declarados em cada arquivo). O relato sai na **segunda**
    regra do par na ordem do arquivo, e essa regra **não** entra em
    `seenRules` (o `add` fica no `else`).
  - Regras com `State.removed` no 3.6.2:
    `always_require_non_null_named_parameters` (3.3.0), `avoid_as` (2.12.0),
    `avoid_returning_null` (3.3.0), `avoid_returning_null_for_future` (3.3.0),
    `avoid_unstable_final_fields` (**sem `since`**), `enable_null_safety`
    (3.0.0), `invariant_booleans` (3.0.0), `iterable_contains_unrelated_type`
    (3.3.0), `list_remove_unrelated_type` (3.3.0), `prefer_bool_in_asserts`
    (3.0.0), `prefer_equal_for_default_values` (3.0.0), `super_goes_last`
    (3.0.0) (`grep "State.removed" linter/lib/src/rules`; versões em
    `analyzer/lib/src/lint/state.dart:8-14`). Nenhuma tem `replacedBy` e
    nenhuma regra usa `State.deprecated`/`DeprecatedState` (greps vazios em
    `linter/lib`): `replaced_lint`, `deprecated_lint` e
    `deprecated_lint_with_replacement` só saem com um `LintRuleProvider`
    injetado (testes).
  - `removed_lint` depende do `environment: sdk:`: só sai se a restrição
    **contém** a versão da remoção. Com `sdk: ^3.6.0` (≥3.6.0 <4.0.0) nenhuma
    das datadas é relatada; com `sdk: '>=2.19.0 <4.0.0'` saem as de 3.0.0 e
    3.3.0; sem pubspec ou sem restrição, nenhuma datada. A exceção é
    `avoid_unstable_final_fields` (`since == null` → sempre), com `{1}` =
    `null` (`state.since.toString()`, `:126`).
  - Forma de mapa: `valor.value as bool` (`:151`) lança `TypeError` para valor
    não booleano (`rule: ignore`, valor vazio) → arquivo sem diagnósticos
    (Mecanismo 1). Regra com `false` é validada com `enabled = false`: só
    `undefined_lint` e os de estado.

#### PubspecValidator — `validatePubspec` e sub-validadores (`analyzer/lib/src/pubspec/pubspec_validator.dart`, `analyzer/lib/src/pubspec/validators/*.dart`)

`validatePubspec` (`pubspec_validator.dart:39-79`) roda, nesta ordem, as
funções da lista `_pubspecValidators` (`:25-33`): `dependencyValidator`,
`fieldValidator`, `flutterValidator`, `nameValidator`, `screenshotsValidator`,
`platformsValidator`, `workspaceValidator`; depois os visitantes de lint de
pubspec (`:59-74`) e, por fim, o filtro de `# ignore` (`:75-78`). Todos
relatam por `ctx.reportErrorForNode(nó, código, args)` = span do nó YAML
(`:160-176`), exceto `missing_name` (0/0). Os caminhos são resolvidos a partir
do diretório do pubspec (`context.dirname(ctx.source.fullName)`), com
`path.posix.split` + `context.joinAll` (separador da plataforma) e existência
consultada no `ResourceProvider` (disco, com overlays). Severidade de todos:
WARNING (`analyzer/lib/src/pubspec/pubspec_warning_code.g.dart:265`).
`MissingDependencyValidator` é uma classe à parte, **não** chamada por
`validatePubspec`.

```text
dependencyValidator(ctx):                                   (validators/dependency_validator.dart:13-113)
  deps    = getDeclaredDependencies('dependencies')         (:83-84)
  devDeps = getDeclaredDependencies('dev_dependencies')     (:85-86)
     # campo ausente ou escalar nulo -> {}; mapa -> nós; outro -> DEPENDENCIES_FIELD_NOT_MAP [chave]   (:17-29)
  publicável = contents['version'] != null e asString(contents['publish_to']) != 'none'   (:88-97)
  para cada dep de deps:     validatePathEntries(valor, publicável)        (:99-101)
  para cada dep de devDeps:                                                (:103-112)
     se deps.containsKey(chave): UNNECESSARY_DEV_DEPENDENCY na chave, [nome]
     validatePathEntries(valor, false)

validatePathEntries(dep, checar):                           (:40-81)
  se dep não é YamlMap: retorna
  p = asString(dep['path'])
  se p != null:
     se p contém '\': PATH_NOT_POSIX no valor, [p]; retorna            (:49-53)   # nem olha 'git'
     pasta = normalize(absolute(join(dirname(pubspec), joinAll(posix.split(p)))))   (:54-60)
     se !pasta.exists: PATH_DOES_NOT_EXIST no valor, [p]               (:61-63)
     senão se !pasta/pubspec.yaml existe: PATH_PUBSPEC_DOES_NOT_EXIST no valor, [p]   (:64-69)
     se checar: INVALID_DEPENDENCY na CHAVE 'path', ['path']           (:70-73)
  se dep['git'] != null e checar: INVALID_DEPENDENCY na CHAVE 'git', ['git']   (:76-80)
```

| código | constante | emissor (arquivo:linha, método) | condição | posição | DartForge |
|---|---|---|---|---|---|
| `asset_directory_does_not_exist` | `PubspecWarningCode.ASSET_DIRECTORY_DOES_NOT_EXIST` | `analyzer/lib/src/pubspec/validators/flutter_validator.dart:127` `_validateAssetPath` (de `flutterValidator` :52, :69) | entrada de `flutter: assets:` (string, ou `path:` de um mapa) que não começa por `packages/`, **termina em `/`**, e `_assetExistsAtPath` é falso | o escalar da entrada (ou do `path:`), `reportErrorForNode` | **não implementado** — ver especificação completa |
| `asset_does_not_exist` | `PubspecWarningCode.ASSET_DOES_NOT_EXIST` | `analyzer/lib/src/pubspec/validators/flutter_validator.dart:128` `_validateAssetPath` | idem, entrada que **não** termina em `/` e não existe como pasta, nem como arquivo, nem como `<subpasta do pai>/<nome>` (variantes) | o escalar da entrada (ou do `path:`) | **não implementado** — ver especificação completa |
| `asset_field_not_list` | `PubspecWarningCode.ASSET_FIELD_NOT_LIST` | `analyzer/lib/src/pubspec/validators/flutter_validator.dart:36` `flutterValidator` | `flutter:` é mapa e `assets:` existe e não é `YamlList` (inclusive `assets:` vazio, que é escalar nulo) | o nó de `assets` (span 0 se vazio) | **não implementado** — sem leitor de pubspec com posições |
| `asset_missing_path` | `PubspecWarningCode.ASSET_MISSING_PATH` | `analyzer/lib/src/pubspec/validators/flutter_validator.dart:57` `flutterValidator` | item de `assets` é mapa sem a chave `path` | o mapa do item inteiro | **não implementado** — idem |
| `asset_not_string` | `PubspecWarningCode.ASSET_NOT_STRING` | `analyzer/lib/src/pubspec/validators/flutter_validator.dart:65` `flutterValidator` | item é mapa, `path:` é escalar com valor não-`String` (número, bool; valor nulo estoura em `valueOrThrow` :62); **interrompe** o validador (`return` :66) | o escalar de `path:` | **não implementado** — idem |
| `asset_not_string_or_map` | `PubspecWarningCode.ASSET_NOT_STRING_OR_MAP` | `analyzer/lib/src/pubspec/validators/flutter_validator.dart:47` e `:73` `flutterValidator` | item de `assets` é escalar não-`String` (:44-49, com `return`: para nos demais itens) ou não é escalar nem mapa, isto é, lista (:71-74, segue) | o nó do item | **não implementado** — idem |
| `asset_path_not_string` | `PubspecWarningCode.ASSET_PATH_NOT_STRING` | `analyzer/lib/src/pubspec/validators/flutter_validator.dart:60` `flutterValidator` | item é mapa e `path:` não é `YamlScalar` (mapa ou lista) | o nó de `path:` | **não implementado** — idem |
| `dependencies_field_not_map` | `PubspecWarningCode.DEPENDENCIES_FIELD_NOT_MAP` | `analyzer/lib/src/pubspec/validators/dependency_validator.dart:27` `dependencyValidator.getDeclaredDependencies`; também `analyzer/lib/src/pubspec/validators/missing_dependency_validator.dart:74` (só `dart fix`) | `dependencies` ou `dev_dependencies` existe, não é escalar nulo e não é mapa; `{0}` = nome do campo | o nó do valor do campo | **não implementado** — ver especificação completa |
| `deprecated_field` | `PubspecWarningCode.DEPRECATED_FIELD` | `analyzer/lib/src/pubspec/validators/field_validator.dart:26` `fieldValidator` | chave de **topo** igual a `author`, `authors`, `transformers` ou `web` (:9-14) | o nó da chave | **não implementado** — ver especificação completa |
| `flutter_field_not_map` | `PubspecWarningCode.FLUTTER_FIELD_NOT_MAP` | `analyzer/lib/src/pubspec/validators/flutter_validator.dart:25` `flutterValidator` | `flutter:` existe, não é mapa e o valor não é nulo (`flutter:` vazio é aceito, :21) | o nó do valor de `flutter` | **não implementado** — sem leitor de pubspec com posições |
| `invalid_dependency` | `PubspecWarningCode.INVALID_DEPENDENCY` | `analyzer/lib/src/pubspec/validators/dependency_validator.dart:71` e `:79` `dependencyValidator.validatePathEntries` | pacote publicável (tem `version` e `publish_to` diferente de `none`) com dependência de `dependencies` (não de `dev_dependencies`) que tem `path:` string ou `git:` não nulo; `{0}` = `path` ou `git` | a **chave** `path` / `git` dentro da dependência | **não implementado** — ver especificação completa |
| `invalid_platforms_field` | `PubspecWarningCode.INVALID_PLATFORMS_FIELD` | `analyzer/lib/src/pubspec/validators/platforms_validator.dart:32` `platformsValidator` | `platforms:` de topo existe e não é mapa (inclusive vazio) | o nó do valor | **não implementado** — sem leitor de pubspec com posições |
| `missing_dependency` | `PubspecWarningCode.MISSING_DEPENDENCY` | `analyzer/lib/src/pubspec/validators/missing_dependency_validator.dart:127` `MissingDependencyValidator.validate`; único chamador: `analysis_server/lib/src/services/correction/bulk_fix_processor.dart:951` (`dart fix`) | algum nome de `usedDeps` fora de `dependencies`, ou de `usedDevDeps` fora de ambos os campos (conjuntos montados pelo `bulk_fix_processor`); **nunca chega ao `dart analyze`** | o **primeiro valor** do mapa de topo (`contents.nodes.values.first`) | **não implementado** — ver especificação completa |
| `missing_name` | `PubspecWarningCode.MISSING_NAME` | `analyzer/lib/src/pubspec/validators/name_validator.dart:16` e `:25` `nameValidator` | documento não é mapa (no servidor vira mapa vazio) ou não tem a chave `name` | `atOffset(offset: 0, length: 0)` | **não implementado** — ver especificação completa |
| `name_not_string` | `PubspecWarningCode.NAME_NOT_STRING` | `analyzer/lib/src/pubspec/validators/name_validator.dart:28` `nameValidator` | `name` existe e não é escalar `String` (número, nulo/vazio, mapa, lista) | o nó do valor de `name` | **não implementado** — sem leitor de pubspec com posições |
| `path_does_not_exist` | `PubspecWarningCode.PATH_DOES_NOT_EXIST` | `analyzer/lib/src/pubspec/validators/dependency_validator.dart:63` `validatePathEntries`; `analyzer/lib/src/pubspec/validators/screenshot_validator.dart:34` `screenshotsValidator`; `analyzer/lib/src/pubspec/validators/workspace_validator.dart:62` `_validateDirectoryPath` | (a) `path:` de dependência (normal ou dev) aponta para pasta inexistente; (b) item de `screenshots:` (mapa) com `path:` string cujo **arquivo** não existe; (c) entrada de `workspace:` que é subpasta mas não existe | o escalar do caminho | **não implementado** — ver especificação completa |
| `path_not_posix` | `PubspecWarningCode.PATH_NOT_POSIX` | `analyzer/lib/src/pubspec/validators/dependency_validator.dart:51` `validatePathEntries` | `path:` de dependência contém `\`; retorna antes de qualquer outra checagem da dependência | o escalar do `path:` | **não implementado** — ver especificação completa |
| `path_pubspec_does_not_exist` | `PubspecWarningCode.PATH_PUBSPEC_DOES_NOT_EXIST` | `analyzer/lib/src/pubspec/validators/dependency_validator.dart:67` `validatePathEntries` | a pasta do `path:` existe mas não tem `pubspec.yaml` | o escalar do `path:` | **não implementado** — ver especificação completa |
| `platform_value_disallowed` | `PubspecWarningCode.PLATFORM_VALUE_DISALLOWED` | `analyzer/lib/src/pubspec/validators/platforms_validator.dart:57` `platformsValidator` | valor de uma chave de `platforms:` não é escalar nulo (qualquer valor presente) | o nó do valor | **não implementado** — sem leitor de pubspec com posições |
| `unknown_platform` | `PubspecWarningCode.UNKNOWN_PLATFORM` | `analyzer/lib/src/pubspec/validators/platforms_validator.dart:41` `platformsValidator` | chave de `platforms:` fora de `android`, `ios`, `linux`, `macos`, `web`, `windows` (:11-18) ou não escalar; `{0}` = a string, o número, ou `toString()` do nó | o nó da chave | **não implementado** — idem |
| `unnecessary_dev_dependency` | `PubspecWarningCode.UNNECESSARY_DEV_DEPENDENCY` | `analyzer/lib/src/pubspec/validators/dependency_validator.dart:108` `dependencyValidator` | chave de `dev_dependencies` que também é chave de `dependencies` | o nó da chave em `dev_dependencies` | **não implementado** — ver especificação completa |
| `workspace_field_not_list` | `PubspecWarningCode.WORKSPACE_FIELD_NOT_LIST` | `analyzer/lib/src/pubspec/validators/workspace_validator.dart:20` `workspaceValidator` | `workspace:` existe e não é lista | o nó do valor | **não implementado** — sem leitor de pubspec com posições |
| `workspace_value_not_string` | `PubspecWarningCode.WORKSPACE_VALUE_NOT_STRING` | `analyzer/lib/src/pubspec/validators/workspace_validator.dart:31` e `:38` `workspaceValidator` | item de `workspace:` é escalar não-`String` (:28-33, com `return`) ou não é escalar (:36-39) | o nó do item | **não implementado** — idem |
| `workspace_value_not_subdirectory` | `PubspecWarningCode.WORKSPACE_VALUE_NOT_SUBDIRECTORY` | `analyzer/lib/src/pubspec/validators/workspace_validator.dart:56` `_validateDirectoryPath` | `join(raiz, entrada)` não está contido na pasta do pubspec (`Folder.contains`); `{0}` = caminho absoluto da pasta do pubspec | o escalar do item | **não implementado** — idem |

Notas: `_assetExistsAtPath` (`flutter_validator.dart:85-111`) aceita o caminho
como pasta **ou** arquivo, e ainda `<pai>/<qualquer subpasta>/<nome>` (variantes
de resolução `2.0x/`, `3.0x/`); por isso uma entrada sem `/` final que é pasta
não é relatada. Entradas `packages/…` nunca são validadas (:117-118).

**Estado em 2026-10-05 (escrito, não compilado).** O `PubspecValidator` está em
`crates/analise/src/naodart/pubspec.rs`, com os sub-validadores da 6.11.0 abertos: os sete de
`validatePubspec`, na ordem do original, com todos os códigos da tabela menos `missing_dependency`
(que nunca chega ao `dart analyze`). O YAML com posições é `crates/analise/src/naodart/yaml.rs`, sobre
os eventos marcados do `yaml-rust2` 0.10 (API conferida na fonte do crate): o fim de cada nó é
calculado ali (aspas casadas, bloco pela indentação, `]`/`}` do fluxo, último filho do bloco).
`crates/analise/src/naodart/mod.rs` define `CodigoNaoDart` e `Relato` e passa a incluir as tabelas
geradas. Saída: `dartforge_paridade::diagnosticos_do_pubspec`, chamada pelo `dartforge analyze` quando
o alvo é a pasta do pacote. Fora: os lints de pubspec, o `# ignore` do pubspec, o `errors:` das opções
sobre estes códigos, e o `documentation` conferido contra o oráculo.

#### `analyzeAnalysisOptions` — include e erro de parse (`analyzer/lib/src/task/options.dart:25-193`)

Mecanismo e pseudocódigo no Mecanismo 3. Os cinco relatos são construídos à
mão (`AnalysisError.tmp`, sem `ErrorReporter`); quatro deles sempre na posição
do valor do `include:` do arquivo inicial.

| código | constante | emissor (arquivo:linha, método) | condição | posição | DartForge |
|---|---|---|---|---|---|
| `included_file_parse_error` | `AnalysisOptionsErrorCode.INCLUDED_FILE_PARSE_ERROR` | `analyzer/lib/src/task/options.dart:170` `analyzeAnalysisOptions.validate` | o texto de um arquivo incluído (direto ou transitivo) lança `YamlException`; args `[fullName do incluído, início, fim, mensagem do yaml]` (aqui `{2}` é o offset final, `e.span.end.offset`) | `initialIncludeSpan` (offset e length do valor do `include:` do arquivo inicial) | **não implementado** — ver especificação de `parse_error` |
| `included_file_warning` | `AnalysisOptionsWarningCode.INCLUDED_FILE_WARNING` | `analyzer/lib/src/task/options.dart:62` `addDirectErrorOrIncludedError`; `:130` `validate` | (a) cada diagnóstico produzido ao validar um arquivo incluído; (b) ciclo de includes que não passa pelo arquivo inicial | `initialIncludeSpan` | **não implementado** — ver especificação completa |
| `include_file_not_found` | `AnalysisOptionsWarningCode.INCLUDE_FILE_NOT_FOUND` | `analyzer/lib/src/task/options.dart:117` `validate` | `resolveUri` devolve `null` ou o `Source` não existe, em qualquer nível da cadeia | `initialIncludeSpan` | **não implementado** — ver especificação completa |
| `recursive_include_file` | `AnalysisOptionsWarningCode.RECURSIVE_INCLUDE_FILE` | `analyzer/lib/src/task/options.dart:105` `validate` | o `include:` (de qualquer nível) resolve para o arquivo inicial; args `[texto do include, fullName do arquivo que o contém]` | `initialIncludeSpan` | **não implementado** — ver especificação de `include_file_not_found` |
| `parse_error` | `AnalysisOptionsErrorCode.PARSE_ERROR` | `analyzer/lib/src/task/options.dart:187` `analyzeAnalysisOptions` | `loadYamlNode` do arquivo inicial lança `YamlException` (`analysis_options_provider.dart:95-99`) | `e.span`: `span.start.offset`, `span.length` do `YamlException` | **não implementado** — ver especificação completa |

**Estado em 2026-10-05 (escrito, não compilado).** `analyzeAnalysisOptions` e os validadores estão em
`crates/analise/src/naodart/opcoes.rs`, com `task/options.dart` e `options_rule_validator.dart` da
6.11.0 abertos: erro de sintaxe, a cadeia de `include:` (não achado, recursivo, avisos e erro de
sintaxe do incluído relatados no `include:` inicial), `analyzer` (chaves, `strong-mode`, `errors`,
`language`, `optional-checks`, `cannot-ignore`, `plugins`), `code-style`, `formatter`, `linter` e as
regras de `linter: rules:` (indefinida, incompatível, repetida, removida). As tabelas de lint ganharam
o módulo `crates/analise/src/lints/mod.rs`. Saída: `dartforge_paridade::diagnosticos_das_opcoes`,
chamada pelo `dartforge analyze`. Diferenças: o texto e o comprimento do erro de sintaxe são os do
`yaml-rust2`; os nomes de `enable-experiment` não são conferidos; `replaced_lint` e
`deprecated_lint_with_replacement` não saem (a tabela não guarda a substituta); a restrição de SDK não
é passada, logo regra removida com `since` não é relatada pelo `dartforge analyze`. O
`ManifestValidator` está em `crates/analise/src/naodart/manifesto.rs` (2026-10-05, escrito, não compilado): o analisador de XML e as sete checagens do original transcritos, ligado por `Opcoes::manifesto_do_chrome_os` e `dartforge_paridade::diagnosticos_do_manifesto`.

**Estado em 2026-10-05 (escrito, não compilado), lints.** Primeiro lote em
`crates/analise/src/lints/regras.rs`, dez regras que só olham a árvore: `camel_case_types`,
`camel_case_extensions`, `non_constant_identifier_names`, `empty_catches`, `avoid_empty_else`,
`curly_braces_in_flow_control_structures`, `use_string_in_part_of_directives`,
`prefer_generic_function_type_aliases`, `provide_deprecation_message` e `avoid_relative_lib_imports`.
Os emissores lidos são os do `main` do SDK (`E:/references\dart-sdk\pkg\linter`), porque a fonte
do linter 3.6.2 não estava na máquina. **Conferido em 2026-10-05**: `pkg/linter` da tag 3.6.2 foi
extraído do repositório de referência para `E:/references/dart-sdk-3.6.2/pkg/linter`, e as quarenta
regras foram comparadas com ele. Seis diferiam e foram ajustadas à 3.6.2: as duas de ordem de
construtores relatam no nome da classe (e a dos sem nome só olha extension type com construtor
primário nomeado); `always_put_required_named_parameters_first` não isenta `super.x`;
`empty_constructor_bodies` relata também o `factory`; o prefixo `_` não é curinga nas duas regras de
prefixo; `use_function_type_syntax_for_parameters` só olha o parâmetro comum. Ligação: `diagnosticos_json`
(`crates/paridade/src/lib.rs`) roda as regras ligadas em `linter: rules:` (com os `include:` fundidos)
sobre a árvore do texto e as emite como `LINT`/`INFO`. Segundo lote, em `lints/regras2.rs`, mais onze:
`avoid_return_types_on_setters`, `empty_constructor_bodies`, `library_prefixes`,
`no_leading_underscores_for_library_prefixes` (só a variante sem sombreamento),
`constant_identifier_names`, `prefer_is_not_operator`, `prefer_adjacent_string_concatenation`,
`unnecessary_null_in_if_null_operators`, `unnecessary_late`, `empty_statements` e
`use_rethrow_when_possible`. Terceiro lote, em `lints/regras3.rs`, mais seis: `unnecessary_new`,
`prefer_typing_uninitialized_variables`, `always_declare_return_types` (sem a isenção dos `test_*`),
`library_names`, `slash_for_doc_comments` e `unnecessary_brace_in_string_interps`. Quarto lote, em
`lints/regras4.rs`, mais cinco: `prefer_if_null_operators`,
`avoid_single_cascade_in_expression_statements`, `use_function_type_syntax_for_parameters`,
`no_leading_underscores_for_local_identifiers` (só a variante sem sombreamento) e
`prefer_function_declarations_over_variables`. Quinto lote, em `lints/regras5.rs`, mais seis:
`sort_constructors_first`, `sort_unnamed_constructors_first`,
`always_put_required_named_parameters_first`,
`avoid_multiple_declarations_per_line`, `eol_at_end_of_file` e `unnecessary_raw_strings`. Primeira
regra com o programa resolvido: `annotate_overrides`, em `crates/types/src/fase_override.rs`
(`sem_anotacao_de_override`, com a mesma noção de "sobrescreve" do `OverrideVerifier`), guardada em
`Arquivo::lints_semanticos` pelo motor e emitida por `diagnosticos_json` quando a regra está ligada.
Pelo mesmo canal, com os tipos da inferência: `prefer_is_empty` (os quatro códigos), em
`crates/types/src/lints_tipados.rs`, com o contexto constante aproximado.
**Lotes de 2026-10-05, escritos direto da fonte 3.6.2** (`E:/references/dart-sdk-3.6.2/pkg/linter`),
não compilados. Sexto, em `lints/regras6.rs`, dezesseis: `control_flow_in_finally`,
`throw_in_finally`, `avoid_final_parameters`, `avoid_void_async`, `avoid_returning_null_for_void`
(só com o tipo de retorno escrito), `prefer_asserts_with_message`, `unnecessary_library_directive`,
`no_self_assignments`, `avoid_annotating_with_dynamic`, `unnecessary_constructor_name`,
`unnecessary_breaks`, `prefer_expression_function_bodies`, `combinators_ordering`,
`prefer_null_aware_operators`, `one_member_abstracts` e `avoid_private_typedef_functions` (conta os
usos só na unidade). Sétimo, em `lints/regras7.rs`, sete, sobre o lexema das strings e as linhas:
`prefer_single_quotes`, `prefer_double_quotes`, `avoid_escaping_inner_quotes`,
`unnecessary_string_escapes`, `use_raw_strings`, `leading_newlines_in_multiline_strings` e
`lines_longer_than_80_chars`. Oitavo, em `lints/regras8.rs`, nove: `prefer_conditional_assignment`,
`join_return_with_assignment`, `literal_only_boolean_expressions`,
`avoid_unused_constructor_parameters`, `unnecessary_const` (com o `constantContext` do analyzer),
`avoid_init_to_null` e `type_init_formals` (as duas pelo tipo escrito), `always_put_control_body_on_new_line`
e `directives_ordering`. Dois apoios novos: `lints/andar.rs`, o passeio pela árvore com a pilha de
ancestrais (o `thisOrAncestorMatching` do analyzer; a árvore em arenas não tem ponteiro para o pai), e
`lints/cordas.rs`, que relê do texto cada literal de string (a árvore junta os adjacentes e guarda o
valor já decodificado) e os comentários. As diferenças conhecidas de cada regra estão no cabeçalho do
arquivo dela. Nono, em `lints/regras9.rs`, cinco: `unnecessary_final` (os dois códigos),
`hash_and_equals`, `unnecessary_getters_setters`, `recursive_getters` e
`prefer_initializing_formals`. Décimo, em `lints/regras10.rs`, cinco: `prefer_inlined_adds` (os dois
códigos), `prefer_spread_collections`, `avoid_field_initializers_in_const_classes`,
`avoid_classes_with_only_static_members` (só a classe sem supertipo escrito) e
`prefer_final_in_for_each` (os dois códigos). Segundo módulo de regras com os tipos da inferência,
`crates/types/src/lints_tipados2.rs`, nove, pelo mesmo canal de `Arquivo::lints_semanticos`:
`avoid_bool_literals_in_conditional_expressions`, `no_literal_bool_comparisons`,
`unnecessary_string_interpolations`, `prefer_contains` (os três códigos), `prefer_is_not_empty`,
`use_is_even_rather_than_modulo`, `unnecessary_to_list_in_spreads`,
`unnecessary_null_aware_assignments` e `await_only_futures`. O condutor passou a tirar o relato de
lint repetido (mesmo código, lugar e argumentos). Total: oitenta regras sintáticas e onze com o
programa resolvido, de 236 arquivos de regra na 3.6.2 (algumas removidas, sem emissor).
Fora: as outras 145
regras e o LSP. Os comentários `// ignore:` (com `type=lint`) e o `errors:` das opções já valem para
os lints (`Ignorados::ignora_lint`, `diagnosticos_json`); o `cannot-ignore` ainda não.

**Lotes 11 a 13, escritos em 2026-10-05 direto da fonte 3.6.2, não compilados.** Décimo primeiro, em
`lints/regras11.rs`, oito: `no_adjacent_strings_in_list`, `missing_whitespace_between_adjacent_strings`,
`flutter_style_todos`, `unnecessary_library_name`, `prefer_null_aware_method_calls`, `avoid_print`
(o `print` do `dart:core` pelo nome), `avoid_catches_without_on_clauses` (as chamadas que não voltam
e as que entregam o erro pelo nome) e `avoid_setters_without_getters` (só sem `extends`/`with`).
Décimo segundo, em `lints/regras12.rs`, cinco: `prefer_if_elements_to_conditional_expressions`,
`prefer_for_elements_to_map_fromIterable`, `prefer_final_parameters`, `prefer_foreach` e
`prefer_asserts_in_initializer_lists` (os membros de instância pelo nome, só com a hierarquia toda na
unidade). Décimo terceiro, em `lints/regras13.rs`, dois: `avoid_shadowing_type_parameters` e
`parameter_assignments` (com a regra literal do emissor: toda expressão prefixa ou pós-fixa sobre o
parâmetro mutado conta). Terceiro módulo com os tipos, `crates/types/src/lints_tipados3.rs`, seis: `use_truncating_division`, `avoid_double_and_int_checks`, `only_throw_errors`, `no_runtimeType_toString`, `avoid_dynamic_calls` e `unrelated_type_equality_checks` (a forma de expressão; o subtipo entre interfaces pela classe). Total: noventa e cinco regras sintáticas e dezessete com o programa resolvido.
Doze das que faltam são marcadores de regra removida na 3.6.2, sem emissor:
`always_require_non_null_named_parameters`, `avoid_as`, `avoid_returning_null`,
`avoid_returning_null_for_future`, `avoid_unstable_final_fields`, `enable_null_safety`,
`invariant_booleans`, `iterable_contains_unrelated_type`, `list_remove_unrelated_type`,
`prefer_bool_in_asserts`, `prefer_equal_for_default_values` e `super_goes_last`. Restam cerca de 120
com emissor, a maioria dependente de tipos (`unrelated_type_equality_checks`, `avoid_dynamic_calls`,
`unawaited_futures`, `prefer_const_constructors`…) ou do Flutter.

#### Validadores de `task/options.dart` — `OptionsFileValidator` (`analyzer/lib/src/task/options.dart:765-795`)

`OptionsFileValidator.validate` roda, nesta ordem (`:776-785`, `:787-794`):
`AnalyzerOptionsValidator` — composto de `TopLevelAnalyzerOptionsValidator`,
`StrongModeOptionValueValidator`, `ErrorFilterOptionValidator`,
`EnabledExperimentsValidator`, `LanguageOptionValidator`,
`OptionalChecksValueValidator`, `CannotIgnoreOptionValidator` (`:321-332`) —,
`CodeStyleOptionsValidator`, `LinterOptionsValidator` e
`LinterRuleOptionsValidator`. `PluginsOptionValidator` roda fora dele, chamado
por `_validatePluginsOption` (`:222-231`) depois do include. Cada validador
recebe o mapa de topo e procura a sua seção; **chaves de topo desconhecidas
não são validadas** (`formatter:`, erros de digitação em `analyzer:` como chave
de topo etc. passam em silêncio), e `analyzer:`/`linter:` que não são mapa
também (`:993-994`, TODO). As propostas de valores usam
`quotedAndCommaSeparatedWithAnd`
(`analyzer/lib/src/utilities/extensions/string.dart:43-96`): `'a'`;
`'a' and 'b'`; `'a', 'b', and 'c'`.

```text
TopLevelOptionValidator(seção, suportadas).validate:            (:976-995)
  nó = options[seção]; se é YamlMap: para cada chave escalar k fora de suportadas:
     código = (suportadas.length == 1) ? UNSUPPORTED_OPTION_WITH_LEGAL_VALUE : ..._VALUES   (:972-974)
     relata em k.span, [seção, k.value, proposta]
  # analyzer: suportadas = cannot-ignore, enable-experiment, errors, exclude, language,
  #           optional-checks, plugins, strong-mode                                   (:279-288, :955-958)
  # linter:   suportadas = rules                                                      (:709-711)

ErrorBuilder(lista).reportError(reporter, escopo, nó):          (:522-569)
  código = lista vazia -> _WITHOUT_VALUES; 1 item -> _WITH_LEGAL_VALUE; senão -> _WITH_LEGAL_VALUES
  relata em nó.span, [escopo, nó.valueOrThrow, proposta]
  # usado por language (3 itens), strong-mode (3) e optional-checks (2): sempre _WITH_LEGAL_VALUES

ErrorFilterOptionValidator (analyzer: errors:):                 (:599-648)
  mapa: para cada (k, v):
     k escalar e UPPER(k) fora de {nomes de errorCodeValues} ∪ {UPPER(nome de lint registrado)} ∪ {MISSING_RETURN}
        -> UNRECOGNIZED_ERROR_CODE em k.span, [k.value.toString()]                    (:607-618)
     v escalar e lower(v) fora de [ignore, false, include, true, error, info, warning]
        -> UNSUPPORTED_OPTION_WITH_LEGAL_VALUES em v.span, ['errors', v.value.toString(), proposta]   (:619-631)
     v não escalar -> INVALID_SECTION_FORMAT em v.span, ['enable-experiment']  (sic)  (:632-638)
  não mapa e não nulo -> INVALID_SECTION_FORMAT em errors.span, ['enable-experiment']  (sic)   (:640-646)

LanguageOptionValidator (analyzer: language:):                  (:655-704)
  mapa: chave fora de strict-casts/strict-inference/strict-raw-types -> builder('language', k)   (:664-667)
        chave válida, v escalar, lower(v) fora de true/false -> UNSUPPORTED_VALUE em v.span,
            [chave, v.valueOrThrow, "'true' and 'false'"]                             (:673-687)
  escalar não nulo ou lista -> INVALID_SECTION_FORMAT em language.span, ['language']  (:690-701)

StrongModeOptionValueValidator (analyzer: strong-mode:):        (:898-951)
  não mapa e não nulo -> INVALID_SECTION_FORMAT ['strong-mode']                       (:905-910)
  mapa: chave fora de declaration-casts/implicit-casts/implicit-dynamic -> builder('strong-mode', k)   (:920-921)
        chave declaration-casts -> UNSUPPORTED_VALUE em v.span, ['strong-mode', v, proposta] sempre   (:922-931)
        chave válida, v escalar fora de true/false -> UNSUPPORTED_VALUE [chave, v, proposta]   (:934-946)

OptionalChecksValueValidator (analyzer: optional-checks:):      (:718-761)
  escalar: lower(v) != 'chrome-os-manifest-checks' -> builder('chrome-os-manifest-checks', v)   (:723-728)
  mapa: chave != 'chrome-os-manifest-checks' -> builder('chrome-os-manifest-checks', k)   (:732-736)
        senão lower(v.value) fora de true/false -> UNSUPPORTED_VALUE [chave, v, proposta]   (:737-749)
  outro não nulo (lista) -> INVALID_SECTION_FORMAT ['enable-experiment']  (sic)       (:753-758)

EnabledExperimentsValidator (analyzer: enable-experiment:):     (:482-518)
  lista: flags = toString() de cada nó; para cada resultado de validateFlags(flags):
           UnrecognizedFlag -> UNSUPPORTED_OPTION_WITHOUT_VALUES no item, ['enable-experiment', flag]
           outro            -> INVALID_OPTION no item, ['enable-experiment', resultado.message]
  não lista e não nulo -> INVALID_SECTION_FORMAT ['enable-experiment']                (:510-515)

CannotIgnoreOptionValidator (analyzer: cannot-ignore:):         (:355-400)
  lista: item String: se é 'error'/'info'/'warning' ok; senão UPPER fora dos três conjuntos
            -> UNRECOGNIZED_ERROR_CODE no item, [nome]                                (:364-377)
         item não String -> INVALID_SECTION_FORMAT no item, ['cannot-ignore']         (:384-390)
  não lista e não nulo -> INVALID_SECTION_FORMAT ['cannot-ignore']                    (:392-397)

CodeStyleOptionsValidator (code-style: de topo):                (:404-464)
  mapa: chave 'format' -> _validateFormat(valor); outra -> UNSUPPORTED_OPTION_WITHOUT_VALUES na chave,
            ['code-style', chave.toString()]                                          (:409-420)
  escalar não nulo ou lista -> INVALID_SECTION_FORMAT ['code-style']                  (:421-433)
  _validateFormat: mapa ou lista -> INVALID_SECTION_FORMAT ['format'];
                   escalar com toBool == null -> UNSUPPORTED_VALUE ['format', valor, proposta]   (:436-463)
```

| código | constante | emissor (arquivo:linha, método) | condição | posição | DartForge |
|---|---|---|---|---|---|
| `analysis_option_deprecated` | `AnalysisOptionsWarningCode.ANALYSIS_OPTION_DEPRECATED` | sem emissor no 3.6.2 | `grep -rn ANALYSIS_OPTION_DEPRECATED` em `analyzer/lib`, `analysis_server/lib` e nos pacotes de `sdk362/pkg` só acha a definição (`option_codes.g.dart:129`) e a lista `analyzer/lib/src/error/error_code_values.g.dart:31` | — | **não implementado** — nada a fazer para a paridade 3.6.2; o nome precisa constar da lista de códigos válidos de `errors:` |
| `analysis_option_deprecated` | `AnalysisOptionsWarningCode.ANALYSIS_OPTION_DEPRECATED_WITH_REPLACEMENT` | sem emissor no 3.6.2 | o mesmo grep só acha `option_codes.g.dart:140`, `error_code_values.g.dart:32` e o produtor de correção `analysis_server/lib/src/services/correction/fix/analysis_options/fix_generator.dart:30`, `:85` (consumidor, não emissor); `strong-mode: implicit-casts`/`implicit-dynamic` são aceitos sem aviso (`options.dart:291-295`) | — | **não implementado** — idem; mesmo `name` (`ANALYSIS_OPTION_DEPRECATED`), `uniqueName` próprio |
| `invalid_option` | `AnalysisOptionsWarningCode.INVALID_OPTION` | `analyzer/lib/src/task/options.dart:502` `EnabledExperimentsValidator.validate` | item de `enable-experiment` reconhecido mas inválido segundo `validateFlags` (`analyzer/lib/src/dart/analysis/experiments_impl.dart:191-220`): flag expirada (desnecessária ou ilegal) ou conflitante com anterior; args `['enable-experiment', mensagem]` | o item da lista | **não implementado** — ver especificação completa |
| `invalid_section_format` | `AnalysisOptionsWarningCode.INVALID_SECTION_FORMAT` | `analyzer/lib/src/task/options.dart:387`, `:395` (`CannotIgnoreOptionValidator`), `:424`, `:430`, `:440`, `:459` (`CodeStyleOptionsValidator`), `:513` (`EnabledExperimentsValidator`), `:635`, `:643` (`ErrorFilterOptionValidator`), `:693`, `:699` (`LanguageOptionValidator`), `:756` (`OptionalChecksValueValidator`), `:908` (`StrongModeOptionValueValidator`) | a seção (ou um item dela) tem a forma YAML errada, conforme o pseudocódigo; `{0}` é o nome da seção — **`enable-experiment`** nos sítios de `errors` e de `optional-checks` (cópia errada na fonte, a reproduzir) | o nó de forma errada (valor da seção ou item) | **não implementado** — ver especificação completa |
| `multiple_plugins` | `AnalysisOptionsWarningCode.MULTIPLE_PLUGINS` | `analyzer/lib/src/task/options.dart:818`, `:830`, `:850`, `:864`, `:884` `PluginsOptionValidator.validate` | `analyzer: plugins:` com mais de um plugin distinto: cada item (lista) ou chave (mapa) diferente do primeiro plugin `String`; ou, havendo plugin num arquivo incluído (`firstPluginName`), cada plugin diferente dele (escalar, item ou chave); `{0}` = o primeiro plugin | o escalar, o item ou a chave do plugin excedente | **não implementado** — plugins não existem no DartForge |
| `unrecognized_error_code` | `AnalysisOptionsWarningCode.UNRECOGNIZED_ERROR_CODE` | `analyzer/lib/src/task/options.dart:375` `CannotIgnoreOptionValidator.validate`; `:614` `ErrorFilterOptionValidator.validate` | chave de `errors:` (ou item de `cannot-ignore:` que não é severidade) cujo nome em maiúsculas não é `name` de nenhum `errorCodeValues`, nem nome de lint registrado, nem `MISSING_RETURN` | a chave (ou o item) | **não implementado** — ver especificação completa |
| `unsupported_option_without_values` | `AnalysisOptionsWarningCode.UNSUPPORTED_OPTION_WITHOUT_VALUES` | `analyzer/lib/src/task/options.dart:416` `CodeStyleOptionsValidator.validate`; `:496` `EnabledExperimentsValidator.validate`; (`:524` `ErrorBuilder.noProposalCode`, inalcançável: nenhuma lista vazia) | chave de `code-style:` diferente de `format`; ou flag de `enable-experiment` desconhecida | a chave; o item da lista | **não implementado** — ver especificação completa |
| `unsupported_option_with_legal_value` | `AnalysisOptionsWarningCode.UNSUPPORTED_OPTION_WITH_LEGAL_VALUE` | `analyzer/lib/src/task/options.dart:973`, `:983-987` `TopLevelOptionValidator.validate` (instância `LinterOptionsValidator`, :709-711); (`:530` `ErrorBuilder.singularProposalCode`, inalcançável: nenhuma lista de 1 item) | chave de `linter:` diferente de `rules`; args `['linter', chave, "'rules'"]` | a chave | **não implementado** — ver especificação completa |
| `unsupported_option_with_legal_values` | `AnalysisOptionsWarningCode.UNSUPPORTED_OPTION_WITH_LEGAL_VALUES` | `analyzer/lib/src/task/options.dart:974`, `:983-987` `TopLevelOptionValidator.validate` (instância `TopLevelAnalyzerOptionsValidator`, :955-958); `:624` `ErrorFilterOptionValidator.validate`; `:527` + `:554-560` `ErrorBuilder.reportError` (de `:667`, `:726`, `:735`, `:921`) | chave desconhecida em `analyzer:`, `analyzer: language:`, `analyzer: strong-mode:`, `analyzer: optional-checks:`; valor de `optional-checks` escalar desconhecido; valor de `errors:` fora dos sete legais | a chave ou o valor | **não implementado** — ver especificação completa |
| `unsupported_value` | `AnalysisOptionsWarningCode.UNSUPPORTED_VALUE` | `analyzer/lib/src/task/options.dart:448` `CodeStyleOptionsValidator._validateFormat`; `:680` `LanguageOptionValidator.validate`; `:742` `OptionalChecksValueValidator.validate`; `:925`, `:939` `StrongModeOptionValueValidator._validateStrongModeAsMap` | valor não booleano para chave válida de `language`, `strong-mode`, `optional-checks: chrome-os-manifest-checks` ou `code-style: format`; e **sempre** para `strong-mode: declaration-casts` | o valor | **não implementado** — ver especificação completa |

#### `LinterRuleOptionsValidator` (`analyzer/lib/src/lint/options_rule_validator.dart`)

Pseudocódigo no Mecanismo 3. Todos em `node.span` do nome da regra (item da
lista, ou **chave** na forma de mapa), via `reporter.atSourceSpan`.

| código | constante | emissor (arquivo:linha, método) | condição | posição | DartForge |
|---|---|---|---|---|---|
| `deprecated_lint` | `AnalysisOptionsHintCode.DEPRECATED_LINT` | `analyzer/lib/src/lint/options_rule_validator.dart:121` `_validateRules.validateRule` | arquivo direto (não incluído), regra com `DeprecatedState` sem `replacedBy` e `currentSdkAllows(since)`; **inalcançável com o registro do 3.6.2** (nenhuma regra depreciada) | o nome da regra | **não implementado** — ver especificação completa |
| `deprecated_lint_with_replacement` | `AnalysisOptionsHintCode.DEPRECATED_LINT_WITH_REPLACEMENT` | `analyzer/lib/src/lint/options_rule_validator.dart:115` `validateRule` | idem, com `replacedBy`; args `[regra, substituta]`; inalcançável no 3.6.2 | o nome da regra | **não implementado** — idem |
| `duplicate_rule` | `AnalysisOptionsHintCode.DUPLICATE_RULE` | `analyzer/lib/src/lint/options_rule_validator.dart:101` `validateRule` | regra registrada, habilitada, sem incompatível já vista, e já presente em `seenRules`; severidade INFO | a ocorrência repetida | **não implementado** — ver especificação completa |
| `incompatible_lint` | `AnalysisOptionsWarningCode.INCOMPATIBLE_LINT` | `analyzer/lib/src/lint/options_rule_validator.dart:95` `validateRule` | regra habilitada cuja lista `incompatibleRules` contém uma regra já vista antes no mesmo arquivo; args `[regra, incompatível]` | a segunda regra do par | **não implementado** — ver especificação completa |
| `removed_lint` | `AnalysisOptionsWarningCode.REMOVED_LINT` | `analyzer/lib/src/lint/options_rule_validator.dart:137` `validateRule` | arquivo direto, regra com `RemovedState` sem `replacedBy` e `currentSdkAllows(since)` (a restrição `sdk:` do pubspec contém a versão; ou `since` nulo); args `[regra, since.toString()]` | o nome da regra | **não implementado** — ver especificação completa |
| `replaced_lint` | `AnalysisOptionsWarningCode.REPLACED_LINT` | `analyzer/lib/src/lint/options_rule_validator.dart:131` `validateRule` | idem, com `replacedBy`; args `[regra, since, substituta]`; inalcançável no 3.6.2 (nenhuma removida com substituta) | o nome da regra | **não implementado** — ver especificação de `removed_lint` |
| `undefined_lint` | `AnalysisOptionsWarningCode.UNDEFINED_LINT` | `analyzer/lib/src/lint/options_rule_validator.dart:84` `validateRule` | valor não nulo que não é `name` de nenhuma regra registrada (comparação `==` com o valor YAML: um número ou bool nunca casa) | o item (ou a chave) | **não implementado** — ver especificação completa |

#### `ManifestValidator` (`analyzer/lib/src/manifest/manifest_validator.dart:396-573`)

Só roda com `chromeOsManifestChecks` (Mecanismo 1). `_checkManifestTag`
(`:419-446`) avança o parser até o elemento `manifest`; coleta os filhos
`uses-feature` e `uses-permission`; chama `_validateTouchScreenFeature`,
`_validateFeatures`, `_validatePermissions` e, para cada `activity` filho de
`application`, `_validateActivity`. Todos relatam por `_reportErrorForNode`
(`:456-467`): span do **atributo** indicado, ou do elemento quando a chave é
`null`. Severidade WARNING (`manifest_warning_code.g.dart:128`). Sem `# ignore`.

| código | constante | emissor (arquivo:linha, método) | condição | posição | DartForge |
|---|---|---|---|---|---|
| `camera_permissions_incompatible` | `ManifestWarningCode.CAMERA_PERMISSIONS_INCOMPATIBLE` | `analyzer/lib/src/manifest/manifest_validator.dart:530` `_validatePermissions` | `uses-permission` com `android:name="android.permission.CAMERA"` e falta o `uses-feature` `android.hardware.camera` **ou** o `android.hardware.camera.autofocus` | atributo `android:name` do `uses-permission` | **não implementado** — sem leitor de XML; só faz sentido com `optional-checks` |
| `non_resizable_activity` | `ManifestWarningCode.NON_RESIZABLE_ACTIVITY` | `analyzer/lib/src/manifest/manifest_validator.dart:482` `_validateActivity` | `activity` com `android:resizeableActivity="false"` | atributo `android:resizeableactivity` (nome em minúsculas no parser) | **não implementado** — idem |
| `no_touchscreen_feature` | `ManifestWarningCode.NO_TOUCHSCREEN_FEATURE` | `analyzer/lib/src/manifest/manifest_validator.dart:570` `_validateTouchScreenFeature` | nenhum `uses-feature` com `android:name="android.hardware.touchscreen"` | o elemento `manifest` (`node.sourceSpan`) | **não implementado** — idem |
| `permission_implies_unsupported_hardware` | `ManifestWarningCode.PERMISSION_IMPLIES_UNSUPPORTED_HARDWARE` | `analyzer/lib/src/manifest/manifest_validator.dart:540` `_validatePermissions` | `uses-permission` (que não é CAMERA) cujo nome está em `getImpliedUnsupportedHardware` (`analyzer/lib/src/manifest/manifest_values.dart:95-124`: as 11 permissões de telefonia → `android.hardware.telephony`); `{0}` = a feature | atributo `android:name` do `uses-permission` | **não implementado** — idem |
| `setting_orientation_on_activity` | `ManifestWarningCode.SETTING_ORIENTATION_ON_ACTIVITY` | `analyzer/lib/src/manifest/manifest_validator.dart:476` `_validateActivity` | `activity` com `android:screenOrientation` em `unsupportedOrientations` (`manifest_values.dart:80-89`) | atributo `android:screenorientation` | **não implementado** — idem |
| `unsupported_chrome_os_feature` | `ManifestWarningCode.UNSUPPORTED_CHROME_OS_FEATURE` | `analyzer/lib/src/manifest/manifest_validator.dart:515` `_validateFeatures`; `:565` `_validateTouchScreenFeature` | `uses-feature` de `unsupportedHardwareFeatures` (`manifest_values.dart:38-78`) ou o touchscreen, com `android:required="true"`; `{0}` = nome da feature | atributo `android:name` do `uses-feature` | **não implementado** — idem |
| `unsupported_chrome_os_hardware` | `ManifestWarningCode.UNSUPPORTED_CHROME_OS_HARDWARE` | `analyzer/lib/src/manifest/manifest_validator.dart:503` `_validateFeatures`; `:558` `_validateTouchScreenFeature` | idem, **sem** o atributo `android:required` | atributo `android:name` do `uses-feature` | **não implementado** — idem |

#### Especificações completas — arquivos não-Dart

**No DartForge (vale para todas as especificações abaixo).** Estado conferido:

- Emissão: `grep -rnE "::<CONSTANTE>\b"` das 53 constantes em
  `crates/{analise,types,frontend,elements,cli}` não acha nada; nenhum dos 52
  nomes emitidos está em `crates/analise/verificados.txt`; e os nomes nem
  existem na tabela (`grep -rniE "asset_does_not_exist|undefined_lint|include_file_not_found|unnecessary_dev_dependency|no_touchscreen_feature|unrecognized_error_code" crates --include=*.rs --include=*.txt`
  vazio): `crates/diagnostics/src/codigos_g.rs` só tem os módulos de códigos
  Dart (`:2104` `compile_time_error`, `:2657` `static_warning`, `:2668`
  `warning`, `:2816` `hint`, `:2828` `ffi`, `:2880` `parser`, `:3152`
  `scanner`, `:3168` `todo`). Os 53 são **não implementados**.
- O que existe de leitura desses arquivos:
  - `crates/paridade/src/filtros.rs:33-90` (`Opcoes::ler`/`de_texto`): lê
    `<raiz>/analysis_options.yaml` linha a linha, por indentação, e extrai só
    `analyzer: exclude:`, `analyzer: errors:` e `analyzer: cannot-ignore:`;
    não segue `include:` (comentário em `:12-13`), não guarda posições, ignora
    valores desconhecidos (`:77`, `_ => continue`). Usado pelo
    `dartforge analyze` (`crates/cli/src/analisar.rs:39-43`) e pelo LSP
    (`crates/lsp/src/tipado.rs:322-338`).
  - `pubspec.yaml`: só como marcador de raiz de pacote
    (`crates/cli/src/analisar.rs:10-20`, `crates/lsp/src/projeto.rs:107-136`,
    `crates/lsp/src/servidor.rs:1241`) e para o nome do pacote por busca de
    linha `name:` (`crates/lsp/src/indice.rs:257-263`). O `crates/build` tem um
    leitor YAML de verdade (`yaml-rust2`, `crates/build/Cargo.toml:25`,
    `crates/build/src/config.rs:13-16`) para pubspec/`build.yaml`, sem
    validação nem diagnósticos; se a API dele dá posições por nó:
    **não verificado**.
  - `AndroidManifest.xml`: nenhuma referência
    (`grep -rn AndroidManifest crates --include=*.rs` vazio).
- O `dartforge analyze` só coleta `.dart`
  (`crates/cli/src/analisar.rs:40-47`, `crates/paridade/src/corpus.rs:60-76`)
  e o placar descarta do oráculo tudo que não é `.dart`
  (`crates/paridade/src/oraculo.rs:87-91`): hoje estes diagnósticos não são
  nem produzidos nem medidos.
- O que falta, em ordem: (1) um leitor YAML com span por nó (início/fim em
  UTF-16, escalares com aspas no span, escalar nulo de span vazio, chaves como
  nós) e a reprodução das mensagens do `package:yaml` para `parse_error`;
  (2) os nomes e severidades dos 53 códigos na tabela de diagnósticos, mais a
  lista completa de `errorCodeValues` e dos 240 nomes de lint (para
  `unrecognized_error_code`/`undefined_lint`), com `incompatibleRules` e o
  estado removido/versão; (3) um passo "arquivos não-Dart" no
  `dartforge analyze` e no LSP, depois da coleta dos `.dart`: percorrer os
  arquivos analisados da raiz pelo nome base, com as mesmas exclusões
  (componentes iniciados por `.`, `exclude:`), só quando o alvo é diretório;
  (4) aplicar `errors:` aos três tipos e `# ignore` só ao pubspec; (5) na
  saída padrão, o bloco de "priority errors" do `dart analyze` e a omissão
  deles em `--format=json`/`machine`; (6) no placar, deixar de descartar os
  não-`.dart` no oráculo.

Os casos de teste abaixo dão offset/length em UTF-16, com `\n` como fim de
linha; `<raiz>` é o caminho absoluto do diretório analisado.

##### `asset_does_not_exist`
- **Emissão:** `_validateAssetPath` (`analyzer/lib/src/pubspec/validators/flutter_validator.dart:115-131`), chamado de `flutterValidator` (`:52` para item string, `:69` para `path:` de item mapa); `flutterValidator` é o 3º da lista de `validatePubspec` (`analyzer/lib/src/pubspec/pubspec_validator.dart:25-33`), chamado por `_analyzePubspecYaml` (`analysis_server/lib/src/context_manager.dart:461-486`).
- **Condição exata:**
  1. o documento é mapa, `flutter` é mapa, `assets` é lista (`flutter_validator.dart:14-39`);
  2. o item é escalar `String`, ou mapa com `path:` escalar `String` (`:42-70`);
  3. o texto não começa por `packages/` (`:117`) e **não termina em `/`** (`:120`, `:126-128`);
  4. `assetPath = join(dirname(pubspec), joinAll(posix.split(texto)))` (`:121-124`) e `_assetExistsAtPath` é falso (`:85-111`): não existe pasta em `assetPath`; não existe arquivo em `assetPath`; e — se a pasta-pai existe — nenhuma **subpasta** direta do pai contém um arquivo com o mesmo nome base (`:97-109`); se a pasta-pai não existe, falso.
- **Posição:** o escalar do item (ou do `path:`): `span.start.offset`, `span.length` (com aspas, se houver).
- **Mensagem:** `The asset file '{0}' doesn't exist.` + correção `Try creating the file or fixing the path to the file.`; `{0}` = o valor string como escrito (sem aspas, sem normalizar).
- **Supressões e ordem:** `# ignore: asset_does_not_exist` / `# ignore_for_file:` e `errors:` (Mecanismo 2). Um item escalar não-string antes na lista interrompe o validador (`return` em `:49` e `:66`), calando os itens seguintes. Ordem na lista de erros: depois dos de `dependencyValidator` e `fieldValidator`, antes de `nameValidator` (a saída final é reordenada por offset pelo `dart analyze`).
- **No DartForge:** não implementado (preâmbulo). Casos:
  1. `name: p\nflutter:\n  assets:\n    - assets/a.png\n    - img/\n` sem `assets/` nem `img/` → `asset_does_not_exist` em 33/12 (linha 4, coluna 7) e `asset_directory_does_not_exist` em 52/4 (linha 5, coluna 7).
  2. mesmo pubspec com `assets/2.0x/a.png` existente e `assets/a.png` ausente → nenhum diagnóstico para o primeiro item.
  3. `    - assets` (pasta existente, sem `/`) → nada.
  4. `    - packages/foo/a.png` → nada.

##### `asset_directory_does_not_exist`
- **Emissão:** mesmo método, `flutter_validator.dart:127`.
- **Condição exata:** como `asset_does_not_exist`, com o texto **terminando em `/`** (`:120`). A checagem de existência é a mesma `_assetExistsAtPath` (um arquivo com esse nome também satisfaria; `join` com o segmento vazio final — efeito **não verificado**).
- **Posição:** o escalar do item.
- **Mensagem:** `The asset directory '{0}' doesn't exist.` + `Try creating the directory or fixing the path to the directory.`; `{0}` = o texto com a `/` final.
- **Supressões e ordem:** idem.
- **No DartForge:** não implementado. Casos: o item `img/` do caso 1 acima; `    - img/` com `img/` existente → nada.

##### `invalid_dependency`
- **Emissão:** `validatePathEntries` dentro de `dependencyValidator` (`analyzer/lib/src/pubspec/validators/dependency_validator.dart:70-73` para `path`, `:76-80` para `git`); primeiro validador da lista.
- **Condição exata:**
  1. `isPublishablePackage`: `contents['version'] != null` **e** `asString(contents['publish_to']) != 'none'` (`:88-97`) — sem `version`, nunca relata;
  2. a dependência está em `dependencies` (para `dev_dependencies` o parâmetro é `false`, `:111`) e o valor dela é mapa (`:41-43`);
  3. `path:` é string (`:44-45`) → relata na chave `path` (mesmo que o caminho exista; depois de `path_does_not_exist`/`path_pubspec_does_not_exist`, se houver); se o caminho contém `\`, sai antes (`:49-53`) e **não** relata;
  4. `git:` não nulo (qualquer forma) → relata na chave `git` (`:76-80`).
- **Posição:** o nó da **chave** (`dependency.getKey('path')` / `getKey('git')`), 4 ou 3 caracteres.
- **Mensagem:** `Publishable packages can't have '{0}' dependencies.` + `Try adding a 'publish_to: none' entry to mark the package as not for publishing or remove the {0} dependency.`; `{0}` = `path` ou `git`.
- **Supressões e ordem:** `# ignore`, `errors:`. `dependency_overrides` não é examinado.
- **No DartForge:** não implementado. Casos:
  1. `name: p\nversion: 1.0.0\ndependencies:\n  a:\n    path: ../a\n` com `../a` inexistente → `path_does_not_exist` em 52/4 (`../a`) e `invalid_dependency` em 46/4 (`path`).
  2. o mesmo com `publish_to: none` → só `path_does_not_exist`.
  3. o mesmo sem a linha `version:` → só `path_does_not_exist`.
  4. `  a:\n    git: https://x/y.git` com `version:` → `invalid_dependency` na chave `git` (length 3), `{0}` = `git`.

##### `unnecessary_dev_dependency`
- **Emissão:** `dependencyValidator` (`dependency_validator.dart:103-110`).
- **Condição exata:** `dev_dependencies` é mapa e a chave também é chave do mapa `dependencies` (`declaredDependencies.containsKey(packageName)`, `:105`; os mapas são os `nodes` do `YamlMap`, com igualdade profunda de chave — `yaml/lib/src/loader.dart:156`).
- **Posição:** o nó da chave em `dev_dependencies`.
- **Mensagem:** `The dev dependency on {0} is unnecessary because there is also a normal dependency on that package.` + `Try removing the dev dependency.`; `{0}` = o nome, **sem aspas**.
- **Supressões e ordem:** `# ignore`, `errors:`. Sai antes do `validatePathEntries` dessa dev-dependência.
- **No DartForge:** não implementado. Casos:
  1. `name: p\ndependencies:\n  meta: any\ndev_dependencies:\n  meta: any\n` → em 54/4 (linha 5, coluna 3).
  2. `dev_dependencies:` com pacote só ali → nada.
  3. o caso 1 com `# ignore: unnecessary_dev_dependency` na linha anterior à chave → nada.

##### `path_does_not_exist`
- **Emissão:** três sítios: `validatePathEntries` (`dependency_validator.dart:61-63`), `screenshotsValidator` (`analyzer/lib/src/pubspec/validators/screenshot_validator.dart:31-36`), `_validateDirectoryPath` do `workspaceValidator` (`analyzer/lib/src/pubspec/validators/workspace_validator.dart:59-63`).
- **Condição exata:**
  - dependência (normal **ou** dev) com valor mapa e `path:` string sem `\`: `normalize(absolute(join(dirname(pubspec), joinAll(posix.split(p)))))` não é pasta existente (`dependency_validator.dart:54-63`);
  - `screenshots:` é lista; item é mapa; `path:` é escalar `String`; não existe **arquivo** em `join(dirname(pubspec), …)` (`screenshot_validator.dart:13-37`);
  - `workspace:` é lista; item é escalar `String`; o caminho está contido na pasta do pubspec (senão é `workspace_value_not_subdirectory`); a pasta não existe (`workspace_validator.dart:46-63`).
- **Posição:** o escalar do caminho.
- **Mensagem:** `The path '{0}' doesn't exist.` + `Try creating the referenced path or using a path that exists.`; `{0}` = o texto como escrito.
- **Supressões e ordem:** `# ignore`, `errors:`; para dependência, exclui `path_pubspec_does_not_exist` (ramo `else`) e precede `invalid_dependency`.
- **No DartForge:** não implementado. Casos: o caso 1 de `invalid_dependency`; `dev_dependencies:\n  a:\n    path: ../nao_existe` → relata (dev também é checada); `screenshots:\n  - description: d\n    path: shots/a.png` sem o arquivo → no escalar `shots/a.png`.

##### `path_pubspec_does_not_exist`
- **Emissão:** `validatePathEntries` (`dependency_validator.dart:64-69`).
- **Condição exata:** a pasta resolvida existe e `pasta.getChild('pubspec.yaml').exists` é falso.
- **Posição:** o escalar do `path:`.
- **Mensagem:** `The directory '{0}' doesn't contain a pubspec.` + `Try creating a pubspec in the referenced directory or using a path that has a pubspec.`
- **Supressões e ordem:** como acima.
- **No DartForge:** não implementado. Casos: `dependencies:\n  a:\n    path: lib` (pasta `lib/` existe, sem pubspec) → no escalar `lib`; com `lib/pubspec.yaml` presente → nada.

##### `path_not_posix`
- **Emissão:** `validatePathEntries` (`dependency_validator.dart:49-53`).
- **Condição exata:** `path:` string contendo `\` (em qualquer posição); retorna em seguida — nem existência, nem `invalid_dependency`, nem `git` são checados para essa dependência.
- **Posição:** o escalar do `path:`.
- **Mensagem:** `The path '{0}' isn't a POSIX-style path.` + `Try converting the value to a POSIX-style path.`; `{0}` = o **valor** YAML (num escalar entre aspas duplas, depois dos escapes).
- **Supressões e ordem:** `# ignore`, `errors:`.
- **No DartForge:** não implementado. Casos: `    path: ..\a` → relata no escalar, e nada mais para a dependência, mesmo em pacote publicável.

##### `missing_name`
- **Emissão:** `nameValidator` (`analyzer/lib/src/pubspec/validators/name_validator.dart:10-30`), 4º validador.
- **Condição exata:** o conteúdo não é `YamlMap` (`:12-19`) ou o mapa não tem a chave `name` (`:20-26`). No servidor, documento não-mapa (arquivo vazio, só comentários, lista) já chega como mapa vazio (`context_manager.dart:467-469`) → cai no segundo ramo, mesmo resultado. `name:` vazio ou não-string é `name_not_string` (`:27-28`), não este.
- **Posição:** `atOffset(offset: 0, length: 0)` → 1:1.
- **Mensagem:** `The 'name' field is required but missing.` + `Try adding a field named 'name'.`
- **Supressões e ordem:** `# ignore_for_file: missing_name` em qualquer linha; `# ignore: missing_name` só vale se cair na linha 1 (comentário no fim da linha 1) — um comentário sozinho na linha 1 vale para a linha 2 e **não** cala; `errors:`. Pubspec com YAML inválido não relata nada.
- **No DartForge:** não implementado. Casos: `version: 1.0.0\n` → em 0/0; arquivo vazio → em 0/0; `name:\n` → `name_not_string` (span vazio em 4/0, no `:`), não `missing_name`.

##### `deprecated_field`
- **Emissão:** `fieldValidator` (`analyzer/lib/src/pubspec/validators/field_validator.dart:17-29`), 2º validador.
- **Condição exata:** chave de **topo** escalar string em `{author, authors, transformers, web}` (`:9-14`, `:22-27`).
- **Posição:** o nó da chave.
- **Mensagem:** `The '{0}' field is no longer used and can be removed.` + `Try removing the field.`
- **Supressões e ordem:** `# ignore`, `errors:`.
- **No DartForge:** não implementado. Casos: `name: p\nauthor: x\n` → em 8/6; `authors:\n  - x` → na chave `authors`; `flutter:\n  web: x` (não é topo) → nada.

##### `dependencies_field_not_map`
- **Emissão:** `getDeclaredDependencies` em `dependencyValidator` (`dependency_validator.dart:17-29`), chamado para `dependencies` e `dev_dependencies` (`:83-86`). (O gêmeo de `missing_dependency_validator.dart:66-76` só roda no `dart fix`.)
- **Condição exata:** o campo existe, não é escalar nulo (`dependencies:` vazio é aceito) e não é `YamlMap` — escalar com valor, ou lista.
- **Posição:** o nó do valor do campo.
- **Mensagem:** `The value of the '{0}' field is expected to be a map.` + `Try converting the value to be a map.`; `{0}` = `dependencies` ou `dev_dependencies`.
- **Supressões e ordem:** `# ignore`, `errors:`; o campo passa a valer como vazio para as demais checagens. `dependency_overrides` não é checado.
- **No DartForge:** não implementado. Casos: `name: p\ndependencies: foo\n` → em 22/3; `dependencies:\n` (vazio) → nada; `dev_dependencies:\n  - a` → no nó da lista, `{0}` = `dev_dependencies`.

##### `missing_dependency`
- **Emissão:** `MissingDependencyValidator.validate` (`analyzer/lib/src/pubspec/validators/missing_dependency_validator.dart:57-133`); único chamador `BulkFixProcessor._runPubspecValidatorAndFixGenerator` (`analysis_server/lib/src/services/correction/bulk_fix_processor.dart:932-959`), isto é, o `dart fix`. `validatePubspec` não o chama: **não aparece no `dart analyze` nem em `analysis.errors`**.
- **Condição exata:** dado `usedDeps`/`usedDevDeps` (menos o próprio pacote e `flutter_gen`, `:82-89`): `addDeps` = usados fora de `dependencies`; `addDevDeps` = usados-dev fora de `dependencies` e de `dev_dependencies`; relata se algum não é vazio (`:100-131`). Como os conjuntos são montados: **não verificado**.
- **Posição:** o primeiro valor do mapa de topo (`contents.nodes.values.first`, `:126`).
- **Mensagem:** `Missing a dependency on imported package '{0}'.` + `Try adding {0}.`; `{0}` é uma frase montada: `'a','b' in 'dependencies'`, e/ou `, 'c' in 'dev_dependencies'` (`:116-123`; note o espaço inicial quando só há dev).
- **Supressões e ordem:** não se aplica ao `dart analyze`.
- **No DartForge:** não implementado e fora do alvo de paridade do `dart analyze`; só interessa a um futuro `fix`.

##### `include_file_not_found`
- **Emissão:** `validate` local de `analyzeAnalysisOptions` (`analyzer/lib/src/task/options.dart:111-122`), chamado por `_analyzeAnalysisOptionsYaml` (`analysis_server/lib/src/context_manager.dart:369-394`).
- **Condição exata:** Mecanismo 3: há chave de topo `include`; `includeUri = includeSpan.text`; `sourceFactory.resolveUri(arquivoAtual, includeUri)` não é o arquivo inicial (senão `recursive_include_file`, `:99-110`) e é `null` ou não existe. Vale em qualquer nível: se B (incluído por A) inclui um arquivo inexistente, o relato sai em A.
- **Posição:** `initialIncludeSpan`: offset e length do nó valor do `include:` do arquivo **inicial** (mesmo quando o include quebrado está num arquivo incluído).
- **Mensagem:** `The include file '{0}' in '{1}' can't be found when analyzing '{2}'.` (sem correção); `{0}` = texto do include que falhou (cru, com aspas se escritas); `{1}` = `fullName` (caminho absoluto, separador da plataforma) do arquivo que contém esse include; `{2}` = caminho da raiz do contexto.
- **Supressões e ordem:** sem `# ignore`; `errors: {include_file_not_found: ignore}` no próprio arquivo cala (as opções efetivas do arquivo são calculadas à parte, pelo `AnalysisOptionsProvider`). Os diagnósticos do próprio arquivo (validadores) já foram acrescentados antes (`:77-84`); depois deste relato o `validate` retorna — `multiple_plugins` não é checado nesse nível.
- **No DartForge:** não implementado; `Opcoes::de_texto` nem segue `include:` (`crates/paridade/src/filtros.rs:12-13`). Para a resolução de `package:` há o `package_config.json` já lido pelo motor (`crates/cli/src/analisar.rs:48-49`). Casos:
  1. `include: package:nao_existe/x.yaml\n` → em 9/25; mensagem com `{0}` = `package:nao_existe/x.yaml`, `{1}` = `<raiz>/analysis_options.yaml`, `{2}` = `<raiz>`.
  2. `include: outro.yaml\n` com `outro.yaml` ao lado → nada.
  3. `include: analysis_options.yaml\n` → `recursive_include_file` em 9/21: `The include file 'analysis_options.yaml' in '<raiz>/analysis_options.yaml' includes itself recursively.` + `Try changing the chain of 'include's to not re-include this file.`
  4. `include: b.yaml` e `b.yaml` com `include: c.yaml` inexistente → em 9/6 de `analysis_options.yaml`, `{0}` = `c.yaml`, `{1}` = `<raiz>/b.yaml`.

##### `included_file_warning`
- **Emissão:** `addDirectErrorOrIncludedError` (`options.dart:44-72`) e o ramo de ciclo (`:123-140`).
- **Condição exata:** (a) para cada `AnalysisError` produzido por `OptionsFileValidator` ou `PluginsOptionValidator` sobre um arquivo **incluído** (qualquer profundidade); (b) o include de um arquivo incluído resolve para um `Source` já presente em `includeChain` (ciclo B→…→B que não passa pelo inicial).
- **Posição:** `initialIncludeSpan` (todos empilhados no mesmo ponto).
- **Mensagem:** `Warning in the included options file {0}({1}..{2}): {3}` (sem correção). Em (a): `{0}` = `fullName` do incluído; `{1}` = offset do erro original; `{2}` = `offset + length - 1` (último caractere, inclusivo); `{3}` = a mensagem já formatada do erro original (sem a correção). Em (b): `{0}` = o `Source` (para `FileSource`, `toString()` é o caminho, `analyzer/lib/source/file_source.dart:95-97`); `{1}` = offset e `{2}` = **length** do include que trouxe o arquivo pela primeira vez; `{3}` = `The file includes itself recursively.`
- **Supressões e ordem:** severidade WARNING para qualquer original; `errors: {included_file_warning: ignore}` cala todos; o código original não é consultável (um `errors: {undefined_lint: ignore}` não cala o aviso vindo do incluído). Lints removidos/depreciados em incluídos não geram nada (Mecanismo 3). Arquivos de `package:lints`/`flutter_lints` válidos não geram nada.
- **No DartForge:** não implementado. Casos:
  1. `analysis_options.yaml` = `include: b.yaml\n`; `b.yaml` = `linter:\n  rules:\n    - camel_case_types\n    - nao_existe\n` → em 9/6: `Warning in the included options file <raiz>/b.yaml(46..55): 'nao_existe' is not a recognized lint rule.`
  2. `b.yaml` com `- super_goes_last` (removida) e pubspec `sdk: '>=2.19.0 <4.0.0'` → nada (estado só é checado no arquivo direto).

##### `parse_error` (e `included_file_parse_error`)
- **Emissão:** `analyzeAnalysisOptions` (`options.dart:177-192`) para o arquivo inicial; `validate` (`:156-174`) para incluídos.
- **Condição exata:** `loadYamlNode(texto)` lança `YamlException` (`analyzer/lib/src/analysis_options/analysis_options_provider.dart:95-99`). Documento vazio ou não-mapa **não** é erro (vira mapa vazio, `:97`).
- **Posição:** inicial: `e.span.start.offset`, `e.span.length` (span do `YamlException`, definido pelo `package:yaml`); incluído: `initialIncludeSpan`.
- **Mensagem:** `parse_error`: `{0}` = `e.message` do `package:yaml` (ex.: chave duplicada → `Duplicate mapping key.`, `yaml/lib/src/loader.dart:165`; demais textos do scanner/parser **não verificados**). `included_file_parse_error`: `{3} in {0}({1}..{2})` com `{0}` = `fullName` do incluído, `{1}` = offset inicial, `{2}` = offset **final** (exclusivo), `{3}` = a mensagem.
- **Supressões e ordem:** severidade ERROR, tipo COMPILE_TIME_ERROR (`option_codes.g.dart:62-65`) → no `dart analyze` vai para o bloco prioritário e some de `--format=json`/`machine` (Mecanismo 1). É o único diagnóstico do arquivo (a exceção interrompe tudo). As opções efetivas desse arquivo ficam vazias (`getOptionsFromSource` devolve mapa vazio, `analysis_options_provider.dart:81-83`), então a análise dos `.dart` segue com os padrões. `errors: parse_error: ignore` no próprio arquivo não tem como valer (o arquivo não parseia).
- **No DartForge:** não implementado; `Opcoes::de_texto` é tolerante e nunca falha. Exige reproduzir os textos e spans das exceções do `package:yaml`. Casos:
  1. `linter:\n  rules:\n    a: true\n    a: false\n` → `parse_error`, mensagem `Duplicate mapping key.`, posição no span da segunda chave `a` (offset 33, length 1).
  2. `include: b.yaml` com `b.yaml` contendo o caso 1 → `included_file_parse_error` em 9/6: `Duplicate mapping key. in <raiz>/b.yaml(33..34)`.

##### `undefined_lint`
- **Emissão:** `validateRule` em `LinterRuleOptionsValidator._validateRules` (`analyzer/lib/src/lint/options_rule_validator.dart:76-88`), último validador de `OptionsFileValidator` (`options.dart:780-784`).
- **Condição exata:** `linter` é mapa; `rules` é lista (cada item) ou mapa (cada chave); `node.value != null`; nenhuma regra de `Registry.ruleRegistry` tem `name == value` (`:42-43`). Regras removidas estão registradas (não são indefinidas). Depois do relato, `return`: sem as outras checagens para o item.
- **Posição:** `node.span` do item ou da chave.
- **Mensagem:** `'{0}' is not a recognized lint rule.` + `Try using the name of a recognized lint rule.`; `{0}` = o valor (para um item mapa/lista, o `toString()` do valor — **não verificado**).
- **Supressões e ordem:** `errors: {undefined_lint: ignore}`; sem `# ignore`. Em arquivo incluído vira `included_file_warning`. Sai depois de todos os relatos de `analyzer:`, `code-style:` e chaves de `linter:` na lista interna (a saída é reordenada por offset).
- **No DartForge:** não implementado; precisa da lista dos 240 nomes do `linter` 3.6.2 (`linter/lib/src/rules.dart`). Casos:
  1. `linter:\n  rules:\n    - camel_case_types\n    - nao_existe\n` → em 46/10.
  2. `linter:\n  rules:\n    nao_existe: true\n` → na chave, em 21/10.
  3. `    - super_goes_last` → **não** é `undefined_lint`.
  4. nome de lint de versão posterior ao 3.6.2 (ex.: `unnecessary_underscores`; ausência em `linter/lib/src/rules.dart` do 3.6.2 **não verificada** aqui) → `undefined_lint`.

##### `duplicate_rule`
- **Emissão:** `validateRule` (`options_rule_validator.dart:98-104`).
- **Condição exata:** regra registrada, `enabled` (item de lista, ou chave de mapa com valor `true`), `findIncompatibleRule(rule) == null`, e `seenRules.add(rule.name)` devolve falso (já vista **no mesmo arquivo**; `seenRules` é local a cada `_validateRules`, `:65`, então repetir uma regra que veio de um `include` não conta).
- **Posição:** a ocorrência repetida (2ª, 3ª…).
- **Mensagem:** `The rule {0} is already specified and doesn't need to be specified again.` + `Try removing all but one specification of the rule.`; `{0}` sem aspas.
- **Supressões e ordem:** severidade **INFO**, tipo HINT (`option_codes.g.dart:117-120`): não altera o código de saída sem `--fatal-infos`. `errors:` vale. Na forma de mapa não ocorre (chave duplicada é `parse_error`).
- **No DartForge:** não implementado. Casos: `linter:\n  rules:\n    - camel_case_types\n    - camel_case_types\n` → info em 46/16; a regra duplicada sendo uma removida → sai `duplicate_rule` **e** `removed_lint` (os blocos são independentes, `:90-105` e `:108-142`).

##### `incompatible_lint`
- **Emissão:** `validateRule` (`options_rule_validator.dart:90-97`).
- **Condição exata:** regra registrada e `enabled`; algum nome de `rule.incompatibleRules` já está em `seenRules` (pares no Mecanismo 3). A regra relatada não é adicionada a `seenRules` (logo uma terceira regra incompatível só com ela não é relatada, e repeti-la dá de novo `incompatible_lint`, não `duplicate_rule`).
- **Posição:** o nome da regra que aparece depois.
- **Mensagem:** `The rule '{0}' is incompatible with the rule '{1}'.` + `Try removing one of the incompatible rules.`; `{1}` = o primeiro incompatível, na ordem da lista `incompatibleRules` da regra.
- **Supressões e ordem:** WARNING; `errors:` vale; só dentro do mesmo arquivo (incompatibilidade com regra vinda de `include` não é vista).
- **No DartForge:** não implementado; precisa da tabela `incompatibleRules`. Casos: `linter:\n  rules:\n    - prefer_single_quotes\n    - prefer_double_quotes\n` → em 50/20, `{0}` = `prefer_double_quotes`, `{1}` = `prefer_single_quotes`; forma de mapa com `prefer_double_quotes: false` → nada.

##### `removed_lint` (e `replaced_lint`)
- **Emissão:** `validateRule` (`options_rule_validator.dart:125-141`).
- **Condição exata:** `sourceIsOptionsForContextRoot` (arquivo validado diretamente, `:108`); `rule.state` é `RemovedState`; `currentSdkAllows(state.since)` (`:35-40`, `:48-51`): `since == null`, ou a restrição `environment: sdk:` do pubspec do pacote existe e **contém** `since`. Sem `replacedBy` → `removed_lint`; com → `replaced_lint` (nenhuma no 3.6.2). Vale também para regra listada com `false` (o bloco não depende de `enabled`).
- **Posição:** o nome da regra.
- **Mensagem:** `'{0}' was removed in Dart '{1}'` (sem ponto final) + `Remove the reference to '{0}'.`; `{1}` = `Version.toString()` (`3.0.0`, `3.3.0`, `2.12.0`) ou `null`. `replaced_lint`: `'{0}' was replaced by '{2}' in Dart '{1}'.` + `Replace '{0}' with '{1}'.` (a correção usa `{1}`, a versão — defeito da fonte).
- **Supressões e ordem:** WARNING; `errors:` vale; nada em arquivos incluídos; depende do pubspec (sem `sdk:`, ou restrição que exclui a versão — caso de `^3.x` para as regras de 3.0.0 quando x ≥ 1 —, nada).
- **No DartForge:** não implementado; precisa da lista das 12 removidas com versão (Mecanismo 3) e do `sdk:` do pubspec como restrição de versão. Casos:
  1. pubspec `environment:\n  sdk: '>=2.19.0 <4.0.0'`; opções `linter:\n  rules:\n    - super_goes_last\n` → em 23/15: `'super_goes_last' was removed in Dart '3.0.0'`.
  2. mesmo arquivo com `sdk: ^3.6.0` → nada.
  3. `    - avoid_unstable_final_fields` com qualquer pubspec → `… was removed in Dart 'null'`.

##### `deprecated_lint` (e `deprecated_lint_with_replacement`)
- **Emissão:** `validateRule` (`options_rule_validator.dart:108-124`).
- **Condição exata:** arquivo direto; `rule.state` é `DeprecatedState`; `currentSdkAllows(state.since)`. No registro do 3.6.2 **nenhuma** regra tem esse estado (`grep -rn "State\.deprecated\|DeprecatedState(" linter/lib` vazio): o código não é emitido pelo `dart analyze` 3.6.2.
- **Posição:** o nome da regra.
- **Mensagem:** `'{0}' is a deprecated lint rule and should not be used.` + `Try removing '{0}'.`; com substituta: `'{0}' is deprecated and should be replaced by '{1}'.` + `Try replacing '{0}' with '{1}'.`
- **Supressões e ordem:** INFO/HINT.
- **No DartForge:** não implementado; para a paridade 3.6.2 basta nunca emitir. Caso: `- package_api_docs` (depreciada em versões posteriores) → **nada** no 3.6.2.

##### `unrecognized_error_code`
- **Emissão:** `ErrorFilterOptionValidator.validate` (`options.dart:599-648`) e `CannotIgnoreOptionValidator.validate` (`:355-400`), ambos dentro de `AnalyzerOptionsValidator`.
- **Condição exata:** `analyzer:` é mapa e:
  - `errors:` é mapa; a chave `k` é escalar; `k.value.toString().toUpperCase()` não está em `{code.name de errorCodeValues}` (`:584-585`; `analyzer/lib/src/error/error_code_values.g.dart:25`), nem em `{nome de lint registrado em maiúsculas}` (`:595-597`), nem em `{MISSING_RETURN}` (`:590-592`);
  - ou `cannot-ignore:` é lista; o item é `String`, não é `error`/`info`/`warning` (`:365-368`), e falha no mesmo teste (`:369-377`).
  O conjunto usa `name`, não `uniqueName`: códigos que compartilham nome são aceitos pelo nome compartilhado e recusados pelo `uniqueName` (ex.: `analysis_option_deprecated_with_replacement` é recusado).
- **Posição:** a chave (ou o item).
- **Mensagem:** `'{0}' isn't a recognized error code.` (sem correção); `{0}` como escrito (sem mudar a caixa).
- **Supressões e ordem:** WARNING; `errors: {unrecognized_error_code: ignore}` vale. Para a mesma entrada de `errors:`, o relato da chave precede o do valor (`unsupported_option_with_legal_values`).
- **No DartForge:** não implementado; `Opcoes::de_texto` aceita qualquer chave (`crates/paridade/src/filtros.rs:71-79`). Precisa do conjunto completo de nomes do 3.6.2 (todas as classes de `errorCodeValues`, inclusive as deste lote, mais os lints). Casos:
  1. `analyzer:\n  errors:\n    nao_existe: ignore\n    dead_code: banana\n` → `unrecognized_error_code` em 24/10 e `unsupported_option_with_legal_values` em 58/6.
  2. `    missing_return: error` → nada (lista de removidos).
  3. `    camel_case_types: ignore` (lint) → nada; `    todo: ignore` → nada.
  4. `analyzer:\n  cannot-ignore:\n    - error\n    - nao_existe\n` → só no item `nao_existe`.

##### `unsupported_option_with_legal_values`, `unsupported_option_with_legal_value`, `unsupported_option_without_values`
- **Emissão:** `TopLevelOptionValidator.validate` (`options.dart:976-995`), `ErrorBuilder.reportError` (`:554-568`), `ErrorFilterOptionValidator` (`:619-631`), `CodeStyleOptionsValidator` (`:413-419`), `EnabledExperimentsValidator` (`:493-498`); pseudocódigo na seção dos validadores.
- **Condição exata:**
  - `_with_legal_values`: (a) chave escalar de `analyzer:` fora das 8 suportadas; (b) chave de `analyzer: language:` fora de `strict-casts`, `strict-inference`, `strict-raw-types`; (c) chave de `analyzer: strong-mode:` fora de `declaration-casts`, `implicit-casts`, `implicit-dynamic`; (d) `analyzer: optional-checks:` escalar diferente de `chrome-os-manifest-checks`, ou chave de mapa diferente dela (inclusive `propagate-linter-exceptions`, que consta da proposta mas é recusada: `:725`, `:734`); (e) valor escalar de uma entrada de `errors:` cujo minúsculo não está em `ignore, false, include, true, error, info, warning` (`:574-577`).
  - `_with_legal_value`: chave escalar de `linter:` diferente de `rules`.
  - `_without_values`: chave de `code-style:` diferente de `format`; item de `enable-experiment` que `validateFlags` não reconhece (`UnrecognizedFlag`).
- **Posição:** a chave; em (d) escalar e (e), o valor; em `enable-experiment`, o item.
- **Mensagem:**
  - `_with_legal_values`: `The option '{1}' isn't supported by '{0}'.` + `Try using one of the supported options: {2}.`; `{0}` = `analyzer`, `language`, `strong-mode`, `errors` ou — em (d) — `chrome-os-manifest-checks` (o escopo passado é a própria opção, `:726-727`, `:735-736`); `{2}`: para `analyzer`, `'cannot-ignore', 'enable-experiment', 'errors', 'exclude', 'language', 'optional-checks', 'plugins', and 'strong-mode'`; `language`: `'strict-casts', 'strict-inference', and 'strict-raw-types'`; `strong-mode`: `'declaration-casts', 'implicit-casts', and 'implicit-dynamic'`; optional-checks: `'chrome-os-manifest-checks' and 'propagate-linter-exceptions'`; `errors`: `'ignore', 'false', 'include', 'true', 'error', 'info', and 'warning'`.
  - `_with_legal_value`: `The option '{1}' isn't supported by '{0}'. Try using the only supported option: '{2}'.` (tudo na mensagem, sem correção); `{2}` já vem entre aspas da proposta (`'rules'`), logo o texto final tem aspas duplicadas `''rules''`.
  - `_without_values`: `The option '{1}' isn't supported by '{0}'.`; `{0}` = `code-style` ou `enable-experiment`.
- **Supressões e ordem:** WARNING; `errors:` vale. Chaves não escalares são ignoradas (`:981`, TODO `:990`). Seção `analyzer:`/`linter:` que não é mapa: nada.
- **No DartForge:** não implementado. Casos:
  1. `analyzer:\n  strong_mode: true\n` → `_with_legal_values` em 12/11, `{0}` = `analyzer`.
  2. `linter:\n  rule:\n    - a\n` → `_with_legal_value` em 10/4 (e nenhum `undefined_lint`: `rules` não existe).
  3. `analyzer:\n  language:\n    strict-cast: true\n` → `_with_legal_values` na chave `strict-cast` (26/11), `{0}` = `language`.
  4. `analyzer:\n  enable-experiment:\n    - nao-existe\n` → `_without_values` em 37/10, `{0}` = `enable-experiment`, `{1}` = `nao-existe`.

##### `unsupported_value`
- **Emissão:** `LanguageOptionValidator` (`options.dart:673-687`), `StrongModeOptionValueValidator` (`:922-946`), `OptionalChecksValueValidator` (`:737-749`), `CodeStyleOptionsValidator._validateFormat` (`:443-455`).
- **Condição exata:** chave válida e valor escalar cujo `toString().toLowerCase()` não é `true` nem `false`; para `strong-mode: declaration-casts`, sempre (qualquer valor). Valor nulo (chave sem valor): a comparação falha e `v.valueOrThrow` lança → arquivo sem diagnósticos (Mecanismo 1). Valor mapa/lista para chave válida de `language`/`strong-mode`: nada.
- **Posição:** o valor.
- **Mensagem:** `The value '{1}' isn't supported by '{0}'.` + `Try using one of the supported options: {2}.`; `{0}` = a chave (`strict-casts`…), exceto `declaration-casts` → `strong-mode`, e `format`; `{1}` = o valor; `{2}` = `'true' and 'false'`.
- **Supressões e ordem:** WARNING; `errors:` vale.
- **No DartForge:** não implementado. Casos: `analyzer:\n  language:\n    strict-casts: yes\n` → em 40/3, `{0}` = `strict-casts`, `{1}` = `yes`; `strict-casts: TRUE` → nada; `strong-mode:\n    implicit-casts: false` → nada (aceito sem aviso no 3.6.2).

##### `invalid_option`
- **Emissão:** `EnabledExperimentsValidator.validate` (`options.dart:499-507`).
- **Condição exata:** `analyzer: enable-experiment:` é lista; `validateFlags(textos dos itens)` (`analyzer/lib/src/dart/analysis/experiments_impl.dart:191-220`) devolve, para o item, um resultado que não é `UnrecognizedFlag`: feature **expirada** (`UnnecessaryUseOfExpiredFlag` se o valor pedido é o padrão, `IllegalUseOfExpiredFlag` se é o contrário) ou **conflito** com flag anterior da mesma feature (`a` e `no-a`). O `isError` do resultado é ignorado.
- **Posição:** o item da lista.
- **Mensagem:** `Invalid option specified for '{0}': {1}`; `{0}` = `enable-experiment`; `{1}` = `Flag "x" is no longer required.` (`experiments_impl.dart:417`), `Flag "x" was supplied, but the feature is already unconditionally enabled.`/`…disabled.` (`:393-397`), ou `Flag "x" conflicts with previous flag "y"` (`:289-292`, sem ponto).
- **Supressões e ordem:** WARNING; `errors:` vale.
- **No DartForge:** não implementado; precisa da tabela de experimentos do 3.6.2 (nome, expirado, padrão; `analyzer/lib/src/dart/analysis/experiments.g.dart`). Casos: `analyzer:\n  enable-experiment:\n    - non-nullable\n` → em 37/12: `Invalid option specified for 'enable-experiment': Flag "non-nullable" is no longer required.`; `    - no-non-nullable` → `… was supplied, but the feature is already unconditionally enabled.`

##### `invalid_section_format`
- **Emissão:** 13 sítios em `options.dart` (lista na tabela).
- **Condição exata:** (pseudocódigo dos validadores)
  - `cannot-ignore`: não é lista e não é nulo → no valor; item não-`String` → no item;
  - `code-style`: escalar não nulo ou lista → no valor; `format` mapa ou lista → no valor de `format`;
  - `enable-experiment`: não é lista e não é nulo;
  - `errors`: não é mapa e não é nulo; ou valor de uma entrada não escalar (mapa/lista);
  - `language`: escalar não nulo ou lista;
  - `optional-checks`: não é escalar, nem mapa, nem nulo (lista);
  - `strong-mode`: não é mapa e não é nulo (inclusive `strong-mode: true`).
- **Posição:** o nó ofensivo.
- **Mensagem:** `Invalid format for the '{0}' section.` (sem correção); `{0}` = nome da seção, **mas `enable-experiment`** para os relatos de `errors` (`:636`, `:644`) e de `optional-checks` (`:757`).
- **Supressões e ordem:** WARNING; `errors:` vale (se a seção `errors:` estiver bem formada).
- **No DartForge:** não implementado. Casos: `analyzer:\n  errors: foo\n` → em 20/3: `Invalid format for the 'enable-experiment' section.`; `analyzer:\n  language: foo\n` → em 22/3, `{0}` = `language`; `analyzer:\n  strong-mode: true\n` → no valor `true`, `{0}` = `strong-mode`.

##### `analysis_option_deprecated`
- **Emissão:** **sem emissor no 3.6.2** (as duas constantes; evidência na tabela). O produtor de correção `analysis_server/lib/src/services/correction/fix/analysis_options/fix_generator.dart:30`, `:85` só reage a um erro que nunca chega.
- **Condição exata / Posição / Mensagem:** não se aplicam; moldes `The option '{0}' is no longer supported.` (+ `Try using the new '{1}' option.` na variante).
- **Supressões e ordem:** —.
- **No DartForge:** nada a emitir; `analysis_option_deprecated` deve constar como nome válido em `errors:`/`cannot-ignore:`. Caso: `analyzer:\n  strong-mode:\n    implicit-casts: false\n` → nenhum diagnóstico.

#### Resumo do lote N

| estado | constantes |
|---|---:|
| publicado | 0 |
| emitido, não publicado | 0 |
| não implementado | 53 |
| **total** | **53** |

Das 53: 2 sem emissor no 3.6.2 (`ANALYSIS_OPTION_DEPRECATED`,
`ANALYSIS_OPTION_DEPRECATED_WITH_REPLACEMENT`), 1 emitida só pelo `dart fix`
(`MISSING_DEPENDENCY`), 3 inalcançáveis com o registro de lints do 3.6.2
(`DEPRECATED_LINT`, `DEPRECATED_LINT_WITH_REPLACEMENT`, `REPLACED_LINT`), 7 só
com `optional-checks: chrome-os-manifest-checks` (manifest). Especificações
completas: 30 códigos em 25 blocos.

Não verificado:

- Nenhum caso de teste foi executado contra o `dart` 3.6.2: offsets, textos e
  ausências vêm da leitura da fonte.
- `package:yaml` na revisão fixada pelo `DEPS` do 3.6.2 (`yaml_rev e773005a…`):
  lido na cópia `yaml-3.1.3` do pub cache. Em particular: fim do span de
  mapas/listas de bloco (token `BLOCK-END`); textos das `YamlException` além de
  `Duplicate mapping key.`; span exato dessa exceção.
- `include:` entre aspas (`includeSpan.text` com as aspas): o resultado
  (`include_file_not_found`) foi deduzido, não executado; idem `include:` com
  lista/mapa.
- Span do elemento `manifest` (`no_touchscreen_feature`) para elemento com
  filhos; o parser XML do manifest não foi lido por inteiro.
- `asset_directory_does_not_exist`: efeito do segmento vazio final em
  `joinAll(posix.split('img/'))`.
- `undefined_lint` com item não escalar (texto de `{0}`); ausência de nomes de
  lint pós-3.6.2 no registro (só a contagem de 240 `..register(` foi conferida).
- Como o `bulk_fix_processor` monta `usedDeps`/`usedDevDeps`
  (`missing_dependency`).
- Se um `analysis_options.yaml` aninhado cria contexto próprio no 3.6.2 (afeta
  o `{2}` de `include_file_not_found` e o `analysisOptions` usado em `errors:`).
- Se o `yaml-rust2` de `crates/build` expõe posições por nó.
- `analyzer_plugin/lib/utilities/analyzer_converter.dart` foi lido por
  `git show 3.6.2:…` (fora da extração `dart-sdk-3.6.2`).

---

# Parte III — Plano de implementação no DartForge

Objetivo: a saída do `dartforge analyze <alvo>` de um projeto inteiro ser
**idêntica, byte a byte**, à do `dart analyze <alvo>` do SDK 3.6.2 nos três
formatos (§5.4–§5.6) e no código de saída (§5.7). O plano parte só do que as
seções "No DartForge" da Parte I registraram (§1.5, §2.5, §3.7, §4.7, §5.8,
§6.5, §7.3, §8.6) e do catálogo da Parte II. Nada aqui é estimativa de
ganho; cada etapa tem critério de pronto verificável.

## III.0 Ordem e dependências

| etapa | o quê | depende de | muda o placar do corpus? |
|---|---|---|---|
| 1 | harness de paridade de **projeto** (saída inteira, três formatos) | — | não (instrumento) |
| 2 | supressões: `// ignore`, `IgnoreValidator`, `analysis_options.yaml` | 1 | não |
| 3 | relato: deduplicação por arquivo, `contextMessages`, sufixo `(where …)` | — | sim (mensagens) |
| 4 | saída do CLI: cabeçalho, caminho relativo, formatos, opções, código de saída | 1, 3 | não |
| 5 | pipeline: unidade de análise por biblioteca e retirada das portas de sintaxe | T5 do outro documento | sim |
| 6 | fases ausentes: `BestPracticesVerifier` e sub-verificadores, `OverrideVerifier`, `RedeclareVerifier`, Unicode, `SdkConstraintVerifier`, `TodoFinder` | 5 | não (códigos fora dos 435) |
| 7 | arquivos não-Dart: `analysis_options.yaml`, `pubspec.yaml`, `AndroidManifest.xml` | 2 | não |
| 8 | lints: validação da lista de regras; depois as regras por frequência | 2, 7 | não |
| 9 | fim do filtro de publicação | todas | — |

As etapas 2, 3 e 4 são independentes entre si depois da 1 e podem ser feitas
em qualquer ordem. A 5 é a única que mexe em `crates/paridade/src/analise.rs`
de forma estrutural; fazê-la antes da 6 evita portar cada verificador novo
duas vezes.

## III.1 Etapa 1 — Harness de paridade de projeto

Hoje o placar (`crates/paridade`) compara **registros** (código, arquivo,
offset, mensagem) contra um oráculo gravado. Falta comparar a **saída do
comando**.

1. Comando novo `dartforge-paridade saida <dir-de-casos>`: cada caso é uma
   pasta com um projeto mínimo (`pubspec.yaml`, `.dart_tool/package_config.json`
   gravado, `analysis_options.yaml` opcional, fontes) e um arquivo
   `ESPERADO.<formato>.txt` por formato (`default`, `json`, `machine`), mais
   `ESPERADO.codigo` (o código de saída) e `ARGS` (os argumentos, um por linha,
   relativos à pasta).
2. Gravação do oráculo: `dartforge-paridade saida --gravar` roda
   `dart analyze` do 3.6.2 com `ANALYZER_STATE_LOCATION_OVERRIDE` numa pasta
   temporária de E:, com stdout **redirecionado para arquivo** (sem terminal: é
   o modo sem ANSI da §5.4), e normaliza só o que depende da máquina: o
   caminho absoluto da pasta do caso vira `<raiz>` (o JSON e o `machine`
   trazem caminhos absolutos, §5.5 e §5.6).
3. Comparação: bytes iguais depois da mesma normalização do nosso lado. O
   relatório lista, por caso e por formato, a primeira linha diferente.
4. Casos iniciais: as sondas `s1`–`s7` de
   `E:\dftemp\analise\spec-infra\sonda\` (copiá-las para
   `corpus/saida-analyze/`), que já têm a saída real nos três formatos.

**Pronto quando** o comando roda no CI (job `analise` do `pesado.yml`, sem
SDK: contra o esperado gravado) e reprova qualquer diferença de byte.

## III.2 Etapa 2 — Supressões

**Estado em 2026-10-04 (escrito, não compilado).** Feitos III.2.1 e III.2.3 em
`crates/paridade/src/filtros.rs`: `Ignorados::de_texto` foi reescrito sobre uma varredura da fonte que
conhece strings, interpolações e comentários de bloco (o lexer não guarda comentários), com a regra de
prefixo (`//+[ ]*ignore:` e `//[ ]*ignore_for_file:`), a ordem dos testes de `forDart`, a linha alvo,
o leitor de nomes de `ignoredElements`, o casamento pelo nome emitido ou pelo nome único sem a classe,
e `type=` só com `hint`, `lint` e `warning`; `Ignorados::duplicados` dá os `duplicate_ignore`, ligados
em `publicaveis` (`crates/paridade/src/lib.rs`). Os testes novos são a tabela do oráculo da §4.1, não
rodados. O `IgnoreValidator` ficou em `filtros.rs`, não num módulo de `crates/analise`. Não feitos:
III.2.2 (a regra de `errors:` × erro ignorável, que já existia em parte em `Opcoes::ignoravel`), III.2.4
(leitor de YAML de verdade, `include:`, opções por subpasta, glob do `package:glob`) e os casos do
harness de III.2.5. As demais etapas da Parte III não foram começadas.

Arquivos: `crates/paridade/src/filtros.rs` (reescrita das três estruturas),
`crates/analise` (um módulo novo `ignores.rs` para o `IgnoreValidator`).

### III.2.1 `IgnoreInfo` sobre tokens

Substituir `Ignorados::de_texto` (`filtros.rs:168-195`) por uma leitura dos
**comentários do lexer**, não das linhas de texto:

```rust
pub struct IgnoreInfo {
    /// linha (1-based) → nomes ignorados naquela linha
    por_linha: BTreeMap<u32, Vec<Ignorado>>,
    /// `ignore_for_file`
    no_arquivo: Vec<Ignorado>,
}
pub enum Ignorado {
    /// nome minúsculo do código; `offset` é o do nome dentro do comentário
    Codigo { nome: Box<str>, offset: u32 },
    /// `type=lint|hint|warning`
    Tipo { tipo: TipoIgnorado, offset: u32, length: u32 },
}
```

Regras a implementar, todas da §4.1 (as linhas da fonte estão lá):
1. só comentários `//` de linha entram; `///` e `/* */` não;
2. o prefixo é reconhecido pela expressão regular do `IgnoreInfo` (espaços
   opcionais entre `//` e `ignore`, `:` obrigatório);
3. comentário no fim de uma linha com código vale para **aquela** linha;
   comentário sozinho na linha vale para a **linha seguinte**;
4. o analisador de nomes para no primeiro caractere que não é de
   identificador nem vírgula nem espaço, sem incluir o pedaço estranho;
5. `type=` aceita só `lint`, `hint` e `warning` (hoje está invertido para
   `warning`/`static_warning`, §4.7);
6. o casamento é pelo **nome emitido** e também pelo `uniqueName` sem o
   prefixo da classe.

### III.2.2 Aplicação

`Opcoes::ignoravel` (`filtros.rs:95-114`) passa a seguir a §4.3: um erro
(`ERROR` de tipo que não é lint) não é ignorável por comentário, **salvo** se
o `errors:` o rebaixou; `cannot-ignore:` proíbe por nome ou por severidade.
A ordem é: (1) `errors:` muda a severidade ou descarta (`ignore`/`false`);
(2) comentário `ignore` descarta se permitido; (3) o que sobra é publicado.

### III.2.3 `IgnoreValidator`

Módulo novo, chamado depois de todos os verificadores da unidade (é a fase C7
da §1.2). No 3.6.2 ele só relata `duplicate_ignore` (os relatos de
`unnecessary_ignore` e `unignorable_ignore` estão comentados na fonte, lote
II.8): nome repetido no mesmo comentário, ou nome de linha já coberto por
`ignore_for_file`. Posição: o nome repetido (`offset` guardado em `Ignorado`).

### III.2.4 `analysis_options.yaml`

1. Trocar o leitor por indentação (`de_texto`, `filtros.rs:40-90`) por um
   leitor de YAML de verdade. O workspace já tem o `yaml-rust2` 0.10
   (`crates/build/Cargo.toml:25`); a exigência a conferir nele é ter
   **posições** (offset e length de cada escalar; o analisador de eventos com
   marcadores serve, a árvore `Yaml` pronta não guarda posição — **não
   verificado** na versão 0.10), porque a etapa 7 relata diagnósticos dentro
   do arquivo.
2. `include:` com `package:` resolvido pelo `package_config.json`
   (`crates/elements/src/config.rs:154-230`) e caminho relativo; fusão na
   ordem da §4.6 (o arquivo que inclui vence; listas de `exclude` e mapas de
   `errors` se fundem chave a chave).
3. Opções por subpasta: o arquivo de opções mais próximo acima de cada
   arquivo analisado (§4.5), não só o da raiz.
4. `exclude:` com o glob do `package:glob` (chaves, classes de caractere,
   escape), relativo à pasta do arquivo de opções; a regra "`x/**` também
   exclui `x`"; o arquivo pedido explicitamente na linha de comando não é
   excluído. Na varredura (`crates/paridade/src/corpus.rs:60-80`): **não**
   pular `build` e **pular** arquivos começados por `.`.
5. `language:` (`strict-casts`, `strict-inference`, `strict-raw-types`) e
   `enable-experiment`: entregues ao motor como opções da análise; os códigos
   que cada uma liga estão na §4.6.

**Estado em 2026-10-04 (escrito, não compilado).** `Opcoes` em `crates/paridade/src/filtros.rs` foi
reescrito sobre o `yaml-rust2` 0.10 (dependência nova de `crates/paridade`): item 1 pela árvore `Yaml`,
**sem posições** (a etapa 7 vai precisar do analisador de eventos); item 2 com `include:` escalar,
relativo ou `package:` (pelo `package_config.json` achado acima do arquivo), fusão pelo `Merger` e corte
de ciclo; item 3 em `diagnosticos_json` (`crates/paridade/src/lib.rs`): o arquivo de opções mais próximo
abaixo da raiz vale para o arquivo e soma os seus `exclude`; o LSP e o placar seguem com o da raiz; item
4 com chaves, classes e escape no glob, a regra do `x/**`, os componentes começados por `.` em
`excluido`, e a varredura de `corpus::arquivos_dart` (entra `build`, saem os arquivos ocultos); item 5
só **lido** (`strict_casts`, `strict_inference`, `strict_raw_types`, `experimentos`, `regras`): o motor
ainda não consome nenhum deles. A condição "o glob não casa com o caminho incluído" do `_isExcluded`
não foi escrita.

### III.2.5 Testes dirigidos (cada um é um caso da etapa 1)

| caso | entrada | esperado do oráculo |
|---|---|---|
| ig-01 | `'http://x'; // ignore: unused_local_variable` numa declaração de local não usado | suprimido (o `//` da string não conta) |
| ig-02 | `/// ignore: unused_local_variable` na linha anterior | **não** suprimido |
| ig-03 | `// ignore: a, unused_local_variable` | suprimido; sem `duplicate_ignore` |
| ig-04 | `// ignore: unused_local_variable, unused_local_variable` | suprimido + `duplicate_ignore` no segundo nome |
| ig-05 | `// ignore_for_file: x` + `// ignore: x` | `duplicate_ignore` no da linha |
| ig-06 | `// ignore: undefined_identifier` sobre um erro | **não** suprimido |
| ig-07 | o mesmo com `errors: undefined_identifier: warning` | suprimido |
| ig-08 | `// ignore: type=lint` e `// ignore: type=warning` | por tipo, §4.1 |
| ao-01 | `errors: unused_import: error` | severidade `error`, código de saída 3 |
| ao-02 | `errors: unused_import: false` | descartado |
| ao-03 | `exclude: [lib/gen/**]` + erro em `lib/gen/a.dart` | ausente |
| ao-04 | o mesmo, com `dart analyze lib/gen/a.dart` | presente |
| ao-05 | `include: package:x/opts.yaml` que rebaixa um código | rebaixado |
| ao-06 | `analysis_options.yaml` em `lib/sub/` com `errors:` próprio | vale só para `lib/sub/` |

Os esperados são gravados pela etapa 1; a tabela só fixa o que cada caso
exercita. **Pronto quando** os 14 casos passam nos três formatos.

## III.3 Etapa 3 — Relato

**Estado em 2026-10-04 (escrito, não compilado).** Feito o item 2 em parte: `Contexto` e
`Diagnostic::contexto` (`crates/diagnostics/src/lib.rs`, com `com_contexto`), a chave `contextMessages`
do JSON (`crates/paridade/src/json.rs`, com a linha e a coluna calculadas pelo arquivo do erro, como o
oficial) e as linhas de contexto do formato de texto (`crates/cli/src/analisar.rs`). O único produtor
por enquanto é o `duplicate_definition` de `crates/analise/src/duplicatas.rs` ("The first definition of
this name."), e só quando a primeira definição está no mesmo arquivo. O item 1 (deduplicação por
arquivo) já existia e passou a usar o código único (T5 do outro documento); o sufixo `(where …)` do
item 3 está em `crates/types/src/exibicao.rs` (T7), sem gerar a mensagem de contexto por tipo. Não
feitos: os demais produtores de contexto e o `intervalo_de_relato` do item 4.

Arquivos: `crates/diagnostics/src/lib.rs` (o `Diagnostic`),
`crates/paridade/src/json.rs`, e os emissores que usam a fábrica.

1. **Deduplicação por arquivo.** Um conjunto por arquivo com a chave da §3.5
   (constante, offset, length, mensagem **já renderizada**), aplicado no ponto
   único por onde todo diagnóstico passa antes da publicação
   (`Motor::analisar_com`, `crates/paridade/src/analise.rs:173-660`, na coleta
   final), inclusive entre sintaxe e semântica. O primeiro relato vence e a
   ordem de chegada é a das fases da §1.2 — por isso a etapa 5 importa.
2. **`contextMessages`.** Campo novo:

   ```rust
   pub struct Contexto { pub arquivo: FileId, pub span: Span, pub mensagem: Box<str> }
   // em Diagnostic:
   pub contexto: Vec<Contexto>,
   ```

   Produtores: os que no oficial passam por `DiagnosticFactory` (lista na
   §3.2: duplicatas com "The first definition of this name", referência antes
   da declaração, conflitos de assinatura) e o `_convertTypeNames` (item 3).
   Saída: as linhas `           - <mensagem> at <rel>:<linha>:<coluna>.` do
   formato padrão (§5.4) e o vetor `contextMessages` do JSON (§5.5).
3. **Sufixo `(where X is defined in <caminho>)`.** Em `Exibidor::argumentos`
   (o formatador único do T7 do outro documento): quando dois argumentos de
   tipo distintos têm o mesmo texto, cada um ganha o sufixo com o caminho do
   arquivo da declaração e um `contextMessage` por tipo (§3.4).
4. **`atElement`.** Uma função única no modelo de elementos,
   `intervalo_de_relato(elemento) -> Span`, com a tabela `nonSynthetic` da
   §3.2 (campo sintético de getter → o getter; construtor sem nome → o nome da
   classe; parâmetro de função-tipo → …), usada por todo verificador que hoje
   escolhe o intervalo à mão.

Testes: um por linha da tabela da §3.2 (o elemento, o intervalo esperado) e
os dois casos de tipos homônimos do T7. **Pronto quando** o placar não tem
nenhuma amostra de `mensagem` cuja diferença seja o sufixo `(where …)`, e os
casos de contexto da etapa 1 (`s1` tem um) saem iguais nos três formatos.

## III.4 Etapa 4 — Saída do CLI

**Estado em 2026-10-04 (escrito, não compilado).** `crates/cli/src/analisar.rs` foi reescrito: itens 1
(opções: `--format=machine`, `--fatal-infos`, `--[no-]fatal-warnings`, `--packages=`, vários alvos,
alvo inexistente com a mensagem do oficial; `--sdk-path=`, `--enable-experiment=`, `--cache=` e
`--memory` são aceitas e não têm efeito — o experimento desconhecido não é recusado), 2 (cabeçalho com
o basename como digitado e sem a linha em branco quando não há diagnósticos), 3 (caminho relativo ao
alvo, só se não for mais longo; só por prefixo, sem `..`), 4 (ordenação por severidade, caminho em
unidades UTF-16, offset e mensagem), 5 em parte (JSON com `
` final; `contextMessages` não, porque o
`Diagnostic` ainda não tem o campo — etapa 3), 6 (código de saída 0, 1, 2 e 3) e 7 (o ponto de extensão
do bloco prioritário está marcado). O formato `machine` tem o escape de `_escapeForMachineMode`. Os
`TODO` de severidade `INFO` são descartados. Dois testes de unidade no arquivo, não rodados.

Arquivo: `crates/cli/src/analisar.rs` (113 linhas hoje). Cada item é uma
linha da tabela da §5.8:

1. Opções: `--format=machine`, `--fatal-infos`, `--[no-]fatal-warnings`,
   `--packages`, `--sdk-path`, `--enable-experiment` (experimento desconhecido
   → erro de uso), **vários alvos**, alvo inexistente →
   `Directory or file doesn't exist: <alvo>`.
2. Cabeçalho: `Analyzing <basename como digitado>...`; com vários alvos, a
   lista da §5.4. A linha em branco pertence ao bloco de erros: com zero
   diagnósticos sai `Analyzing x...` e logo `No issues found!`.
3. Caminho relativo ao **alvo** (pasta alvo, ou pasta do arquivo alvo), usado
   só se não for mais longo que o absoluto.
4. Ordenação pela §5.3: severidade processada, caminho absoluto **canônico**
   comparado por unidades UTF-16, offset, mensagem. Empates além disso são
   indefinidos no oficial; manter a ordem de chegada.
5. JSON com `\n` final e `contextMessages`; conferir o escape de caracteres
   de controle contra o `json.encode` do Dart com um caso dirigido (mensagem
   com aspas, barra invertida e um caractere acima de U+FFFF em nome de
   arquivo).
6. Código de saída pela §5.7 (0, 1, 2, 3; 4 fica reservado a pânico do
   analisador).
7. Bloco prioritário (arquivos não-Dart primeiro, §5.4): só passa a existir
   com a etapa 7; deixar o ponto de extensão.

Decisão registrada: paridade só no modo **redirecionado** (sem ANSI, sem
quebra por largura de terminal). Com terminal, o DartForge pode colorir, mas
isso não é medido.

**Pronto quando** `s1`–`s7` passam nos três formatos e no código de saída.

## III.5 Etapa 5 — Pipeline

Arquivo: `crates/paridade/src/analise.rs`.

1. **Unidade de análise = biblioteca.** Manter a carga em lote (é o que dá a
   velocidade), mas rodar as fases **por biblioteca, na ordem da §1.2**, com
   um coletor por arquivo. Uma parte sem biblioteca conhecida é analisada
   como biblioteca própria (hoje é pulada: `e_parte`, `analise.rs:64`).
2. **Portas de sintaxe.** Retirar `libs_com_erro_de_sintaxe`
   (`analise.rs:300`, `:349-352`, `:525`) e `recuperacao_do_parser` (`:725`)
   verificador por verificador, na ordem do T5 do outro documento: cada
   retirada é medida no placar e só fica se não criar FP em código publicado.
   O que impede a retirada de uma vez é a árvore de recuperação do parser
   (família E): onde ela difere da do fasta, o verificador vê nós que o
   oficial não vê.
3. **Ordem de emissão.** A lista final por arquivo sai na ordem das fases:
   scanner, parser, diretivas, resolução (com o fluxo), constantes, herança,
   `ErrorVerifier`, FFI, duplicatas de membros, campos de construtor, e os
   avisos na ordem C4a–C4j (§1.2). Com a deduplicação da etapa 3, é essa
   ordem que decide qual de dois relatos iguais sobrevive.

**Pronto quando** o placar do corpus não regride em nenhum código publicado e
as amostras de FN atribuídas a "porta de sintaxe" no T5 viram acerto.

## III.6 Etapa 6 — Fases ausentes

**Estado em 2026-10-04 (escrito, não compilado).** Entraram, em `crates/analise/src/fases.rs` e ligados
em `crates/paridade/src/analise.rs`: do `ImportsVerifier`, `duplicate_import`, `duplicate_export`,
`duplicate_shown_name` e `duplicate_hidden_name` (com `_duplicates` e
`areSyntacticallyIdenticalExceptUri` literais); `undefined_hidden_name`; e o `UnicodeTextVerifier`
(`text_direction_code_point_in_comment` e `…_in_literal`; "literal" é o trecho de texto de uma string,
fora das expressões interpoladas, mais as URIs das diretivas). `undefined_shown_name` está em
`importacoes.rs`. O `OverrideVerifier` (`override_on_non_overriding_member`, as quatro variantes) está em
`crates/types/src/fase_override.rs`, lido da fonte oficial, com "sobrescreve" = algum supertipo
transitivo tem membro de instância com a mesma chave, visível da biblioteca. Não feitos:
`BestPracticesVerifier`
e `AnnotationVerifier`. O `TodoFinder` (`todo`, `fixme`, `hack`, `undone`) está em
`crates/analise/src/todos.rs`, com a expressão `Todo.TODO_REGEX` escrita à mão, as continuações de
linha e o desdobramento do bloco; os comentários são os dos vãos entre os tokens do lexer. O
`MustCallSuperVerifier` (`must_call_super`) está em `crates/types/src/fase_super.rs`: a busca em
largura pelo membro anotado (mixins, superclasse, restrições `on`), a implementação concreta herdada
e `invokesSuperSelf` procurado na árvore do corpo; a anotação é reconhecida só pelo nome
`mustCallSuper`, sem conferir o `package:meta`. O `RedeclareVerifier`
(`redeclare_on_non_redeclaring_member`) está em `fase_override.rs` (`sem_redeclaracao`): redeclara
quem tem a mesma chave num membro de instância de alguma superinterface de `implements`, transitiva. O
`UseResultVerifier` (`unused_result`, com e sem mensagem) está em `crates/types/src/fase_resultado.rs`,
escrito com o emissor da 6.11.0 aberto: desce dos contextos que não usam o valor (comando de expressão,
inicialização e atualizações de `for`, seções de cascata, operando de `is`, valor de `switch`
expressão) através de parênteses, `await`, `as`, `?:`, `!` e prefixos; `unless` só por argumento
nomeado; a anotação é reconhecida pelo nome. O `DeprecatedMemberUseVerifier` (as quatro constantes de
`deprecated_member_use*`) está em `crates/types/src/fase_deprecado.rs`, também com o emissor aberto:
identificadores e propriedades resolvidos, construtores, tipos nomeados e imports ou exports de
biblioteca depreciada, com a pilha de declarações depreciadas e a pontuação da mensagem; "mesmo
pacote" é a biblioteca cujo arquivo fica dentro da raiz analisada. Fora: operadores, argumentos de
parâmetros depreciados, nomes de `show`, campos de padrão, `extension override`, `call` implícito e
`expires`. O `unnecessary_import` está em `crates/analise/src/importacoes.rs`, junto do
`unused_import`, com os "elementos usados" aproximados pelos nomes citados (como o `unused_import` de
lá) e as extensões tratadas à parte. O `SdkConstraintVerifier` (`sdk_version_since` e
`sdk_version_gt_gt_gt_operator`) está em `crates/types/src/fase_sdk.rs`, com o emissor aberto: a
restrição vem do `environment: sdk:` do `pubspec.yaml` da raiz (`^v`, faixas, `any`, versão exata;
uniões não) e o `@Since` do elemento do SDK ou da classe que o contém. Fora: argumentos posicionais de
parâmetros com `@Since`, operadores de índice e de atribuição, e a propriedade `index`.

**Estado em 2026-10-04 (escrito, não compilado), lote II.7.** Em `crates/analise/src/meta.rs`, com os
emissores abertos: do `AnnotationVerifier`, `invalid_factory_method_decl`, `invalid_factory_method_impl`,
`invalid_internal_annotation`, `invalid_literal_annotation`, `invalid_non_virtual_annotation`,
`invalid_reopen_annotation`, `invalid_annotation_target` (só o caso de `@redeclare`),
`undefined_referenced_parameter`, `invalid_visibility_annotation`,
`invalid_visible_for_overriding_annotation` e `invalid_visible_outside_template_annotation`; do
`BestPracticesVerifier`, os três `invalid_required_*_param`,
`import_deferred_library_with_load_function`, `must_be_immutable` e
`invalid_override_of_non_virtual_member`. As anotações são reconhecidas pelo nome; a criação sem `new`
no `@factory` é reconhecida pela inicial maiúscula; o membro herdado do `@nonVirtual` é o primeiro pela
cadeia de mixins e superclasses. Em `crates/types/src/fase_acesso.rs`: os cinco `invalid_use_of_*`
(interno, protegido, de template, de teste e de sobrescrita) em identificadores, propriedades,
construtores e tipos nomeados, e `subtype_of_sealed_class` com `mixin_on_sealed_class` (posição: a
declaração sem o comentário de documentação); fora dali os operadores, o import de biblioteca
interna, os campos de padrão e o `@doNotSubmit`. O `invalid_annotation_target` geral (`_checkKinds`
e `_isValidTarget`) entrou em `meta.rs` em 2026-10-05: lê o `@Target({...})` da classe da anotação
(resolvida pelo escopo da unidade), sem visitar parâmetros nem parâmetros de tipo. Faltam do lote:
`non_const_call_to_literal_constructor` (os dois `invalid_export_of_internal_element*`
entraram em `meta.rs` em 2026-10-05, sem os limites dos parâmetros de tipo no caso indireto),
`assignment_of_do_not_store`, `return_of_do_not_store`, os dois `inference_failure_on_*` e
`deprecated_colon_for_default_value`.

**Estado em 2026-10-05 (escrito, não compilado), o resto do lote II.7.** Em `crates/types/`, ligados
em `crates/paridade/src/analise.rs` com os tipos da inferência: `fase_nao_guardar.rs`
(`assignment_of_do_not_store` nos inicializadores de topo e de campo; `return_of_do_not_store` nos
`return e;` e `=> e` com a declaração de função ou método que os contém, fora de membro, classe ou
biblioteca `@doNotStore`; nada num diretório `test`), `fase_literal.rs`
(`non_const_call_to_literal_constructor` e a variante `_USING_NEW`) e `fase_estrita.rs`
(`inference_failure_on_untyped_parameter` e `inference_failure_on_function_return_type`; o motor os
relata sempre e `publicaveis`, o caminho do JSON e do LSP, os tira sem `strict-inference: true`).
`deprecated_colon_for_default_value` já estava em `fases.rs`.

**Estado em 2026-10-05 (escrito, não compilado), sem aproximações no lote II.7.** As aproximações
da primeira escrita saíram; cada regra usa o mesmo dado que o emissor do analyzer:

- Anotações do `package:meta` (`@literal`, `@doNotStore`, `@required`…): resolvidas pelo escopo da
  unidade em que estão escritas, com ou sem prefixo, até a variável de topo de uma biblioteca
  `package:meta/` (`fase_resultado::anotacao_do_meta`); uma constante própria de mesmo nome não conta.
- `canBeConst` (`non_const_call_to_literal_constructor`): o construtor `const` e a verificação de
  constantes da criação como se ela tivesse `const`, sem nenhum dos 34 erros que o
  `_ConstantAnalysisErrorListener` do linter conta (`constantes::verificador::criacao_pode_ser_const`);
  a fase roda depois de `constantes::verificar`, com o mesmo `Motor`.
- `fase_estrita.rs`: "citado" pelo local resolvido (`UnitBodyTypes::declaracao_local` igual à
  declaração do parâmetro, o `_UsedParameterVisitor`); "sobrescreve" pelo `getOverridden2`
  (`fase_override::sobrescreve`, que vale também para o membro estático, como os candidatos herdados
  do analyzer); a expressão de função conferida só sem tipo de função no contexto, gravado pela
  inferência (`UnitBodyTypes::com_tipo_de_funcao`, o `wasFunctionTypeSupplied`); o construtor e o
  corpo `native` sempre visitados (a lista de inicializadores nunca é nula no analyzer); os
  parâmetros-função percorridos em toda a árvore (também em método que sobrescreve, em expressão de
  função e dentro de `Function(…)`); o texto do `Function(…)` sem retorno é o `toString()` do nó.
- `crates/frontend/src/fonte.rs` (novo): o `ToSourceVisitor` da 3.6.2 sobre a árvore daqui (tipos,
  parâmetros, anotações, expressões, elementos de coleção, instruções e padrões). O que a árvore não
  guarda sai da fonte no lugar em que o parser o consumiu (nome de campo posicional de record,
  `var`/`final` de curinga, `const` de padrão constante, separador do valor padrão, limites de
  strings adjacentes e de interpolações). A metadata das declarações locais, que o parser
  descartava, agora fica em `Ast::metadados_locais` (o formato do cache do SDK subiu para 4).
- `fase_nao_guardar.rs`: a metadata da biblioteca é a da diretiva `library` ou, sem ela, a da
  primeira diretiva; a criação de instância sem `new` não conta como `MethodInvocation`; o
  `_inDoNotStoreMember` é ligado só por classe, função (também local) e método marcados (não por
  mixin, enum nem extensão); o elemento local (variável, parâmetro, função local) conta pela própria
  metadata; os diretórios de teste são `test`, `integration_test`, `test_driver` e `testing`.
- `lints_tipados3.rs`: `only_throw_errors` com o `typeForInterfaceCheck` (limite promovido ou
  declarado); `no_runtimeType_toString` pula a extensão só quando o tipo estendido não é classe
  concreta, aceita o `runtimeType` resolvido a getter e o `realTarget` da cascata;
  `avoid_dynamic_calls` com o `realTarget` da cascata, as exclusões de `toString`/`noSuchMethod` só
  com alvo escrito, o `call` sobre tipo de função e o `++`/`--` pela leitura;
  `unrelated_type_equality_checks` com subtipagem normativa, `promoteToNonNull`, limites dos
  parâmetros de tipo, records pela atribuição, `lookUpConcreteMethod('call')` e o padrão relacional;
  `avoid_double_and_int_checks` pelo tipo resolvido da anotação
  (`UnitBodyTypes::tipos_de_anotacoes`, novo) e também com `is!`.
- `crates/frontend/src/pais.rs` (novo): o pai de cada expressão no papel que o analyzer distingue e o
  `inConstantContext` exato (`constantContext`, `ast.dart:6208`): atravessa expressões, argumentos,
  `if`/`for` de coleção, entradas de mapa e `...`; verdadeiro na anotação, nos argumentos de
  constante de enum, na criação/coleção/record com `const`, na lista `const`, no padrão constante com
  `const` e no `case` antigo (antes da 3.0); falso na lista sem `const`, no padrão sem `const` e em
  qualquer outro pai (interpolação, corpo de função, valor padrão, inicializador de construtor,
  partes do `for`, `when`, `?e`). `lints_tipados::pais_da_unidade` o monta por unidade.
- `lints_tipados.rs` (`prefer_is_empty`): contexto constante exato e o inicializador de construtor
  `const` de fora (o `_check`); o hexadecimal até 64 bits.
- `lints_tipados2.rs`: `prefer_contains` com o `getIntValue(…, context)` (identificador simples
  avaliado pelo `Motor` de constantes) e o `implementsAnyInterface` pelos limites;
  `use_is_even_rather_than_modulo` com o contexto constante exato e o literal sem sinal;
  `prefer_is_not_empty` pelo elemento que declara o `isEmpty` (`getChildren`, extensão inclusive);
  `unnecessary_string_interpolations` sem `trim` (o trecho de antes e o de depois têm de ser vazios);
  `unnecessary_null_aware_assignments` não cala o `[]=`.
- Lints da árvore com a semântica do motor: `dartforge_analise::lints::executar_com` recebe uma
  `Semantica` (programa, unidade, corpo, tabela de tipos, outline). Em `paridade` (`analise.rs`) os
  lints rodam sobre a árvore do programa, a mesma versão de linguagem da biblioteca e o `interner`
  dele, e o resultado fica em `Arquivo::relatos_de_lint`; a publicação filtra pelas regras ligadas.
  Sem semântica (arquivo fora do programa), as regras que o original decide pelo elemento não
  relatam. O crate `analise` passou a depender de `dartforge-types`.
- `crates/types/src/anotacoes.rs` (novo): o elemento de uma anotação pelos escopos do analyzer
  (locais declarados antes no bloco, parâmetros e parâmetros de tipo das funções que a contêm,
  membros declarados do tipo que a contém, unidade e prefixo); `anotacao_do_meta` e o
  `isDeprecated` do `dart:core` usam ele.
- `regras.rs`: `non_constant_identifier_names` com os doze visitantes (campos de record, construtor
  de tipo de extensão, `for`/`for-in` de coleção, parâmetros de `Function(…)`, variável de padrão de
  declaração sem palavra-chave, atalho `:nome` fora, augmentation só pulando os nomeados);
  `prefer_generic_function_type_aliases` com o `toSource`; `provide_deprecation_message` pelo
  elemento, em toda anotação da unidade (`pais::todas_as_anotacoes`).

Cada verificador é um módulo novo em `crates/analise/src/`, chamado no ponto
da ordem da §1.2. Os códigos de cada um estão nos lotes II.7 e II.8 (II.8 com
a especificação lida do emissor; II.7 gerado por script: **ler o emissor
antes de implementar**). Ordem sugerida, pelo que aparece em projeto real:

| ordem | verificador | lote | o que precisa do motor |
|---|---|---|---|
| 1 | `OverrideVerifier` (`override_on_non_overriding_member`) | II.8 | interface herdada por nome (já existe em `crates/types/src/hierarchy.rs`) |
| 2 | `DeprecatedMemberUseVerifier` | II.8 | anotação `@Deprecated` resolvida no elemento; o elemento de cada referência |
| 3 | imports: `duplicate_import`, `unnecessary_import`, `duplicate_shown_name`, `duplicate_hidden_name`, `undefined_hidden_name` | II.8 | o namespace por diretiva (qual import traz cada elemento usado) |
| 4 | `MustCallSuperVerifier`, `UseResultVerifier`, `RedeclareVerifier` | II.8 | anotações do `package:meta` |
| 5 | `BestPracticesVerifier` e `AnnotationVerifier` (`invalid_annotation_target`, `must_be_immutable`, `invalid_use_of_protected_member`, `invalid_use_of_visible_for_testing_member`, …) | II.7 | idem; a pasta do arquivo (`lib/src`, `test`) |
| 6 | `SdkConstraintVerifier` | II.8 | `environment: sdk:` do `pubspec.yaml` e os `@Since` do SDK |
| 7 | `UnicodeTextVerifier` | II.8 | só o texto |
| 8 | `TodoFinder` | II.8 | os comentários do lexer |

Regra de publicação durante a etapa: a mesma de hoje (nada emitido errado no
corpus e nos pacotes do pub-cache). O corpus de diagnósticos não cobre vários
desses códigos; antes de publicar um, acrescentar ao corpus os casos do
diretório de testes do próprio analyzer para o código
(`pkg/analyzer/test/src/diagnostics/<codigo>_test.dart`, na tag 3.6.2 —
**não verificado** que todos existem na cópia de `E:\references`).

## III.7 Etapa 7 — Arquivos não-Dart

Lote II.10 (lido do emissor, com as especificações completas). Três
validadores, cada um um módulo em `crates/analise/src/naodart/`:

1. `analysis_options.yaml`: os `AnalysisOptions*Code` (chaves desconhecidas
   com a sugestão, valores inválidos de `errors:`, `include` inexistente,
   regras de lint desconhecidas, duplicadas, incompatíveis, removidas).
2. `pubspec.yaml`: os `PubspecWarningCode` (dependência por caminho
   inexistente, `asset` inexistente, nome inválido, campos obsoletos).
3. `AndroidManifest.xml`: os `ManifestWarningCode` — só em projeto Flutter;
   por último.

Saída: entram no **bloco prioritário** da §5.4 (antes dos `.dart`, com a
ordenação própria) e contam para o código de saída. O placar precisa parar
de descartar do oráculo o que não é `.dart`
(`crates/paridade/src/oraculo.rs:87-91`).

**Pronto quando** os casos dirigidos de II.10 (cada especificação completa
traz os seus) passam pela etapa 1.

## III.8 Etapa 8 — Lints

1. Primeiro o que sai para **qualquer** projeto com `linter:`, sem nenhuma
   regra implementada: a validação da lista de regras (`undefined_lint`,
   `duplicate_rule`, `incompatible_lint`, `removed_lint`, `deprecated_lint`),
   que só precisa da tabela de nomes e estados da §8.5.
2. `Registry` de regras e a execução da §8.4 (um visitante por unidade com o
   registro de nós por regra; as exceções de uma regra são engolidas).
3. As regras, na ordem: conjunto `core` do `package:lints` da versão que o
   `dart create` do 3.6.2 gera, depois `recommended`, depois `flutter_lints`,
   cada uma com a sua especificação (fora do escopo deste documento: a §8 só
   descreve o mecanismo e lista os nomes).

Enquanto a etapa 8.3 não fechar, a paridade de saída só é exigida de projetos
**sem** regras de lint ligadas; o harness da etapa 1 marca cada caso com
`lints: sim|não` e o relatório separa os dois grupos.

## III.9 Etapa 9 — Fim do filtro de publicação

O filtro (`crates/analise/src/publicacao.rs:24-34`: só sintaxe e
`verificados.txt`) é, por definição, uma diferença do oficial. Ele sai por
último, código a código, pela regra que já vale: um código entra em
`verificados.txt` quando não emite nada errado no corpus e nos pacotes do
pub-cache. O estado de cada constante está nas tabelas da Parte II; o total
hoje, pelas contagens do script, é o da tabela de abertura da Parte II.

**Critério final do documento:** para cada caso do harness da etapa 1 marcado
`lints: não`, os três formatos e o código de saída são iguais aos do oráculo;
e `dartforge analyze` sem `--todos` e com `--todos` dão a mesma saída (o
filtro deixou de filtrar).

## III.10 O que este plano não cobre

- A correção de cada código: é o outro documento (`ANALYZER-ESPECIFICACAO.md`).
- O servidor de análise como processo residente (o `dart analyze` fala com um
  `analysis_server`; o DartForge analisa em processo): só a **saída** é
  especificada, não o protocolo interno.
- `--cache` e `--memory` do `dart analyze`: aceitar e ignorar.
- A saída com terminal (ANSI, quebra por largura).
