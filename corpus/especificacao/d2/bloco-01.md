### D.R4-2 ConstantVerifier, criação const e padrões constantes (rodada 4)

Fonte: SDK 3.6.2 — `analyzer/lib/src/dart/constant/{constant_verifier,potentially_constant,has_type_parameter_reference,evaluation,value}.dart`,
`analyzer/lib/src/generated/{error_verifier,resolver,ffi_verifier}.dart`, `analyzer/lib/src/error/type_arguments_verifier.dart`,
`analyzer/lib/src/dart/ast/ast.dart`, `analyzer/lib/src/dart/element/element.dart`, `analyzer/lib/error/listener.dart`,
`analyzer/lib/src/fasta/error_converter.dart`, `_fe_analyzer_shared/lib/src/{parser/parser_impl,type_inference/type_analyzer,flow_analysis/flow_analysis}.dart`,
`analyzer/messages.yaml`; linhas conferidas nesta sessão. Exemplos rodados no oráculo vivo (`C:\tools\dartsdk-3.6.2`); arquivos em
`E:\dftemp\analise\spec-r4\casos\d2\{a,b,c,e,f,g,h}\` (46 arquivos; `python roda.py <pasta>` reproduz). Formato dos diagnósticos:
`código off=offset len=length linha:coluna | mensagem`; `ctx` = mensagem de contexto. Nos exemplos longos a entrada aparece linha a linha
(`linha: trecho → diagnóstico`); o arquivo inteiro está na pasta de casos. Perdas de `E:\dftemp\analise\trab\familia-D.txt`, amostras de
`placar-r7.txt`. O motor de avaliação (valor/erro de cada nó, grafo, exceções) é de D.R4-1; aqui entra o que o verificador manda avaliar,
com que código padrão, e as regras de constantes que moram fora dele (`ErrorVerifier`, `TypeArgumentsVerifier`, parser, `FfiVerifier`).

#### 1. Percurso do `ConstantVerifier`

##### 1.1 Quando roda e estado

`LibraryAnalyzer._computeVerifyErrors` roda, por unidade, `unit.accept(ConstantVerifier)` antes do `InheritanceOverrideVerifier`, do
`ErrorVerifier` e do `FfiVerifier` (§0 T5 e D.R4-1 §1.1). É um `RecursiveAstVisitor<void>` (`constant_verifier.dart:40`): todo nó sem
`visit*` próprio só desce nos filhos. Estado (`:41-69`):

| campo | uso |
|---|---|
| `_errorReporter` | relator real da unidade |
| `_currentLibrary`, `_typeSystem`, `_typeProvider` | `featureSet` (patterns, non_nullable, constructor_tearoffs) e versão vêm da **biblioteca** |
| `_evaluationEngine` | um `ConstantEvaluationEngine` novo, com `ConstantEvaluationConfiguration()` **vazio** (`:94-97`): não tem os `errorNode` registrados pelo `ConstantFinder`; reavaliações feitas aqui relatam no nó da AST real |
| `_exhaustivenessCache` | cache da exaustividade (um por verificador) |
| `_constantPatternValues`, `_mapPatternKeyValues` | valores dos padrões constantes / chaves de padrão de mapa do `switch` corrente; `null` fora de `switch` (`_withConstantPatternValues`, `:1049-1061`, empilha e restaura) |

O mesmo relato vindo de dois lugares (verificador e `ErrorVerifier` para `const_with_non_const`; literal const visitado como filho e
reavaliado como spread) aparece **uma vez** na saída do `dart analyze` (mesmo código, offset, length e mensagem). Onde essa deduplicação
é feita: não verificado nesta parte. Nenhuma regra depende da ordem entre verificadores.

##### 1.2 `isConst`, contexto constante e "expressão constante"

- `InstanceCreationExpression.isConst` (`ast.dart:10472-10478`): com palavra escrita, `keyword == const`; sem palavra (`isImplicit`,
  `:10481`), `inConstantContext`. `new C()` dentro de contexto constante **não** é const.
- `TypedLiteral.isConst` (`ast.dart:18067-18069`) e `RecordLiteral.isConst` (`:14752`): `constKeyword != null || inConstantContext`.
- `inConstantContext` = `constantContext(includeSelf: false) != null` (`ast.dart:6156-6158`, laço em `:6208-6263`). Sobe pelos pais:

| pai encontrado | resultado |
|---|---|
| `Annotation`, `ConstantContextForExpressionImpl` (inicializador desanexado pelo linker, `summary2/detach_nodes.dart:112`), `EnumConstantArguments`, `SwitchCase` (caso antigo) | contexto constante |
| `ConstantPatternImpl` | constante **só se** tem `const` escrito; sem `const` devolve `null` (para de subir) |
| `InstanceCreationExpression` com `const` escrito, `RecordLiteral`/`TypedLiteral` com `const` escrito | contexto constante |
| `VariableDeclarationList` | constante se a palavra é `const`; senão `null` (para) |
| `ArgumentList`, `Expression`, `IfElement`, `ForElement`, `MapLiteralEntry`, `SpreadElement`, `VariableDeclaration` | continua subindo |
| qualquer outro (`DefaultFormalParameter`, `ConstructorFieldInitializer`, `AssertInitializer`, `RelationalPattern`, `MapPatternEntry`, corpo de função…) | `null`: **não** é contexto constante |

  Consequências: valor padrão de parâmetro, inicializador de construtor const, operando de `RelationalPattern`, chave de padrão de mapa e
  `case expr` de padrão sem `const` **não** são contexto constante — `[1]` ali é lista não constante e `C()` é criação não constante
  (o erro sai pela avaliação: `non_constant_default_value` no nó, `invalid_constant`, etc.).
- `inConstantExpression` (extensão privada, `constant_verifier.dart:1417-1450`), usada só por `visitConstructorReference` e
  `visitFunctionReference`: verdadeiro se, subindo, o nó é o `defaultValue` de um `DefaultFormalParameter`, ou o inicializador de um
  campo **de instância** (`FieldDeclaration` não estática) de uma `ClassDeclaration` cuja classe tem `hasGenerativeConstConstructor`
  (`element.dart:324-326`: algum construtor não factory e const). A subida para no primeiro `VariableDeclaration` cujo inicializador é o
  filho (devolve `false` para local, topo, estático, enum, mixin, extension type). Conferido: `g04.dart` (classe `K` × classe `L` só com
  `const factory` × enum `En`), em `const_with_type_parameters`.

##### 1.3 As visitas, em ordem de arquivo

Pseudocódigo fiel (`→ R` = relata; "desce" = `super.visit…`):

```
visitAnnotation (:103-128)
  desce PRIMEIRO (nome, argumentos de tipo, argumentos: criações/literais const dos argumentos são verificados aqui)
  el = node.element
  se el é ConstructorElement:
     se !el.isConst:              → R NON_CONSTANT_ANNOTATION_CONSTRUCTOR no nó Annotation; return
     se node.arguments == null:   → R NO_ANNOTATION_CONSTRUCTOR_ARGUMENTS no nó Annotation; return
     _validateConstantArguments(arguments)   // cada argumento (sem o rótulo): _evaluateAndReportError(arg, CONST_WITH_NON_CONSTANT_ARGUMENT)
  // el variável/getter, função, null: nada aqui (INVALID_ANNOTATION/UNDEFINED_ANNOTATION são da resolução)

visitConstantPattern (:131-162)                    — §4.1
visitConstructorDeclaration (:165-192)
  se tem `const`:
     se element.isCycleFree == false e não factory: → R RECURSIVE_CONSTANT_CONSTRUCTOR em node.returnType
     _validateConstructorInitializers(node)        — §2 (INVALID_CONSTANT por nó não potencialmente constante)
     se não factory: _validateFieldInitializers(membros da declaração-pai, constKeyword, isEnumDeclaration)
  _validateDefaultValues(node.parameters)          // sempre, const ou não
  desce                                            // parâmetros, inicializadores, corpo: criações/literais const internos
visitConstructorReference (:195-201)   desce; se inConstantContext || inConstantExpression:
                                         _checkForConstWithTypeParameters(constructorName.type, …_CONSTRUCTOR_TEAROFF)
visitEnumConstantDeclaration (:204-217) desce; se há argumentos: _validateConstantArguments;
                                         r = element.evaluationResult; se InvalidConstant: _reportError(r, null)   // só códigos da lista
visitFunctionExpression (:220-223)     desce; _validateDefaultValues(parameters)   // closures e funções locais/de topo
visitFunctionReference (:226-238)      desce; se inConstantContext || inConstantExpression: para cada argumento de tipo ESCRITO:
                                         _checkForConstWithTypeParameters(arg, …_FUNCTION_TEAROFF)
visitGenericFunctionType (:241-250)    desce; se o pai é AsExpression/IsExpression e o pai está inConstantContext:
                                         _checkForConstWithTypeParameters(node, CONST_WITH_TYPE_PARAMETERS)
visitInstanceCreationExpression (:253-291)
  se !node.isConst: desce; return
  _checkForConstWithTypeParameters(constructorName.type, CONST_WITH_TYPE_PARAMETERS)
  ctor = constructorName.staticElement
  se ctor != null:
     r = evaluateAndFormatErrorsInConstructorCall(lib, node, ctor.returnType.typeArguments, argumentos, ctor, ConstantVisitor(relator REAL))
     InvalidConstant e !avoidReporting: → R r.errorCode em (r.offset, r.length), com contextMessages    // SEM filtro de _reportError
     DartObjectImpl: node.argumentList.accept(this)       // só a lista de argumentos
  // NÃO desce: nem no tipo, nem nos argumentos quando ctor == null ou a avaliação falhou
visitListLiteral (:294-308)            desce; se isConst: _ConstLiteralVerifier(NON_CONSTANT_LIST_ELEMENT,
                                         listElementType = staticType.typeArguments[0]) em cada elemento — §3
visitMapPattern (:311-358)             — §4.3
visitMethodDeclaration (:361-364)      desce; _validateDefaultValues(parameters)
visitRecordLiteral (:367-378)          desce; se isConst: cada campo → _evaluateAndReportError(campo, NON_CONSTANT_RECORD_FIELD)
visitRelationalPattern (:381-388)      desce; _evaluateAndReportError(operand, NON_CONSTANT_RELATIONAL_PATTERN_EXPRESSION) — §4.2
visitSetOrMapLiteral (:391-434)        desce; se isSet && isConst: verificador de conjunto; se isMap && isConst: de mapa — §3
visitSwitchExpression (:437-451)       _withConstantPatternValues { desce; _validateSwitchExhaustiveness(mustBeExhaustive: true) }
visitSwitchStatement (:454-473)        _withConstantPatternValues { desce;
                                          se patterns: _validateSwitchExhaustiveness(mustBeExhaustive: isAlwaysExhaustive(tipo do valor))
                                          senão: _validateSwitchStatement_nullSafety(node) }
visitVariableDeclaration (:476-512)
  desce
  se há inicializador e (isConst || isFinal):
     se o elemento é campo NÃO estático e a classe (ClassElementImpl) não tem hasGenerativeConstConstructor: return
     r = element.evaluationResult      // calculado antes, por computeConstants (D.R4-1)
     se r == null: return              // final que não é alvo de constante
     se InvalidConstant: isConst ? _reportError(r, CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE) : _reportError(r, null)
```

Quem **para a descida**: `visitConstantPattern` (não desce se a expressão tem `InvalidType`, se a avaliação falhou, ou se relatou
`CONSTANT_PATTERN_NEVER_MATCHES_VALUE_TYPE`), `visitInstanceCreationExpression` const (só desce nos argumentos, e só com valor),
`visitMapPattern` (visita argumentos de tipo e elementos à mão). `_validateFieldInitializers` e `_validateConstructorInitializers` não
descem: avaliam/colhem; a descida normal vem depois, pelo `super.visitConstructorDeclaration` e pela visita dos campos.

`_validateDefaultValues` (`:811-836`): para cada `DefaultFormalParameter`: sem valor → resultado `null` constante; valor com
`staticType is InvalidType` → nada (erro já relatado; `t = Indef` em `g05.dart` só dá `undefined_identifier`); senão
`_evaluateAndReportError(valor, NON_CONSTANT_DEFAULT_VALUE)`; o resultado é gravado em `element.evaluationResult`. Chamado em
construtores, métodos e `FunctionExpression` (funções de topo, locais e closures); tipos de função não têm valor padrão.

`_validateFieldInitializers` (`:842-881`): para cada `FieldDeclaration` **não estática** da declaração-pai (todas as variáveis com
inicializador, `final` **ou não**; em enum pula a de nome `values`): `initializer.accept(ConstantVisitor(relator NULO))`; se o resultado
não é `DartObjectImpl` → `CONST_CONSTRUCTOR_WITH_FIELD_INITIALIZED_BY_NON_CONST` no token `const` do construtor, argumento = nome do campo.
Uma vez por construtor gerador const × campo.

##### 1.4 `_evaluateAndReportError` e a tabela de `_reportError`

`_evaluateAndReportError(expr, padrão)` (`:615-628`): `ConstantVisitor(engine, lib, relator de gravação descartado).evaluateConstant(expr)`;
se `InvalidConstant` → `_reportError(erro, padrão)`; devolve o resultado. `_reportError(erro, padrão?)` (`:634-747`):

```
se erro.avoidReporting: nada
se erro.errorCode ∈ LISTA: relata erro.errorCode em (erro.offset, erro.length) com erro.arguments e erro.contextMessages
senão se padrão != null:   relata padrão em (erro.offset, erro.length), SEM argumentos e SEM contextMessages
senão: nada
```

A posição é sempre a do erro (o nó mais interno que falhou), nunca a do nó pedido. Classificação completa dos códigos que o motor produz
(busca por `CompileTimeErrorCode` em `evaluation.dart` e `value.dart`):

| destino | códigos |
|---|---|
| **repassados** (LISTA, `:645-726`) | `CONST_EVAL_EXTENSION_METHOD`, `CONST_EVAL_EXTENSION_TYPE_METHOD`, `CONST_EVAL_FOR_ELEMENT`, `CONST_EVAL_METHOD_INVOCATION`, `CONST_EVAL_PROPERTY_ACCESS`, `CONST_EVAL_THROWS_EXCEPTION`, `CONST_EVAL_THROWS_IDBZE`, `CONST_EVAL_TYPE_BOOL_NUM_STRING`, `CONST_EVAL_TYPE_BOOL`, `CONST_EVAL_TYPE_BOOL_INT`, `CONST_EVAL_TYPE_INT`, `CONST_EVAL_TYPE_NUM`, `CONST_EVAL_TYPE_NUM_STRING`, `CONST_EVAL_TYPE_STRING`, `RECURSIVE_COMPILE_TIME_CONSTANT`, `CONST_CONSTRUCTOR_FIELD_TYPE_MISMATCH`, `CONST_CONSTRUCTOR_PARAM_TYPE_MISMATCH`, `CONST_TYPE_PARAMETER`, `CONST_WITH_TYPE_PARAMETERS_FUNCTION_TEAROFF`, `CONST_SPREAD_EXPECTED_LIST_OR_SET`, `CONST_SPREAD_EXPECTED_MAP`, `EXPRESSION_IN_MAP`, `VARIABLE_TYPE_MISMATCH`, `NON_BOOL_CONDITION`, os `*_FROM_DEFERRED_LIBRARY` (`NON_CONSTANT_DEFAULT_VALUE_`, `NON_CONSTANT_MAP_KEY_`, `NON_CONSTANT_MAP_VALUE_`, `SET_ELEMENT_`, `SPREAD_EXPRESSION_`, `NON_CONSTANT_CASE_EXPRESSION_`, `INVALID_ANNOTATION_CONSTANT_VALUE_`, `IF_ELEMENT_CONDITION_`, `CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE_`, `NON_CONSTANT_LIST_ELEMENT_`, `NON_CONSTANT_RECORD_FIELD_`, `PATTERN_CONSTANT_`), `WRONG_NUMBER_OF_TYPE_ARGUMENTS_FUNCTION`, `WRONG_NUMBER_OF_TYPE_ARGUMENTS_ANONYMOUS_FUNCTION` |
| **substituídos pelo padrão do chamador** (fora da LISTA; com padrão `null`, silenciados) | `INVALID_CONSTANT` (o genérico), `CONST_WITH_NON_CONST`, `CONST_WITH_NON_CONSTANT_ARGUMENT`, `CONST_EVAL_ASSERTION_FAILURE`(`_WITH_MESSAGE`), `CONST_EVAL_TYPE_TYPE`, `MISSING_CONST_IN_LIST_LITERAL`/`_MAP_`/`_SET_`, `MAP_ENTRY_NOT_IN_MAP`, `NULL_AWARE_ELEMENT_IN_MAP`, `AMBIGUOUS_SET_OR_MAP_LITERAL_BOTH`/`_EITHER`, `CONST_CONSTRUCTOR_CONSTANT_FROM_DEFERRED_LIBRARY` |
| **silenciados sempre** | qualquer erro com `avoidReporting` (uso de variável em ciclo, argumento não resolvido já relatado) |

Chamadores e o código padrão de cada um:

| chamador | padrão |
|---|---|
| `visitConstantPattern` | `CONSTANT_PATTERN_WITH_NON_CONSTANT_EXPRESSION` |
| `visitRelationalPattern` | `NON_CONSTANT_RELATIONAL_PATTERN_EXPRESSION` |
| `visitMapPattern` (chave) | `NON_CONSTANT_MAP_PATTERN_KEY` |
| `visitRecordLiteral` | `NON_CONSTANT_RECORD_FIELD` |
| `_validateConstantArguments` (anotação, constante de enum) | `CONST_WITH_NON_CONSTANT_ARGUMENT` |
| `_validateDefaultValues` | `NON_CONSTANT_DEFAULT_VALUE` |
| `visitVariableDeclaration` const / final | `CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE` / `null` |
| `visitEnumConstantDeclaration` (resultado guardado) | `null` |
| `_validateSwitchStatement_nullSafety` | `NON_CONSTANT_CASE_EXPRESSION` |
| `_ConstLiteralVerifier` elemento, condição de `if`, spread | `NON_CONSTANT_LIST_ELEMENT` / `NON_CONSTANT_SET_ELEMENT` / `NON_CONSTANT_MAP_ELEMENT`; chave/valor de entrada: `NON_CONSTANT_MAP_KEY` / `NON_CONSTANT_MAP_VALUE` |

`visitInstanceCreationExpression` é a exceção: relata o código do erro **como veio**, sem a tabela (por isso `const_with_non_const`, o
`const_with_non_constant_argument` de argumento direto e `const_eval_throws_exception` saem dali com o próprio nome).

##### 1.5 O que não é do `ConstantVerifier`

| regra | dono | linha |
|---|---|---|
| `CONST_WITH_NON_CONST` (2º caminho), `CONST_WITH_UNDEFINED_CONSTRUCTOR(_DEFAULT)`, `CONST_DEFERRED_CLASS`, `INSTANTIATE_ABSTRACT_CLASS`, `MIXIN_INSTANTIATE` | `ErrorVerifier.visitInstanceCreationExpression` | `error_verifier.dart:1101-1122` |
| `NON_CONST_GENERATIVE_ENUM_CONSTRUCTOR`, `CONST_CONSTRUCTOR_WITH_MIXIN_WITH_FIELD(S)`, `CONST_CONSTRUCTOR_WITH_NON_CONST_SUPER`, `CONST_CONSTRUCTOR_WITH_NON_FINAL_FIELD` | `ErrorVerifier.visitConstructorDeclaration`, nesta ordem | `:589-615` |
| `CONST_NOT_INITIALIZED` | parser (campos e topo) + `ErrorVerifier._checkForFinalNotInitialized` (topo e locais) | `parser_impl.dart:3942-3947`; `error_verifier.dart:3576-3617` |
| `INVALID_TYPE_ARGUMENT_IN_CONST_LIST/MAP/SET` | `TypeArgumentsVerifier`, chamado do `ErrorVerifier.visitListLiteral`/`visitSetOrMapLiteral` | `type_arguments_verifier.dart:159-222`, `:501-544`; `error_verifier.dart:1154`, `:1393`, `:1397` |
| `CONST_CONSTRUCTOR_THROWS_EXCEPTION` | `ErrorVerifier._checkForConstEvalThrowsException` | `:2936-2943` |
| `NON_CONSTANT_TYPE_ARGUMENT` | `FfiVerifier` (9 pontos) | `ffi_verifier.dart:1127-2035` |
| `INVALID_ANNOTATION`, `UNDEFINED_ANNOTATION` | resolução de anotações | emissor não verificado nesta parte |

