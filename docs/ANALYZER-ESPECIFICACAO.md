# Especificação do analyzer 3.6.2 por código de diagnóstico (rodada 3, 2026-10-02)

Fonte: o código oficial na tag 3.6.2 do SDK, extraído para
`E:\references\dart-sdk-3.6.2\pkg\{analyzer,_fe_analyzer_shared,analysis_server}\lib`
e `pkg/analyzer/messages.yaml`, `pkg/front_end/messages.yaml` (o checkout
`E:\references\dart-sdk` está no main/3.14 e não serve). Citações
`analyzer/lib/src/...:linha`, `_fe_analyzer_shared/lib/src/...:linha`.

Cobertura: os **436 códigos** com alguma perda no placar `r7`
(18.535/23.030; perda total 5.256 = mensagem + posição + FP + FN), em seis
famílias com listas fixas (`E:\dftemp\analise\trab\familia-*.txt`):

| família | parte | códigos | perda | linhas |
|---|---|---:|---:|---:|
| A invocação, argumentos, membros, atribuição | §A | 101 | 1.603 | 728 |
| B fluxo, nulidade, padrões, código morto | §B | 60 | 703 | 1.223 |
| C declarações, herança, construtores, limites, FFI | §C | 179 | 1.402 | 2.623 |
| D constantes | §D | 34 | 439 | 564 |
| E parser e scanner (com o `for` de padrão) | §E | 38 | 477 | 252 |
| F nomes e tipos não resolvidos, não usados | §F | 23 | 632 | 361 |

Cada código traz: **emissão** (visitante/verificador e método, arquivo:linha,
fase), **condição exata**, **posição** (nó/token, offset e length), **mensagem**
(modelo e formatação de cada argumento), **supressões e ordem**, e **no
DartForge** (onde a regra entra, o que existe e a causa das divergências, com
as amostras do placar).

## 0. Achados transversais — implementar ANTES das famílias

Estes pontos aparecem em várias famílias; cada um tem **uma** correção no
ponto comum, feita uma vez, antes de qualquer família que dependa dele.

### T1. Declarações homônimas e `augment` sem o experimento: a primeira vence

No 3.6.2, `augment class X …` sem o experimento é recuperado pelo parser como
uma variável de topo `augment` seguida de uma **segunda** declaração `X`
(`expected_token` + `missing_const_final_var_or_type` + `duplicate_definition`).
- O escopo dá ao nome a **primeira** declaração
  (`analyzer/lib/src/dart/element/scope.dart:793-797`, `_getters[id] ??= element`);
  o DartForge dá a **última** (`crates/elements/src/outline.rs:131-134`).
- A igualdade de elementos é por localização
  (`analyzer/lib/src/dart/element/element.dart:2872`, `hashCode` em `:2472`):
  caches por elemento (`InheritanceManager3._interfaces`,
  `inheritance_manager3.dart:69/239`; `ClassHierarchy._map`,
  `class_hierarchy.dart:14,44`; o `Map<FieldElement,…>` do verificador de
  inicialização) **compartilham** o resultado entre homônimos: a interface é a
  da primeira declaração, os membros AST são os de cada uma, e o relato vai no
  nome de cada declaração.
- O que não é cache por elemento continua separado
  (`element.augmented.constants` de `enum_without_constants`).

Correção única: (1) em `elements/src/outline.rs`, o nome de topo repetido
resolve para a **primeira** declaração (as seguintes continuam elementos
próprios, com os próprios membros); (2) remover os filtros que pulam classes
homônimas e unidades com linha `augment ` quando a biblioteca **não** tem
`Feature::Augmentations`/`Macros` — `analise/src/clausulas.rs:1114`
(`verificador_de_heranca_prossegue`), `types/src/sobrescritas.rs:1029-1042`,
`:1358-1364`, `:1525-1534`, `:699-710`, `analise/src/construtores.rs`
(`verificar`), `analise/src/inicializacao.rs` (índice por nome de classe);
(3) para homônimos, interface/hierarquia calculadas uma vez pela primeira
declaração e reaproveitadas, relato no nome de cada uma. Famílias que
dependem: C (herança, construtores: FN AUG de `invalid_factory_name_not_a_class`,
`non_abstract_class_inherits_abstract_member`, `conflicting_*`,
`recursive_interface_inheritance`, `final_not_initialized_constructor`…),
F (`undefined_class` em `augment mixin/extension`), A (membros resolvidos).

### T2. Qual oráculo julga cada arquivo (3.6.2 × 3.13.4)

Os arquivos de sintaxe nova são julgados pelo 3.13.4 (`corpus/diagnosticos/sintaxe-nova.json`
e a regra do parser em §E: versão da biblioteca > 3.6, ou uso de
dot-shorthands/primary-constructors/private-named-parameters). Códigos e
mensagens mudaram entre as versões: `unused_element_parameter` (3.13) ×
`unused_element` (3.6) para parâmetros opcionais; `assignment_to_primary_constructor_parameter`
e `unused_field_from_primary_constructor` só existem no 3.13;
`experiment_not_enabled` cita "3.6.0" para null-aware-elements no 3.6.2
(`releaseVersion` nula → versão do SDK, `analyzer/lib/src/fasta/ast_builder.dart:6078`)
e "3.8.0" no 3.13; `representation_field_modifier` mudou de texto;
`const_constructor_with_non_const_super` mostra a classe derivada no 3.6.2 e a
base no 3.13. Correção única: um marcador por biblioteca, calculado uma vez
(`LibraryFeatures` ou o `Program`), que os emissores consultam para o nome do
código e o texto — nada de regras ad hoc por família.

### T3. O SDK como o analyzer o vê: declarações públicas, sem patches

O analyzer lê `dart:core` etc. sem os patches da plataforma; o DartForge
aplica o patch `js_dev_runtime`. Efeitos: FP de `const_with_non_const` em
`bool.fromEnvironment` (`factory` sem `const` no patch × `const factory` em
`core/bool.dart:67`), documentação/definição caindo em `core_patch.dart` no
LSP, nomes de parâmetros do patch em mensagens e inlay hints. Correção única
no carregamento (`crates/elements`): para a análise, o elemento público do
SDK é a declaração original; o patch só substitui o corpo (já existe
`FunctionElement::patched_by`).

### T4. `InvalidType` distinto de `dynamic`

Nome ou tipo que não resolve dá `InvalidType` no analyzer (comporta-se como
`dynamic`, mas suprime diagnósticos de tipo e aparece como `InvalidType` em
mensagens). Hoje: `UnitBodyTypes::tipos_invalidos` só marca expressões não
resolvidas, `padroes_invalidos`/`locais_invalidos` cobrem padrões e
parâmetros (rodada 2). Correção única: o tipo de recuperação na tabela de
tipos (um `TypeId` decorado como os de exibição, estrutura `dynamic`,
`Exibicao::Invalido`), propagado pela resolução de anotações e pela leitura,
consultado pelas supressões (`hasInvalidType` da exaustividade, `argument_type_not_assignable`,
`invalid_assignment`, `unnecessary_*`) e exibido "InvalidType".

### T5. Ordem dos verificadores e supressões gerais

`analyzer/lib/src/dart/analysis/library_analyzer.dart:420-456`
(`_computeVerifyErrors`): `ConstantVerifier` → `InheritanceOverrideVerifier.verifyUnit`
→ `ErrorVerifier`; antes, por biblioteca, `MemberDuplicateDefinitionVerifier`.
O verificador de elementos não usados **não** tem porta de erro de sintaxe
(§F): a nossa porta por biblioteca (`paridade/src/analise.rs`,
`libs_com_erro_de_sintaxe`) custa FN em `unused_*`. Erros de sintaxe não
descartam o resto da análise; a lista dos que não descartam código
(`paridade::analise::recuperacao_do_parser`) é a referência.

### T6. Fluxo de padrões e o tipo casado

O analyzer modela o valor casado e o acumulador "não casou" no fluxo
(`_fe_analyzer_shared/lib/src/flow_analysis/flow_analysis.dart`, `switchStatement_*`,
`patternAssignment`, `promoteForPattern`): dele dependem `dead_code` em
padrões, `constant_pattern_never_matches_value_type` (tipo casado promovido
pelos casos anteriores), `pattern_never_matches_value_type`,
`unnecessary_*_pattern`. Ver §B Apêndice A; a tabela lateral do tipo casado
por padrão (`UnitBodyTypes`) é a interface entre B (inferência) e D
(verificador de constantes).

### T7. Formatação de tipos nas mensagens

`ErrorReporter._convertTypeNames` (`analyzer/lib/error/listener.dart:362-376`):
argumento `DartType` → `getDisplayString(preferTypeAlias: true)` (`TypeTable::format`);
argumento já `String` → sem alias (`format_sem_alias`). Tipos de função
genéricos aninhados renomeiam parâmetros que colidem com subscritos (`T₀`,
`ElementDisplayStringBuilder`). Alguns códigos mostram o **nome** do elemento,
não o tipo (`undefined_method` `{1}`: `'C'` sem argumentos, `'Function'` para
tipos de função, `'<unknown>'` para records — §A). Correção única no
`TypeTable` (renomeação `T₀`) e nos emissores pela regra do argumento.

### T8. Grafo de dependências de constantes

`analyzer/lib/src/dart/constant/compute.dart:62-73` +
`evaluation.dart:223-325` (Tarjan): ciclos dão `recursive_compile_time_constant`
e `recursive_constant_constructor`; as constantes de enum são avaliadas pelo
construtor (§D). Correção única no `Motor` de constantes.

### Ordem de implementação

1. T1, T2, T3, T4 (pontos comuns, um responsável cada, com placar antes/depois).
2. Famílias em paralelo, cada uma nos seus arquivos: A, B (com T6), C, D (com
   T8), E (com o scanner e o `for` de padrão ponta a ponta), F.
3. Placar, 209 pacotes, testes e publicação no fim.

---


## A. Invocação, argumentos, membros, atribuição

### Especificação — família A (invocação, argumentos, membros, atribuição) — r3-a

Fonte: analyzer **3.6.2** (`E:\references\dart-sdk-3.6.2\pkg\analyzer\lib`, `…\_fe_analyzer_shared\lib`, `…\analyzer\messages.yaml`); citações `analyzer/lib/src/...:linha` relativas a `E:\references\dart-sdk-3.6.2\pkg\`. Números de perda de `E:\dftemp\analise\trab\familia-A.txt` (placar base `placar-base-r3.txt`).

Cobertura: os 101 códigos da lista, agrupados por mecanismo (a ordem dentro de cada grupo segue a perda):

1. Atribuibilidade — argumentos, atribuições, retornos, coleções, `await`/`throw`/`for-in` (19 códigos).
2. Membros indefinidos, invocações e `void` (16).
3. `this`/`super` em contexto inválido, atribuição a final/const (10).
4. Declarações de extension type, `main`, diretivas de doc, FFI (24).
5. Argumentos de tipo — limites, inferência, contagem (17).
6. Aridade de argumentos, `super.x`, construtores indefinidos (15).

Achados transversais (valem para vários códigos):

- **Nome vs. tipo nos argumentos.** `UNDEFINED_METHOD {1}` é o **nome do elemento** (`'C'`, sem argumentos de tipo; `'Function'` para tipos de função; `'<unknown>'` para o resto); `UNDEFINED_GETTER/SETTER/OPERATOR {1}` de instância é o **DartType** (com argumentos); os estáticos usam o nome da classe (`typeReference.name`). Vários "mensagem errada" e os FP de `{1}` vazio vêm daí.
- **Chamada estática indefinida `C.m()`** sai hoje como `UNDEFINED_GETTER` sem `{1}`; no analyzer é `UNDEFINED_METHOD [m, C]` (ou `NEW_WITH_UNDEFINED_CONSTRUCTOR_DEFAULT` para `C.new()`).
- **`f()` indefinido** sai hoje como `UNDEFINED_IDENTIFIER`; no analyzer é `UNDEFINED_FUNCTION` (sem `this`) ou `UNDEFINED_METHOD [f, classe]` (com `this`).
- **Top-merge (NNBD_TOP_MERGE)** das assinaturas combinadas de interfaces: falta; causa ~60 divergências em `argument_type_not_assignable`, `use_of_void_result`, `return_of_invalid_type(_from_closure)` (`nnbd/top_merge/*`, `extension_type/combined_member_signature_error_test`).
- **Pontos de checagem ausentes** na atribuibilidade: `==` (parâmetro tornado anulável), `++`/`--` (`int` contra o parâmetro do operador, no operando; resultado contra o `writeType`, no nó), atribuição composta e `??=` (resultado do operador contra o `writeType`; lado direito contra o parâmetro), índice de leitura e escrita, valor padrão de parâmetro, elementos de coleção **não const**, closures `async`, `throw`, `await`, `for-in`.
- **`getErrorNode`** (desembrulha parênteses e cascata) vale para argumentos, inicializadores de variável e valores padrão; **não** para o lado direito de atribuição.
- **Tear-off implícita de `call`** (objeto com `call` em contexto de tipo de função) falta na inferência: origina FP e mensagens erradas em `invalid_assignment`.
- **Aridade** (já reescrita no working tree, a validar): posição do excedente/token depois do último posicional, quatro variantes do `NOT_ENOUGH_POSITIONAL_ARGUMENTS` com o nome conforme a forma da invocação, `_COULD_BE_NAMED`, `DUPLICATE_NAMED_ARGUMENT`, `super.x` contados como argumentos, entidade correta do `MISSING_REQUIRED_ARGUMENT`.
- Códigos que **não existem no 3.6.2** (julgados pelo oráculo 3.13 em arquivos de sintaxe nova): `assignment_to_primary_constructor_parameter` (40) e parte de `extension_type_declares_member_of_object`, `return_of_invalid_type` (`constructor_body/*`), `undefined_getter` (`anonymous_methods/*`), `instantiate_abstract_class` (`dot_shorthands/*`), `*_positional_arguments` (`dot_shorthands/*`).
- `implicit_super_initializer_missing_arguments` e `super_formal_parameter_without_associated_*` **já estão implementados** em `crates/analise/src/construtores.rs:515-588`, mas o placar dá *nosso 0*: o bloco não é alcançado (ou o diagnóstico não chega à paridade) — investigar antes de reimplementar.

#### Grupo 1 — atribuibilidade (argumentos, atribuições, retornos, coleções, await/throw/for-in)

Base comum (vale para vários códigos abaixo):

- `checkForAssignableExpressionAtType` (`analyzer/lib/src/generated/error_detection_helpers.dart:69-158`):
  1. se o tipo esperado não é `void` e `checkForUseOfVoidResult(expression)` relatou, para (sai `USE_OF_VOID_RESULT`, não o código pedido);
  2. se `isAssignableTo(actual, expected, strictCasts)`, nada;
  3. **nó do erro** = `getErrorNode(expression)`: desce `CascadeExpression → target` e `ParenthesizedExpression → expression` recursivamente (linhas 83-91);
  4. se `expected` é `RecordType` com exatamente 1 campo posicional, `actual` não é record, e a expressão é `ParenthesizedExpression`, e o tipo do campo é atribuível a `actual`: `RECORD_LITERAL_ONE_POSITIONAL_NO_TRAILING_COMMA` **no parêntese inteiro** (sem `getErrorNode`) e para (linhas 93-108);
  5. para `ARGUMENT_TYPE_NOT_ASSIGNABLE` o 3º argumento `{2}` é `additionalInfo.join(' ')` (vazio fora de records; linhas 109-149);
  6. senão relata `errorCode` com `[actual, expected]` (DartType → `getDisplayString()` do ErrorReporter, com alias preferido).
- Atribuibilidade (`TypeSystemImpl.isAssignableTo`): `dynamic` atribuível a tudo (sem strict-casts); subtipo; **sem** conversão implícita de `call` — esta é feita antes, na inferência (o tipo da expressão já passa a ser o da tear-off `call`, ver `invalid_assignment`).

##### `argument_type_not_assignable` (perda 160: FN 86, FP 5, msg 59, pos 10)
- **Emissão:** `_checkForArgumentTypeNotAssignableForArgument` (`analyzer/lib/src/generated/error_detection_helpers.dart:348-368`) → `checkForArgumentTypeNotAssignable` → `checkForAssignableExpressionAtType`. Chamado de:
  - argumentos de invocações: `checkForArgumentTypesNotAssignableInList` (`analyzer/lib/src/generated/resolver.dart:511-520`, um por argumento com `staticParameterElement`), feito pelos resolvedores de `MethodInvocation`, `FunctionExpressionInvocation`, `InstanceCreationExpression` (`analyzer/lib/src/dart/resolver/instance_creation_expression_resolver.dart:72`), redirecionamento/super, anotações e constantes de enum (`analyzer/lib/src/generated/resolver.dart:2556-2559`);
  - operador binário definido pelo usuário: `_resolveUserDefinableElement` + `checkForArgumentTypeNotAssignableForArgument(right)` (`analyzer/lib/src/dart/resolver/binary_expression_resolver.dart:301`);
  - `==`/`!=`: `binary_expression_resolver.dart:117-124` com `promoteParameterToNullable: true` (o parâmetro de `operator ==` é tornado anulável: `'A?'`);
  - atribuição composta `a op= b` (exceto `=` e `??=`): o lado direito contra o parâmetro do operador (`analyzer/lib/src/dart/resolver/assignment_expression_resolver.dart:337-341`);
  - índice: `checkIndexExpressionIndex` (`error_detection_helpers.dart:256-283`) — o índice é checado contra o parâmetro do `[]` (leitura) **e** do `[]=` (escrita) quando ambos existem (composta, `??=`, `++`): dois relatos possíveis no mesmo nó se os dois parâmetros recusam;
  - `++x`/`--x`/`x++`/`x--`: `_checkForIntNotAssignable(operand)` (`analyzer/lib/src/generated/error_verifier.dart:3899-3906`, chamado em `visitPrefixExpression` :1335-1344 e `visitPostfixExpression` :1311-1319): o tipo `int` (o `1` implícito) contra `operand.staticParameterElement` = 1º parâmetro do `operator +`/`-` resolvido (`analyzer/lib/src/dart/ast/ast.dart:6185-6192`, 14299-14310, 14510-14520). **O nó do erro é o operando** (`a` em `++a`, `a` em `a++`), não o `1`.
- **Condição exata:** `!isAssignableTo(argType, paramType)` depois do passo 1 (void) e do 4 (record de 1 campo).
- **Posição:** `getErrorNode(argumento)` — sem parênteses envolventes e, em cascata, o alvo; para argumento nomeado, a expressão (não o rótulo; `NamedExpression.expression`, `error_detection_helpers.dart:62`).
- **Mensagem:** `The argument type '{0}' can't be assigned to the parameter type '{1}'. {2}` — `{0}`,`{1}` DartType (display com alias); `{2}`:
  - só quando **ambos** são `RecordType`: `"Expected N positional arguments, but got M instead."` se `N != 0 && M != N`; `"Expected N named arguments, but got M instead."` se `N != 0 && M != N`; para cada campo nomeado do atual que não exista no esperado com mesmo nome **e mesmo tipo** (igualdade, não subtipo): ``"Unexpected named argument `x` with type `T`."`` (T com `getDisplayString()`); juntados com espaço (`error_detection_helpers.dart:109-139`);
  - fora disso, string vazia — a mensagem termina em `'. '` (ponto, espaço).
  - Records: campos nomeados exibidos **em ordem alfabética** (o `RecordType` normaliza a ordem): `'({int bb, int c})'`.
- **Supressões e ordem:** `void` antes (USE_OF_VOID_RESULT); record de 1 campo vira `RECORD_LITERAL_ONE_POSITIONAL_NO_TRAILING_COMMA`; argumento sem parâmetro correspondente (aridade errada) não é checado; tipo do parâmetro `InvalidType` não relata (atribuível).
- **No DartForge:** `chamadas.rs::invocar` (`verificar_atribuivel_expr` com o molde de texto, convertido pela `ponte.rs` com `ULTIMO_OPCIONAL` que acrescenta `{2}` vazio); operadores em `expr.rs::operador_binario_com_membro` (:2218-2240) e índice de escrita em `expr.rs::escrita_indice` (:3083-3095); mensagem em `mod.rs::verificar_atribuivel` (texto) — não há `getErrorNode`, nem `{2}` de records, nem checagem para `==`, `++/--`, composta e índice de leitura.
  Causas no placar base:
  1. **46 FN `operator/equality_static_test.dart` + 2 `binary_eqEq` + 2 `super_equals…`**: `a == b` não checa `b` contra o parâmetro de `operator ==` tornado anulável (`Covar1?`), nem `super == x`.
  2. **8 FN `incrementAnd_*`**: `++a`/`a++` com `operator +(double)`/`(String)` — falta `_checkForIntNotAssignable` no operando.
  3. **4 FN `index_validR` / `map_indexSet_ifNull` / `implicit_downcast_during/indexed_*`**: índice em composta/`??=`/`++` checado só contra `[]=`; falta o `[]` de leitura, e o lado direito da composta contra o parâmetro do operador.
  4. **~49 msg + 10 pos `nnbd/top_merge/*`**: a assinatura combinada de membros herdados de várias interfaces deve ser o **NNBD_TOP_MERGE** dos tipos (`Object?` de `FutureOr<dynamic>`/`Object?`/`void`), e nós pegamos a de uma das interfaces (`analyzer/lib/src/dart/element/inheritance_manager3.dart`, `_topMerge`/`combineSignatures`). As "posições" são o mesmo erro casado com a linha errada.
  5. **4 msg `recordType*`**: falta o `{2}` de records e a ordem alfabética dos campos nomeados no display.
  6. **1 pos + 1 FP `RecordLiteralOnePositional…`**: falta o passo 4 (`RECORD_LITERAL_ONE_POSITIONAL_NO_TRAILING_COMMA`) e o `getErrorNode` (sem o parêntese).
  7. **13 FN `variance/*`**: arquivos com o experimento `variance` (não habilitado); o oráculo ainda resolve `in`/`out`… não vale perseguir.
  8. FP `records/type_inference_error_test.dart:130/137` (inferência de record com `dynamic`), `function/call_via_bound…` e `referenced_before_declaration…localFunction` (tipo de função local usada antes da declaração): casos isolados de inferência.
  **Mudança:** (a) emitir com `aviso_com_codigo(ARGUMENT_TYPE_NOT_ASSIGNABLE, getErrorNode(e), [atual, esperado, info])` com a regra de `{2}`; (b) `RecordType` exibido com nomeados ordenados (em `table.format`); (c) `==`: depois de inferir o operando direito, achar `operator ==` do tipo esquerdo promovido a não nulo e checar contra `nullable(param)`; (d) `++/--`: checar `int` contra o 1º parâmetro do operador resolvido, no span do operando; (e) composta: lado direito contra o parâmetro do operador; índice contra `[]` e `[]=`; (f) top-merge das assinaturas combinadas (fora deste arquivo: `membros.rs`/outline).

##### `invalid_assignment` (perda 126: FN 68, FP 33, msg 14, pos 11)
- **Emissão:**
  - declaração de variável com inicializador: `VariableDeclarationResolver.resolve` → `checkForAssignableExpressionAtType(initializer, type, element.type, INVALID_ASSIGNMENT)` (`analyzer/lib/src/dart/resolver/variable_declaration_resolver.dart:80-87`) — com `getErrorNode` (parênteses/cascata);
  - valor padrão de parâmetro: `ErrorVerifier.visitDefaultFormalParameter` (`analyzer/lib/src/generated/error_verifier.dart:640-652`) — idem;
  - atribuição `=`/composta/`??=`: `AssignmentExpressionResolver._checkForInvalidAssignment(writeType, right, assignedType)` (`analyzer/lib/src/dart/resolver/assignment_expression_resolver.dart:117-155`; chamado em :331-336) — erro **no `rightHandSide` sem `getErrorNode`** (o parêntese conta); `assignedType` é o tipo do lado direito para `=`, o **retorno do operador** para composta (`i += d` com `int + double → double`) e `LUB` para `??=`;
  - `++x`/`x++`: `_checkForInvalidAssignmentIncDec` (`analyzer/lib/src/dart/resolver/prefix_expression_resolver.dart:96-110`, `postfix_expression_resolver.dart:73-88`) — o retorno do operador `+`/`-` contra o `writeType` do operando, **no nó inteiro** (`++a`, `a++`); nos `?.` o tipo do resultado é anulável (`E?`).
  - também pelo conversor do CFE (`analyzer/lib/src/fasta/error_converter.dart:184-190`), só para erros de parser — irrelevante.
- **Condição exata:** void primeiro (`USE_OF_VOID_RESULT`), record de 1 campo (`RECORD_LITERAL_ONE_POSITIONAL_NO_TRAILING_COMMA`), senão `!isAssignableTo(right, write)`.
- **Posição:** acima; inicializador/padrão sem parênteses e cascata; atribuição no lado direito como escrito.
- **Mensagem:** `A value of type '{0}' can't be assigned to a variable of type '{1}'.` — DartTypes; quando dois tipos exibem o mesmo nome e são elementos diferentes, o ErrorReporter acrescenta `' (where C is defined in <uri>)'` a cada um (`analyzer/lib/error/listener.dart:362-425`, `_convertTypeNames`: só quando dois argumentos DartType da mesma mensagem têm o mesmo display e elementos distintos com o mesmo nome; o caminho é `source.fullName`): `'C (where C is defined in <raiz>/regress/regress1363_lib.dart)'`.
  Tear-off implícita de `call`: quando o valor é de classe com `call` e o contexto é tipo de função, a inferência insere `.call` (`ImplicitCallReference`); o `{0}` é o tipo da tear-off (`'void Function(int)'`, genérico instanciado se `constructor-tearoffs` habilitado — senão o genérico cru, `'T Function<T>(T)'`).
- **Supressões e ordem:** inicializador de **campo** em construtor não é este código (`FIELD_INITIALIZER_NOT_ASSIGNABLE`/`CONST_FIELD_INITIALIZER_NOT_ASSIGNABLE`, `error_detection_helpers.dart:160-...`); atribuição a método (`ASSIGNMENT_TO_METHOD`) não tem `writeType` → não relata; `writeType` dynamic/inválido não relata.
- **No DartForge:** `expr.rs::atribuicao` (:2800-2830, só `=`), `instrucoes.rs:643` (variável local), `mod.rs:418` (topo/campo), `funcoes.rs:157` (inicializador de campo — **errado**: deveria ser `FIELD_INITIALIZER_NOT_ASSIGNABLE`). Mensagem via `mod.rs::verificar_atribuivel` (texto + ponte).
  Causas:
  1. **27 FN `operator/number_operator_error_test.dart` + 3 `implicit_downcast_during/compound_assignment` + 1 `if_null_assignment` + `InvalidAssignment__ifNullAssignment`**: composta e `??=` não checam o tipo resultante (`retorno do operador`/LUB) contra o `writeType`.
  2. **4 FN `prefix/postfixExpression_*`** + **4 msg `null_aware/increment_decrement_test`** (`E` vs `E?`): falta `_checkForInvalidAssignmentIncDec` e, em `?.`, o tipo anulável.
  3. **2 FN `defaultValue_*`**: valor padrão de parâmetro não checado.
  4. **8 FP + 6 msg `ImplicitCallReference_*`/`callable_test`/`method_implicit_tear_off…`**: falta a conversão implícita de `call` na inferência (tipo atual deve ser a tear-off, e quando ela é atribuível não há erro).
  5. **6 FN `generic/function_subtype_parametrized_typedef_test`**: subtipo entre typedefs de função parametrizados (`H<B>` vs `H<A>`), provavelmente variância do parâmetro do typedef.
  6. **FP `field_initializer_not_assignable/*`, `const_field_initializer…`, `implicit_downcast_during/constructor_initializer_test`**: código errado no inicializador de campo.
  7. **FP `assignment_to_method/*` (4)**: atribuição a método relata também `INVALID_ASSIGNMENT`.
  8. **5 FP `spread_collections/null_spread_test`**: `[...?x]` com `x: X extends Null` deve ter elemento `Never` (o tipo do spread de `Null`/`Never`), nós deduzimos `dynamic`.
  9. **2 FP + 1 pos `compile_time_constant/static2/3_test`, 3 FP `prefix_shadowed…`, 2 FP `RecordLiteralOnePositional…`**: respectivamente erro em contexto já inválido, nome sombreado, e passo 4.
  10. **pos `cascadeExpression`/`parenthesizedExpression`/`null_aware/assignment_test` (6)**: falta `getErrorNode` nas declarações e, em `?.` atribuição, o span.
  11. **msg `regress1363_test`**: falta a desambiguação `(where C is defined in …)`.
  **Mudança:** completar os quatro pontos de emissão com as regras acima, emitir com `aviso_com_codigo`, trocar o código do inicializador de campo, e implementar a tear-off implícita de `call` na inferência de expressões com contexto de função.

##### `argument_type_not_assignable_to_error_handler` (perda 33: FN 33)
- **Emissão:** `ErrorHandlerVerifier.verifyMethodInvocation` (`analyzer/lib/src/error/error_handler_verifier.dart:43-150`), chamado do `BestPracticesVerifier.visitMethodInvocation` (`analyzer/lib/src/error/best_practices_verifier.dart:630`) — fase de avisos (WarningCode).
- **Condição exata:** com alvo `t` não nulo e argumentos não vazios:
  - `t.catchError(cb, …)` com `t` do tipo `Future` (`isDartAsyncFuture`) e 1º argumento posicional: `_checkFutureCatchErrorOnError` → `_checkErrorHandlerFunctionType(cb, tipo, FutureOr<T>)`;
  - `t.then(…, onError: cb)` (Future), `Stream.listen(…, onError: cb)`: com `checkFirstParameterType` só se `cb.expression is FunctionExpression`, retorno esperado `void`;
  - `Stream.handleError(cb)`, `StreamSubscription.onError(cb)`: 1º posicional, `checkFirstParameterType: cb is FunctionExpression`, retorno `void`.
  - `_checkErrorHandlerFunctionType` (:160-213) relata se: sem parâmetros; 1º parâmetro nomeado; (se checa) `Object` não é subtipo do tipo do 1º; com 2 parâmetros, o 2º nomeado ou `StackTrace` não subtipo dele; mais de 2 parâmetros.
  - Só quando o tipo estático do callback é `FunctionType`.
- **Posição:** a expressão do callback (`NamedExpression` inteiro para `onError:`, pois `callback` é o argumento nomeado? — não: em `then`/`listen` o nó passado é o `NamedExpression` (`callback`), então o span **inclui o rótulo** `onError: …`); nos posicionais, a expressão.
- **Mensagem:** `The argument type '{0}' can't be assigned to the parameter type '{1} Function(Object)' or '{1} Function(Object, StackTrace)'.` — `{0}` o tipo de função do callback, `{1}` `FutureOr<T>` (catchError) ou `void`.
- **Supressões e ordem:** independe de `ARGUMENT_TYPE_NOT_ASSIGNABLE` (que normalmente não sai porque `onError` é `Function`).
- **No DartForge:** não existe. **Mudança:** implementar no fim da invocação de método (`chamadas.rs`, ramo `Busca::Achado` com `m.metodo`), com os tipos já inferidos dos argumentos.

##### `return_of_invalid_type` (perda 17: FN 13, msg 4)
- **Emissão:** `ReturnTypeVerifier._checkReturnExpression` (`analyzer/lib/src/error/return_type_verifier.dart:134-275`), de `verifyReturnStatement`/`verifyExpressionFunctionBody` (ErrorVerifier).
- **Condição exata:** pula se o retorno declarado é ilegal (`hasLegalReturnType`) ou gerador. Síncrono: `T` void e `S` ∉ {void, dynamic, Null}; `S` void e `T` ∉ {void, dynamic}; senão (record de 1 campo → `RECORD_LITERAL_…`) `!isAssignableTo(S, T)`. Assíncrono: `T_v = futureValueType(T)`, `flatten(S)`; mesmas regras com `T_v`/`flatten(S)`, e o último teste é `!isAssignableTo(S, T_v) && !isSubtypeOf(flatten(S), T_v)`.
- **Posição:** a expressão retornada (sem desembrulhar parênteses).
- **Mensagem:** variantes por tipo de executável: `…_FROM_CONSTRUCTOR` (`'{2}'` = `displayName` do construtor: `C` ou `C.named`), `…_FROM_FUNCTION` (função de topo **ou local**), `…_FROM_METHOD`; `A value of type '{0}' can't be returned from the {function|method|constructor} '{2}' because it has a return type of '{1}'.`; `RETURN_OF_INVALID_TYPE_FROM_CATCH_ERROR` (WarningCode) quando dentro do `onError` de `catchError`.
- **Supressões e ordem:** closures são o código `…_FROM_CLOSURE`.
- **No DartForge:** `funcoes.rs::verificar_retorno_de_expressao` e `instrucoes.rs` (return). Causas: **9 FN `constructor_body/*`** são de sintaxe nova (corpos de construtor secundário com `return null`, oráculo 3.13 — construtores de classe/enum/extension type com `return expr`: o retorno `Null` contra o tipo da classe); **2 FN `enum/enhanced_enums_error_test`** (retorno em construtor que conflita com membro); **1 `NotMapSpread…`** (`{...iterable}` em contexto de Map infere `Set<int>`); **1 `UndefinedGetter__propertyAccess_functio…`** (`Function` retornado de `void`). **4 msg `extension_type/combined_member_signature_error_test`**: tipo combinado `(Object?, Object?)` vs nosso `(Object?, dynamic)` — mesmo top-merge do `argument_type_not_assignable`.
  **Mudança:** retornos em corpos de construtor (para 3.13: só quando a sintaxe nova está ativa), top-merge, e o caso `{...x}` em contexto de mapa.

##### `return_of_invalid_type_from_closure` (perda 17: FN 16, msg 1)
- **Emissão:** `return_type_verifier.dart:160-166` (`enclosingExecutable.isClosure`).
- **Condição exata:** a mesma do anterior; `T` é o **tipo de retorno inferido/imposto da closure** (para `() async {…}` em contexto `void Function()` é `Future<void>`; `T_v = void`).
- **Posição:** a expressão retornada.
- **Mensagem:** `The returned type '{0}' isn't returnable from a '{1}' function, as required by the closure's context.` — `{1}` é `T` (ex.: `'Future<void>'`).
- **No DartForge:** `funcoes.rs` (closure: `RETURN_OF_INVALID_TYPE_FROM_CLOSURE` existe para síncronas). Causa das **16 FN `invalid_returns/async_invalid_return_*`**: closures `async` não são checadas (regras com `flatten`/`futureValueType`). **1 msg**: top-merge.
  **Mudança:** aplicar as regras assíncronas às closures `async` com `T` = retorno da closure (`Future<flatten(contexto)>`).

##### `list_element_type_not_assignable` (perda 14: FN 14)
- **Emissão:** `ErrorVerifier._checkForListElementTypeNotAssignable` (`analyzer/lib/src/generated/error_verifier.dart:4097-4122`) para **todo** `ListLiteral` (const ou não) → `LiteralElementVerifier` (`analyzer/lib/src/error/literal_element_verifier.dart:52-66` elementos; :176-262 spread). Para literais const também o `ConstantVerifier` (`analyzer/lib/src/dart/constant/constant_verifier.dart:1205-1213`) quando o valor avaliado não casa (duplicata possível no mesmo nó).
- **Condição exata:** tipo do elemento = `typeArguments[0]` do tipo **estático** do literal. Por elemento: expressão → void primeiro, depois `!isAssignableTo(tipo, E)`; `if`/`for` → recursivo nos corpos (`then` e `else`); `...x` → se `x` dynamic/Never/Null(com `?`) nada; senão `Iterable<T>` de `x` e `!isAssignableTo(T, E)` (com a tear-off de `call` como exceção); elemento null-aware `?e` (3.8+) → o tipo promovido a não nulo.
- **Posição:** o elemento (expressão) ou a expressão do spread.
- **Mensagem:** `The element type '{0}' can't be assigned to the list type '{1}'.`
- **No DartForge:** só o verificador de constantes (`crates/types/src/constantes/verificador.rs:1055`). **Todas as 14 FN** são literais não const (ou const com o ramo `if` não avaliado). **Mudança:** em `colecoes.rs`, depois de inferir o literal, percorrer os elementos com a regra do `LiteralElementVerifier` (usar o tipo final do literal); evitar duplicata com o relato de constante (mesmo código, mesmo nó: emitir uma vez).

##### `map_key_type_not_assignable` (perda 12: FN 12)
- **Emissão:** `LiteralElementVerifier._verifyMapLiteralEntry` (`literal_element_verifier.dart:121-155`) e `_verifySpreadForMap` (:266-325), via `ErrorVerifier._checkForMapTypeNotAssignable`; const: `constant_verifier.dart:1286-1292`.
- **Condição exata:** void primeiro (chave e valor); chave com `?` (null-aware) é promovida a não nula; `!isAssignableTo(K, mapKeyType)`. Spread de mapa: `Map<K, V>` de `x`.
- **Posição:** a chave (ou a expressão do spread).
- **Mensagem:** `The element type '{0}' can't be assigned to the map key type '{1}'.`
- **No DartForge:** só constantes. Causas: as 12 FN são literais não const / ramos não avaliados / spreads / chaves null-aware. **Mudança:** com `list_element…`, em `colecoes.rs`.

##### `map_value_type_not_assignable` (perda 12: FN 12)
- Igual ao anterior para o valor (`literal_element_verifier.dart:157-170`, spread :312-323; const `constant_verifier.dart` logo abaixo da chave). **Mensagem:** `The element type '{0}' can't be assigned to the map value type '{1}'.` **Posição:** o valor. Causa e mudança idênticas.

##### `set_element_type_not_assignable` (perda 7: FN 7)
- Igual a `list_element_type_not_assignable` com `forSet` (`literal_element_verifier.dart:58-61`; `ErrorVerifier._checkForSetElementTypeNotAssignable3` em `error_verifier.dart:5275-...`; const `constant_verifier.dart:1360-1366`). **Mensagem:** `The element type '{0}' can't be assigned to the set type '{1}'.` Causa e mudança idênticas.

##### `extension_override_argument_not_assignable` (perda 4: FN 4)
- **Emissão:** `ExtensionMemberResolver.resolveOverride` → bloco final (`analyzer/lib/src/dart/resolver/extension_member_resolver.dart:227-243`).
- **Condição exata:** receptor `void` → `USE_OF_VOID_RESULT` no receptor; senão `!isAssignableTo(receiverType, extendedType)` com `extendedType` já substituído pelos argumentos de tipo (explícitos ou inferidos).
- **Posição:** a expressão do argumento (`E(arg)` → `arg`).
- **Mensagem:** `The type of the argument to the extension override '{0}' isn't assignable to the extended type '{1}'.` (`{0}` tipo do argumento, `{1}` tipo estendido).
- **No DartForge:** `chamadas.rs::chamada` (ramo `RefTipo::Extensao`, :515-539) infere o argumento e não checa. **Mudança:** após `extensao_aplicavel`, checar atribuibilidade ao `on` substituído e relatar no argumento.

##### `await_of_incompatible_type` (perda 16: FN 16)
- **Emissão:** `ErrorVerifier.visitAwaitExpression` → `_checkForAwaitOfIncompatibleType` (`analyzer/lib/src/generated/error_verifier.dart:2214-2223`).
- **Condição exata:** `isIncompatibleWithAwait(T)` (`analyzer/lib/src/dart/element/type_system.dart:971-1001`): `S?` → recursivo em `S`; extension type que **não** é subtipo de `Future<Object?>`; parâmetro de tipo promovido `X & B` → recursivo em `B`; parâmetro de tipo com limite `S` → recursivo em `S`.
- **Posição:** o token `await`.
- **Mensagem:** `The 'await' expression can't be used for an expression with an extension type that is not a subtype of 'Future'.` (sem argumentos).
- **Supressões e ordem:** sai junto com `AWAIT_IN_WRONG_CONTEXT`/`USE_OF_VOID_RESULT` se houver (independentes).
- **No DartForge:** `expr.rs` (`ExprKind::Await`, :1070). **Mudança:** checar com a função recursiva e relatar no `await` (span de 5 caracteres no início da expressão).

##### `throw_of_invalid_type` (perda 16: FN 16)
- **Emissão:** `ErrorVerifier.visitThrowExpression` → `_checkForThrowOfInvalidType` (`analyzer/lib/src/generated/error_verifier.dart:5336-5347`); antes, `checkForUseOfVoidResult(expression)` (:1551).
- **Condição exata:** `!isAssignableTo(type, Object)` (não anulável). `dynamic` passa; `void` não é atribuível a `Object` → relata **também** (os dois: `USE_OF_VOID_RESULT` e `THROW_OF_INVALID_TYPE 'void'`, como no oráculo `UseOfVoidResult__throwExpression…`).
- **Posição:** a expressão lançada.
- **Mensagem:** `The type '{0}' of the thrown expression must be assignable to 'Object'.`
- **No DartForge:** `expr.rs` (`ExprKind::Throw`, :1076) não checa. **Mudança:** checar `atribuivel(t, Object)` e relatar na expressão.

##### `for_in_of_invalid_type` (perda 17: FN 17)
- **Emissão:** `ErrorVerifier._checkForEachParts` (`analyzer/lib/src/generated/error_verifier.dart:3103-3170`), de `visitForEachPartsWithDeclaration`/`WithIdentifier`; e `patternForInExpressionIsNotIterable` (`analyzer/lib/src/dart/resolver/shared_type_analyzer.dart:161-171`) para `for (var (a, b) in x)`.
- **Condição exata:** iterável `void` → `USE_OF_VOID_RESULT` e para; `dynamic` só com strict-casts; **tipo anulável (`isNullable`) → para** (o erro é o `UNCHECKED_USE_OF_NULLABLE_VALUE`); variável sem elemento → para; `T = resolveToBound(tipo)` (um parâmetro de tipo `SQ extends Stream<Object?>?` vira `Stream<Object?>?`, e aí relata!); topo → requerido; `!isAssignableTo(T, Iterable<dynamic>/Stream<dynamic>)` → relata.
- **Posição:** a expressão iterável.
- **Mensagem:** `The type '{0}' used in the 'for' loop must implement '{1}'.` — `{0}` o tipo **depois** de `resolveToBound`; `{1}` a string literal `'Iterable'` ou `'Stream'` (com `await`).
- **No DartForge:** `instrucoes.rs::cabecalho_for_in` (:703) não relata; o mesmo para `for` em coleções (`colecoes.rs`, `CollectionElement::ForIn`). **Mudança:** implementar a regra no cabeçalho de `for-in` de instrução e de elemento de coleção.

##### `for_in_of_invalid_element_type` (perda 14: FN 14)
- **Emissão:** mesma função (`error_verifier.dart:3172-3233`).
- **Condição exata:** com o tipo da sequência `Iterable<E>`/`Stream<E>` (`asInstanceOf`), `!isAssignableTo(E, tipo da variável)` — exceto se a tear-off implícita de `call` de `E` é atribuível.
- **Posição:** a expressão iterável.
- **Mensagem:** `The type '{0}' used in the 'for' loop must implement '{1}' with a type argument that can be assigned to '{2}'.` — `{0}` tipo do iterável (resolvido), `{1}` `'Iterable'`/`'Stream'`, `{2}` tipo da variável (declarada, ou a do identificador/variável existente no `for (x in …)`).
- **No DartForge:** inexistente. **Mudança:** junto com o anterior.

##### `for_in_with_const_variable` (perda 1: FN 1)
- **Emissão:** `ErrorVerifier.visitForEachPartsWithDeclaration` (`analyzer/lib/src/generated/error_verifier.dart:912-923`), só se `_checkForEachParts` devolveu `true`.
- **Condição exata:** variável do laço declarada `const`.
- **Posição:** o token `const`.
- **Mensagem:** `A for-in loop variable can't be a 'const'.`
- **No DartForge:** inexistente (o parser aceita `const` no `for`?). **Mudança:** em `instrucoes.rs::cabecalho_for_in`, se o alvo declarado é `const`, relatar no token.

##### `async_for_in_wrong_context` (perda 3: FN 3)
- **Emissão:** erro do **parser** do CFE (`codes_generated.dart:366`, mensagem `AsyncForInWrongContext`) convertido em `analyzer/lib/src/fasta/error_converter.dart:34-38` (`CompileTimeErrorCode.ASYNC_FOR_IN_WRONG_CONTEXT`).
- **Condição exata:** `await for` dentro de função que não é `async`/`async*` (o parser sabe o modificador do corpo).
- **Posição:** o token `await` do `await for` (offset do `await`, length 5) — ver oráculo `2:3`.
- **Mensagem:** `The async for-in loop can only be used in an async function.`
- **No DartForge:** inexistente. **Mudança:** no parser ou em `instrucoes.rs` (com o modificador da função corrente: `cx.funcoes.last().modificador`).

##### `record_literal_one_positional_no_trailing_comma` (perda 4: FN 4)
- **Emissão:** `error_detection_helpers.dart:93-108`, `assignment_expression_resolver.dart:133-146`, `return_type_verifier.dart:218-233` (o `ParserErrorCode` homônimo é de outra situação).
- **Condição exata:** tipo esperado é record de exatamente 1 campo posicional (e nenhum nomeado não importa), atual não é record, a expressão é `ParenthesizedExpression`, e o tipo do campo é atribuível ao tipo atual.
- **Posição:** o parêntese inteiro `(x)`.
- **Mensagem:** `A record literal with exactly one positional field requires a trailing comma.`
- **Supressões:** substitui o `ARGUMENT_TYPE_NOT_ASSIGNABLE`/`INVALID_ASSIGNMENT`/`RETURN_OF_INVALID_TYPE` daquele ponto.
- **No DartForge:** existe em algum ponto (2 acertos), falta nas três emissões comuns. **Mudança:** acrescentar o passo em `verificar_atribuivel_expr` (com o tipo esperado record) — e nos retornos.

##### `must_return_void` (perda 1: FN 1)
- **Emissão:** `FfiVerifier._validateNativeCallable…` (`analyzer/lib/src/generated/ffi_verifier.dart:1830-1838`): `NativeCallable.listener(f)` com tipo nativo de retorno ≠ `Void`.
- **Posição:** o argumento `f`. **Mensagem:** `The return type of the function passed to 'NativeCallable.listener' must be 'void' rather than '{0}'.` (`{0}` o tipo nativo de retorno, ex. `Int32`).
- **No DartForge:** inexistente (FFI; `crates/analise/src/nativos.rs` se for implementado).

##### `await_in_late_local_variable_initializer` (perda 2: FN 2)
- **Emissão:** `ErrorVerifier._checkForAwaitInLateLocalVariableInitializer` (`analyzer/lib/src/generated/error_verifier.dart:2205-2212`), de `visitAwaitExpression`.
- **Condição exata:** `await` dentro do inicializador de variável **local** `late` (pilha `_isInLateLocalVariable`, empilhada em `visitVariableDeclarationList` para locais `late` e desempilhada nas expressões de função — closures dentro do inicializador não contam).
- **Posição:** o token `await`. **Mensagem:** `The 'await' expression can't be used in a 'late' local variable's initializer.`
- **No DartForge:** inexistente. **Mudança:** em `instrucoes.rs` (declaração local `late`) marcar o contexto e relatar em `expr.rs` no `await`.

#### Grupo 2 — membros indefinidos, invocações e `void`

Base comum: a resolução de membro passa por `TypePropertyResolver.resolve` (`analyzer/lib/src/dart/resolver/type_property_resolver.dart`), que devolve `getter`/`setter` e os sinalizadores `needsGetterError`/`needsSetterError` (falsos quando houve outro erro já relatado: receptor anulável → `UNCHECKED_…`, extensão ambígua → `AMBIGUOUS_EXTENSION_MEMBER_ACCESS`, `dynamic`/`Never`/`InvalidType`). Regra de ouro dos argumentos: quando o argumento é `String` (nome de classe) sai sem argumentos de tipo; quando é `DartType` sai com `getDisplayString()` (com argumentos de tipo, alias preferido, `?`).

##### `undefined_method` (perda 87: FN 56, FP 9, msg 22)
- **Emissão:** `MethodInvocationResolver` (`analyzer/lib/src/dart/resolver/method_invocation_resolver.dart`):
  - `_resolveReceiverType` (:795-885): instância (`x.m()`), implícito `m()` dentro de classe (via `_resolveReceiverNull` → `thisType` não nulo, :640-655), cascata;
  - `_resolveReceiverNull` com setter de topo/extensão/estático e sem getter (:610-647: `noGetterIsPossible`);
  - `_resolveReceiverTypeLiteral` (:890-939): `C.m()` estático (classe, ou alias cujo `aliasedType` é interface — `Object`, `FutureOr`? não: só `InterfaceType`; `FutureOr`/`Null` aliasados viram `InterfaceType` do `FutureOr`/`Null`);
  - `FunctionReferenceResolver` (`analyzer/lib/src/dart/resolver/function_reference_resolver.dart:715-722`): tear-off `C.m<int>` indefinido (argumento DartType).
- **Condição exata:** sem getter (método/getter) no tipo e sem extensão aplicável, `needsGetterError`, e o nome não sintético. O alvo é resolvido **antes**: receptor `void` → `USE_OF_VOID_RESULT` no receptor; `dynamic`/`Never` → nada; receptor anulável com o membro só no não nulo → `UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE`. Se o membro achado é getter/campo, vira `FunctionExpressionInvocation` (não é este código).
- **Posição:** o nome do método (`methodName`).
- **Mensagem:** `The method '{0}' isn't defined for the type '{1}'.` — `{1}` é **String**:
  - instância/implícito (`_resolveReceiverType` :867-873, `_resolveReceiverNull` :631-635): `InterfaceType → element.name` (**sem argumentos de tipo**: `'C'`, não `'C<String>'`; `'Map'`, `'Pointer'`), `FunctionType → 'Function'`, outros (record, parâmetro de tipo, …) → `'<unknown>'`;
  - estático (`_resolveReceiverTypeLiteral` :935): `receiver.displayName` = o nome da classe (o **elemento** aliasado: `typedef T = Object` → `'Object'`; `typedef N = Null` → `'Null'`; `FutureOr` → `'FutureOr'`).
  - variante `UNDEFINED_METHOD_ON_FUNCTION_TYPE` (`'{0}' isn't defined for the '{1}' function type`, :169-182): receptor é literal de tipo de alias de função com argumentos de tipo (`Fn<int>.m()`), `{1}` = `qualifiedName` do alias.
  - `C.new()` sem construtor sem nome: `NEW_WITH_UNDEFINED_CONSTRUCTOR_DEFAULT` em vez deste (:917-929).
- **Supressões e ordem:** `C.m()` com `m` de instância → `STATIC_ACCESS_TO_INSTANCE_MEMBER`; extensão estática (`E.m()`) → `UNDEFINED_EXTENSION_METHOD`; prefixo de import (`p.f()`) → `UNDEFINED_FUNCTION` (abaixo); `super.m()` → `UNDEFINED_SUPER_METHOD`.
- **No DartForge:** `chamadas.rs::chamada`, ramo `Busca::Ausente` (:718-745) — texto `"… para o tipo '{tipo formatado}'"` (formata o DartType inteiro); estático (`C.m()`, :557-584) passa por `inferir(target)` e sai como `UNDEFINED_GETTER` sem `{1}`; índice `[]=` indefinido sai como `UNDEFINED_METHOD` em `expr.rs::escrita_indice` (:3096-3100).
  Causas:
  1. **22 msg**: `{1}` deve ser o nome do elemento (`'C'`, `'Map'`, `'Pointer'`), `'Function'` para tipos de função (`Comparator<dynamic>` → `'Function'`, `void Function()` → `'Function'`), `'<unknown>'` para record; estático via alias → o elemento aliasado (`'FutureOr'`, `'Null'`).
  2. **~20 FN estáticos** (`mixin/illegal_static_access`, `mixin_declaration_constructor_error`, `nonfunction_type_aliases/*`, `UndefinedMethod__typeAlias_interfaceType`, `…static_mixinApplicatio…`, `…method_undefined_enum`, `null_aware/invocation_test` `C?.toString()`): saem como `undefined_getter` com `{1}` vazio (FP lá). **Mudança:** em `chamada`, ramo `referencia_a_tipo(recv)`, quando não há membro estático: se `name == new` → `NEW_WITH_UNDEFINED_CONSTRUCTOR_DEFAULT [nome da classe]` no `new`; senão `UNDEFINED_METHOD [m, nome do elemento]` no nome, sem inferir o alvo como propriedade.
  3. **9 FP `[]=`/`call`**: `a[i] = v` com `[]=` ausente é `UNDEFINED_OPERATOR ['[]=', tipo]` no `[...]` (ver `undefined_operator`); `f.call()`/`f()` com `f: Function` não relata (o tipo `Function` tem `call` dinâmico, `property_element_resolver.dart:440-446`).
  4. **FN restantes**: `constructor/reference_test` (5, `C.m` sobre construtor), `extension_methods/static_extension_internal_basename_shadowing` (6, nome de extensão sombreado por membro), `this_promotion/*` (5), `type_variable/conflict2`, `private/member3`, `UndefinedMethod__localSetterShadowingEx…` (`noGetterIsPossible`: setter de topo/estático/de extensão no escopo sem getter → `UNDEFINED_METHOD` com `thisType`), `…extensionMethodHidden/Shadowi…`, `…functionAlias_typeInstantiated` (variante `_ON_FUNCTION_TYPE`).
  **Mudança:** emitir com `aviso_com_codigo(UNDEFINED_METHOD, name.span, [m, nome_para_mensagem(r_ty)])` com a função de nome acima.

##### `undefined_getter` (perda 79: FN 42, FP 31, msg 6)
- **Emissão:** `PropertyElementResolver._resolve` (`analyzer/lib/src/dart/resolver/property_element_resolver.dart:383-540`; relato :505-512) para `x.g`, `x?.g`, cascata, implícito `g` em classe (`resolver.dart:1580-1590` via `SimpleIdentifierResolver`); estático `_resolveTargetInterfaceElement` (:640-705, relato :697-704).
- **Condição exata:** sem getter/método, `needsGetterError`; `x.call` com `x` de tipo função ou `Function` **não** relata (:440-446); receptor `void` → `USE_OF_VOID_RESULT` no nome (:448-453); tipo de alias de função com argumentos de tipo → `UNDEFINED_GETTER_ON_FUNCTION_TYPE` (:459-476). Estático: sem getter estático e sem método estático acessível → `UNDEFINED_GETTER` (ou `UNDEFINED_ENUM_CONSTANT` se o tipo é enum).
- **Posição:** o nome da propriedade.
- **Mensagem:** `The getter '{0}' isn't defined for the type '{1}'.` — instância: `{1}` = **DartType** do alvo (promovido a não nulo em `?.`), com argumentos de tipo; estático: `{1}` = `typeReference.name` (String, nome da classe/mixin).
- **Supressões e ordem:** ver base; em atribuição composta lê o getter (relata getter e, se faltar o setter, `UNDEFINED_SETTER` também).
- **No DartForge:** `expr.rs::propriedade`/`membros.rs`. Causas:
  1. **~25 FP com `{1}` vazio**: chamadas estáticas indefinidas (`C.m()`) — mesma causa do item 2 de `undefined_method`; e `E.TWO` de enum (`UNDEFINED_ENUM_CONSTANT`).
  2. **6 msg** (`static_undefined`, `static_definedInSuperclass`, `typeLiteral_conditionalAccess`, `compoundAssignment…`, `null_aware/access_test`): estático sem `{1}` — falta o nome da classe.
  3. **5 FP `call`**: `f.call` com `f: Function`/tipo de função não relata.
  4. **4 FP `ffi_address_of_cast/*`**: `.address` de extensão FFI em elementos (`array[i].address`) — o membro existe por extensão do `dart:ffi` (`int`? receptor deveria ser o tipo nativo) — inferência de FFI.
  5. **26 FN `anonymous_methods/*`**: sintaxe nova (oráculo 3.13); **6 FN `static_extension_getter_setter_conflicts`**: getter ausente quando só há setter na extensão/classe; **2 FN `write_promoted_value_in_switch`**; FN `UndefinedGetter__new_*` (`C.new` como getter em instância/dynamic/cascata), `…functionAlias_typeInst…` (`_ON_FUNCTION_TYPE`), `…extension_this/instance…`.
  **Mudança:** emitir com código; `{1}` instância = tipo formatado; estático = nome da classe; `call` em função; `UNDEFINED_ENUM_CONSTANT` para enum.

##### `use_of_void_result` (perda 44: FN 18, FP 4, pos 22)
- **Emissão:** `checkForUseOfVoidResult` (`analyzer/lib/src/generated/error_detection_helpers.dart:235-253`), chamado de muitos lugares (argumentos, atribuições, condições `bool_expression_verifier.dart:95-106`, `await`/`throw`/`!`/spread/for-in, elementos de coleção, `yield_statement_resolver.dart:45-62`, `record_literal_resolver.dart:150-156`); receptores: `method_invocation_resolver.dart:319` (no **receptor**), `property_element_resolver.dart:87` (índice: no **alvo**? não — `atNode(node.target)`? ver abaixo) e :448-453 (no nome da propriedade), `function_expression_invocation_resolver.dart:115-135`, `assignment_expression_resolver.dart:161-222`, `extension_member_resolver.dart:227-231`.
- **Condição exata:** o tipo estático é **identicamente** `void` (não `FutureOr<void>`); `checkForUseOfVoidResult(e)`: `MethodInvocation` → relata no `methodName`; senão no nó inteiro.
- **Posição:** método → nome do método; `FunctionExpressionInvocation` (variável/getter de tipo função que retorna void) → **a invocação inteira**; acesso `x.foo` com `x` void → o nome `foo`; índice `x[0]` com `x` void (depois de `resolveToBound`) → do `[` ao `]` (`property_element_resolver.dart:80-88` via `_reportUnresolvedIndex`); `x!` → o nó `x!`; record `(one: x,)` → o campo nomeado inteiro `one: x`; `x.foo = null` → `foo`.
- **Mensagem:** `This expression has a type of 'void' so its value can't be used.`
- **Supressões e ordem:** sai **no lugar** de `ARGUMENT_TYPE_NOT_ASSIGNABLE`/`INVALID_ASSIGNMENT` (destino não void); em mapa `{v: v}` a chave void relata e **para** (o valor não é checado — `literal_element_verifier.dart:121-133`); no `throw` sai junto com `THROW_OF_INVALID_TYPE`.
- **No DartForge:** `expr.rs::uso_de_void` (:1142-1169) e vários chamadores. Causas:
  1. **~20 pos + 9 FN `nnbd/top_merge/*`**: top-merge (`void` vs `Object?` combinados) — fora das chamadas.
  2. **4 FN `least_upper_bound_futureor_test`**: `if (x is Future) throw 0;` sobre `FutureOr<void>` deve rebaixar `x` a `void` (fatoração de tipo no fluxo: `factor(FutureOr<void>, Future) = void`) — `fluxo.rs`.
  3. **5 FN** `x.foo = null` (setter em void: no nome, sem `UNDEFINED_SETTER` — que é nosso FP), `x[0]`, `x!`, campos de record.
  4. **FP/pos `type_promotion/functions_test`**: chamada de **variável local** de tipo função (`funcDynToVoid(…)`) é `FunctionExpressionInvocation` → span da invocação inteira; nós usamos o nome (`cx.funcoes_locais` mal classificado).
  5. **FP `mapLiteralEntry_keyAnd…`**: parar depois da chave; **FP `control_flow_collections/void_error_test:92`**, **pos `extensionOverride_argu…`** (`E(v).g` com `v` void: no argumento `v`, `extension_member_resolver.dart:227-231`).
  **Mudança:** os pontos acima; manter `aviso_com_codigo`.

##### `undefined_operator` (perda 40: FN 30, FP 10)
- **Emissão:** binário `BinaryExpressionResolver._resolveUserDefinableElement` (`analyzer/lib/src/dart/resolver/binary_expression_resolver.dart:440-460`, no **operador**); prefixo (`prefix_expression_resolver.dart:180-206`, no token do operador, nome `unary-`/`~`/`+`/`-` para `++`/`--`); pósfixo (`postfix_expression_resolver.dart:130-158`, no operador, `+`/`-`); índice (`property_element_resolver.dart:109-133` + `_reportUnresolvedIndex` :365-381: **do `[` até o `]` inclusive**, `'[]'` para leitura e `'[]='` para escrita — ambos podem sair na composta); composta (`assignment_expression_resolver.dart:240-256`, no operador `+=`); `resolver.dart:1635-1642` (`==` em extensão?).
- **Condição exata:** `needsGetterError` do operador no tipo do operando (lido; em `?.`/`?[` promovido).
- **Posição:** acima.
- **Mensagem:** `The operator '{0}' isn't defined for the type '{1}'.` — `{1}` DartType (com argumentos: `'C1<int>'`).
- **Supressões e ordem:** `super` → `UNDEFINED_SUPER_OPERATOR`; override de extensão → `UNDEFINED_EXTENSION_OPERATOR`; `dynamic`/`Never` nada.
- **No DartForge:** binário em `expr.rs` (:2185-2215, já com código); falta índice (`[]=` sai como `UNDEFINED_METHOD`, `[]` sem relato), prefixo/pósfixo (`++a`, `-a`, `~s`). Causas: **11 FN índice** (`UndefinedOperator__index*`, `static_extension_getter_setter_conflicts` 9, `string_test`, `first_class_types_literals`), **8 FN `++`/`--`/`-`** (`prefix/postfixExpression*`, `string/no_operator_test`), **2 FN `variance/syntax`** (experimento); **10 FP `number_operator_error_test`** (`O extends num?` promovido a `O & int` — o operador deve ser procurado no limite **promovido** `int`, não em `O`).
  **Mudança:** índice com o span `[..]` e os dois nomes; pré/pós-fixos no token; buscar operadores em tipos interseção pelo limite promovido.

##### `undefined_function` (perda 25: FN 25)
- **Emissão:** `MethodInvocationResolver._reportUndefinedFunction` (`analyzer/lib/src/dart/resolver/method_invocation_resolver.dart:296-310`), de `_resolveReceiverNull` (sem elemento no escopo **e** `thisType == null`, :620-629) e `_resolveReceiverPrefix` (`p.f()` sem `f` no prefixo, :718-724); também `library_analyzer.dart:600-608` (`loadLibrary`?).
- **Condição exata:** `f(...)` com `f` não encontrado no escopo léxico e fora de classe/extensão (sem `this`); ou `p.f(...)` com `p` prefixo. Ignorado se `shouldIgnoreUndefined(prefix, name)` (import de URI inexistente com o prefixo, ou nome em `show`… — `analyzer/lib/src/dart/element/scope.dart`/`library_fragment`: import não resolvido torna os nomes desconhecidos silenciosos).
- **Posição:** o nome do método.
- **Mensagem:** `The function '{0}' isn't defined.`
- **Supressões e ordem:** dentro de classe com `this` → `UNDEFINED_METHOD [f, nome da classe]`; nome de classe privada de outra biblioteca (`_Class()`) também cai aqui.
- **No DartForge:** hoje `f()` indefinido sai como `UNDEFINED_IDENTIFIER` (`expr.rs:2737`/`:423`) — é **FP** em `undefined_identifier` (vários dos 95 FP). **Mudança:** em `chamadas.rs::chamada`, ramo `_` com alvo `Identifier` que não resolve: se há `this` (`cx.tipo_this`) → `UNDEFINED_METHOD [nome, nome da classe]`; senão `UNDEFINED_FUNCTION [nome]`; em `p.f()` com prefixo → `UNDEFINED_FUNCTION`; sem relatar `UNDEFINED_IDENTIFIER` no alvo; respeitar `shouldIgnoreUndefined`.

##### `undefined_super_member` (perda 13: FN 13)
- **Emissão:** variantes: `UNDEFINED_SUPER_GETTER` (`property_element_resolver.dart:820-830`), `…_METHOD` (`method_invocation_resolver.dart:780-790`), `…_OPERATOR` (binário :448, pré :197, pós :148, índice :121/:131), `…_SETTER` (`property_element_resolver.dart:866-876`).
- **Condição exata:** membro não achado na superclasse (incluindo mixins aplicados) de `this`; para setter, `super.x = v` sem setter.
- **Posição:** o nome (getter/método/setter); o operador; para índice, `[`…`]`.
- **Mensagem:** `The {getter|method|operator|setter} '{0}' isn't defined in a superclass of '{1}'.` — `{1}` DartType de `this` da classe que contém (`'B'`, `'E'`).
- **No DartForge:** `expr.rs::membro_super` cobre getter/método (15 acertos). Faltam **4 índice `[]=`** e **9 setter** (`super.x = v`). **Mudança:** em `escrita_propriedade`/`escrita_indice` com alvo `super`.

##### `undefined_setter` (perda 11: FN 7, FP 4)
- **Emissão:** `AssignmentVerifier.verify` (`analyzer/lib/src/error/assignment_verifier.dart:28-110`, relato :100-108), chamado de `PropertyElementResolver` quando `needsSetterError`; estático em `_resolveTargetInterfaceElement` (setter estático ausente → `UNDEFINED_SETTER [nome, typeReference.name]`); `UNDEFINED_SETTER_ON_FUNCTION_TYPE` (:470-476).
- **Condição exata:** sem setter; se houver getter final/método no lugar, `AssignmentVerifier` relata outro código (`ASSIGNMENT_TO_FINAL`, `ASSIGNMENT_TO_FINAL_NO_SETTER`, `ASSIGNMENT_TO_METHOD`…).
- **Posição:** o nome. **Mensagem:** `The setter '{0}' isn't defined for the type '{1}'.` (instância DartType; estático nome da classe).
- **No DartForge:** `expr.rs::escrita_propriedade`. **FN**: `C.s = v` estático (2), `x.new = …` (2), `…functionAlias…`, `if_null/assignment_behavior_test`, `regress13494`. **FP**: receptor `void` (deve ser só `USE_OF_VOID_RESULT`), `instance_access_to_static_member` (estático acessado por instância: outro código), `getter/no_setter*_test` (getter sem setter de `this` implícito → `ASSIGNMENT_TO_FINAL_NO_SETTER`).

##### `undefined_extension_operator` (perda 8: FN 8)
- **Emissão:** `binary_expression_resolver.dart:405-414` (e prefixo :160-168, índice `property_element_resolver.dart:52-70`).
- **Condição exata:** `E(x) op y` com o override de extensão sem o operador; para `==` também (`E(x) == y` sem `==` declarado na extensão).
- **Posição:** o operador. **Mensagem:** `The operator '{0}' isn't defined for the extension '{1}'.` (`{1}` nome da extensão).
- **No DartForge:** existe para operadores comuns (13); falta `==` (8 FN em `equality_extension_override_error_test`). **Mudança:** no `==`, quando o operando esquerdo é override de extensão.

##### `ambiguous_extension_member_access` (perda 20: FN 19, msg 1)
- **Emissão:** `ExtensionMemberResolver.findExtension` (`analyzer/lib/src/dart/resolver/extension_member_resolver.dart:87-140`).
- **Condição exata:** mais de uma extensão aplicável com o membro e nenhuma mais específica (`_chooseMostSpecific`).
- **Posição:** a entidade do nome (nome do membro; para operador binário/unário o token do operador; para índice o `[`…`]`? — oráculo `10:3`/`11:15`: o nó do alvo/operador conforme o chamador passa `nameEntity`).
- **Mensagem:** `A member named '{0}' is defined in {1}, and none are more specific.` — `{1}`: lista de `"extension 'E'"` ou `"unnamed extension on 'T'"` (`extendedType.getDisplayString()`), juntadas com `, ` e ` and ` no último (`analyzer/lib/src/dart/resolver/extension_member_resolver.dart:122-135`, `StringUtilities.commaSeparatedWithAnd`?).
- **No DartForge:** `inf.relatar_ambiguidade_de_extensao` (54 acertos). FN: operadores (`+`, `unary-`, `[]`), getter+setter (`getter_setter`), membros em `static_extension_getter_setter_conflicts` e `internal_resolution_4`. **msg**: `'Iterable<InvalidType>'` — tipo inválido exibido como `InvalidType`.
  **Mudança:** consultar a ambiguidade também nos caminhos de operador/índice/atribuição.

##### `invocation_of_non_function` (perda 28: FN 28)
- **Emissão:** `_reportInvocationOfNonFunction` (`method_invocation_resolver.dart:257-270`), de `_resolveReceiverNull` (:575-600) quando o elemento achado no escopo não é executável, variável nem prefixo: **parâmetro de tipo** (`T()`), alias de tipo (`typedef T = dynamic; T()`), `dynamic`; e do tipo literal com elemento não executável (:902-915).
- **Posição:** o nome (`methodName`). **Mensagem:** `'{0}' isn't a function.` (`{0}` o nome).
- **No DartForge:** `chamadas.rs::invocar_valor` deixa de fora (comentário :389-396). **Mudança:** em `chamada`, ramo `Identifier` que resolve a parâmetro de tipo/alias não-classe/`dynamic` → `INVOCATION_OF_NON_FUNCTION [nome]` no nome, inferindo os argumentos livres.

##### `invocation_of_non_function_expression` (perda 12: FN 12)
- **Emissão:** `FunctionExpressionInvocationResolver.resolve` (`analyzer/lib/src/dart/resolver/function_expression_invocation_resolver.dart:72-104`) e `resolver.dart:2015-2025`.
- **Condição exata:** o tipo do alvo não é função/`dynamic`/`Never`/`Function`, e `call` não é achado (`needsGetterError`), **ou** `call` existe mas não é método (getter/campo `call`, :92-101).
- **Posição:** a expressão alvo (`function`). **Mensagem:** `The expression doesn't evaluate to a function, so it can't be invoked.`
- **No DartForge:** `chamadas.rs::invocar_valor` (:386-414) — 9 acertos. FN: `call` como getter (`call/through_getter_test`), extensões sombreadas (`static_extension_internal_basename_shadowing` 6, `issue_45551`), `type_variable/conflict2`, `why_not_promoted/nullable_expression_call` (anulável?), `patterns/invalid_const_pattern`. Hoje excluímos "nome solto que não é local" e "receptor anulável"; rever cada exclusão contra o analyzer (o anulável é `UNCHECKED_INVOCATION_OF_NULLABLE_VALUE`, mas `null` literal/`Null` não).

##### `class_instantiation_access_to_member` (perda 2: FN 1, FP 1)
- **Emissão:** `ConstructorReferenceResolver` (`analyzer/lib/src/dart/resolver/constructor_reference_resolver.dart:55-85`): `C<int>.x` onde `x` não é construtor: estático → `…_STATIC_MEMBER [x]`, instância → `…_INSTANCE_MEMBER [x]`, nada → `…_UNKNOWN_MEMBER [C, x]`.
- **Posição:** o nó `ConstructorReference` inteiro (`C<int>.x`, do início do tipo ao fim do nome; oráculo `265:3` comprimento 8 em `X<X>.any`). Só quando o tipo nomeado é classe (ou alias de interface); alias de função não relata. **No DartForge:** FN `explicit_type_instantiation_parsing_test:265` (`UNKNOWN_MEMBER`); FP `generic_usage_class_error_test:36` (alias genérico para classe com `staticMethod` — o analyzer não relata ali porque o alias instanciado… outro código).

##### `extension_as_expression` (perda 1: FN 1)
- **Emissão:** `SimpleIdentifierResolver` (`analyzer/lib/src/dart/resolver/simple_identifier_resolver.dart:315-323`) e `prefixed_identifier_resolver.dart:172-180`: identificador que resolve a uma extensão usado como expressão (não alvo de acesso). **Posição:** o identificador. **Mensagem:** `Extension '{0}' can't be used as an expression.` **No DartForge:** inexistente; em `expr.rs` (identificador que é extensão).

##### `extension_override_with_cascade` (perda 3: FN 3)
- **Emissão:** `method_invocation_resolver.dart:430-438`, `property_element_resolver.dart:605-613`, `function_reference_resolver.dart:370-376`: `E(x)..m()`/`..g`/`..s = v`. **Posição:** o override `E(x)` (o alvo da cascata). **Mensagem:** `Extension overrides have no value so they can't be used as the receiver of a cascade expression.` **No DartForge:** inexistente.

##### `extension_override_without_access` (perda 1: FN 1)
- **Emissão:** `ExtensionMemberResolver.resolveOverride` (`extension_member_resolver.dart:175-187`): `E(x)` fora de um acesso a membro (`_isValidContext`: alvo de propriedade/método/índice/operador/atribuição), exceto alvo de cascata. **Posição:** o override inteiro. **Mensagem:** `An extension override can only be used to access instance members.` **No DartForge:** `chamadas.rs` (`E(x)` isolado) não relata.

##### `deferred_import_of_extension` (perda 2: FN 2)
- **Emissão:** `ErrorVerifier._checkForDeferredImportOfExtensions` (`analyzer/lib/src/generated/error_verifier.dart:3053-3066`), de `visitImportDirective` para imports `deferred`.
- **Condição exata:** o namespace importado (depois de `show`/`hide`) contém alguma extensão. **Posição:** a URI do import. **Mensagem:** `Imports of deferred libraries must hide all extensions.` **No DartForge:** inexistente; lugar natural `crates/analise/src/importacoes.rs`.

#### Grupo 3 — `this`/`super` em contexto inválido e atribuição a final/const

##### `instance_member_access_from_static` (perda 18: FN 18)
- **Emissão:** `ErrorVerifier._checkForInvalidInstanceMemberAccess` (`analyzer/lib/src/generated/error_verifier.dart:3975-4040`), chamado de `visitSimpleIdentifier`.
- **Condição exata:** (fora de comentário) estamos em inicializador de construtor, ou em método estático (`_enclosingExecutable.inStaticMethod`: método/getter/setter `static` de classe, mixin, enum **ou extensão**), ou em factory, ou em declaração de campo de instância não `late`, ou em declaração de variável estática; o identificador (lido ou escrito: `writeOrReadElement`) resolve a `MethodElement`/`PropertyAccessorElement` **de instância** cujo dono é `InterfaceElement` ou `ExtensionElement`; e não é o nome qualificado de `x.m()`, `x.p` ou `p.x` (só acesso **implícito**). Em método estático → este código.
- **Posição:** o identificador.
- **Mensagem:** `Instance members can't be accessed from a static method.` (sem argumentos).
- **Supressões e ordem:** a precedência é `inStaticMethod` → `INSTANCE_MEMBER_ACCESS_FROM_STATIC`; `inFactoryConstructor` → `…_FROM_FACTORY`; senão `IMPLICIT_THIS_REFERENCE_IN_INITIALIZER`. A resolução de nomes do analyzer **encontra** o membro de instância pelo escopo da classe mesmo em contexto estático (não é `UNDEFINED_IDENTIFIER`).
- **No DartForge:** inexistente. Hoje, num método estático, `expr.rs` resolve o nome implícito… (sem `this`, provavelmente como indefinido ou como membro sem relato). **Mudança:** em `expr.rs` (identificador e alvo implícito de chamada/atribuição), quando o nome resolve a membro de instância da classe/extensão envolvente e o contexto (`CtxFuncao`) é método estático → relatar no identificador; também em `chamadas.rs` para `m()` implícito (o identificador é o `methodName`).

##### `instance_member_access_from_factory` (perda 6: FN 6)
- **Emissão/condição/posição:** a mesma função (`error_verifier.dart:4028-4032`), com `_enclosingExecutable.inFactoryConstructor` (corpo de construtor `factory`, inclusive o redirecionamento? não — só o corpo).
- **Mensagem:** `Instance members can't be accessed from a factory constructor.`
- **No DartForge:** inexistente. **Mudança:** idem, com o contexto de factory.

##### `implicit_this_reference_in_initializer` (perda 17: FN 17)
- **Emissão:** a mesma função (`error_verifier.dart:4033-4038`).
- **Condição exata:** `_isInConstructorInitializer` (lista de inicializadores — campo, `super(...)`, `this(...)`, `assert` — marcado em `visitConstructorFieldInitializer` :619-630, `visitSuperConstructorInvocation` :1447-1452, `visitRedirectingConstructorInvocation` :1365-1369, `visitAssertInitializer` :348-352), ou inicializador de campo de instância **não `late`** (`visitFieldDeclaration` :845-889: `_isInInstanceNotLateVariableDeclaration`), ou inicializador de variável **estática** (`_isInStaticVariableDeclaration`) — e não é método estático nem factory. Inclui membros de instância de **extensão** e chamadas implícitas `m()`/tear-off `m`.
- **Posição:** o identificador.
- **Mensagem:** `The instance member '{0}' can't be accessed in an initializer.` — `{0}` o nome do identificador.
- **Supressões e ordem:** `this.x` explícito é outro código (`INVALID_REFERENCE_TO_THIS`); parâmetros de inicialização (`this.f`) e parâmetros de construtor com o mesmo nome sombreiam o membro (não relata). Campo `late` não relata.
- **No DartForge:** inexistente. **Mudança:** marcar no `Corpo` o contexto "inicializador" (em `funcoes.rs::inicializador` e no inicializador de campo não `late`/estático em `mod.rs`) e relatar em `expr.rs` quando um identificador implícito resolve a membro de instância.

##### `super_in_invalid_context` (perda 20: FN 20)
- **Emissão:** `ElementResolver.visitSuperExpression` (`analyzer/lib/src/generated/element_resolver.dart:404-424`) com `SuperContext.of` (`analyzer/lib/src/generated/super_context.dart`); os resolvedores (`method_invocation_resolver.dart:738`, `property_element_resolver.dart:795`, `binary_expression_resolver.dart:329`) apenas param de resolver o membro quando o contexto não é válido (não relatam `UNDEFINED_SUPER_*`).
- **Condição exata:** subindo a árvore a partir do `super`: `Annotation` → `annotation`; `ClassDeclaration`/`EnumDeclaration`/`MixinDeclaration` → válido; `CompilationUnit` → estático; construtor `factory` → estático; **qualquer `ConstructorInitializer`** → estático; `ExtensionDeclaration` → extensão; `ExtensionTypeDeclaration` → extension type; `FieldDeclaration` `static` → estático, ou **não `late`** → estático; `MethodDeclaration` `static` → estático. `annotation` e `static` relatam este código.
- **Posição:** o token `super` (a `SuperExpression`).
- **Mensagem:** `Invalid context for 'super' invocation.`
- **Supressões e ordem:** nenhum `UNDEFINED_SUPER_*` no mesmo `super`.
- **No DartForge:** inexistente (o `super` em contexto estático hoje cai em `membro_super` com `this` ausente). **Mudança:** em `expr.rs` (`ExprKind::Super` como alvo de acesso, invocação, índice, operador binário/unário), calcular o contexto pelo `Corpo` (função/membro/inicializador corrente) e relatar no `super`, sem resolver o membro.

##### `super_in_extension` (perda 8: FN 8)
- **Emissão/condição:** a mesma (`element_resolver.dart:414-418`), contexto `ExtensionDeclaration` (antes de achar classe).
- **Posição:** `super`. **Mensagem:** `The 'super' keyword can't be used in an extension because an extension doesn't have a superclass.`
- **No DartForge:** inexistente. **Mudança:** idem.

##### `super_in_extension_type` (perda 3: FN 3)
- **Emissão/condição:** `element_resolver.dart:419-423`, contexto `ExtensionTypeDeclaration`.
- **Posição:** `super`. **Mensagem:** `The 'super' keyword can't be used in an extension type because an extension type doesn't have a superclass.`
- **No DartForge:** inexistente. **Mudança:** idem.

##### `assignment_to_final_no_setter` (perda 16: FN 16)
- **Emissão:** `AssignmentVerifier.verify` (`analyzer/lib/src/error/assignment_verifier.dart:30-110`, relato :82-87) para atribuições resolvidas sem setter; `ErrorVerifier._checkForAssignmentToFinal` (`analyzer/lib/src/generated/error_verifier.dart:2138-2190`, relato :2172-2177) para `++`/`--` e `for (x in …)`.
- **Condição exata:** não há setter (`requested == null`) e o elemento de recuperação é um **getter** cuja variável (`variable2`) é um `FieldElement` **sintético** — i.e., getter explícito sem campo nem setter (inclusive getter **estático** acessado de construtor/`this.x`, getter de extensão: o "campo" sintético da extensão). Getter de campo `final` real → `ASSIGNMENT_TO_FINAL`; `const` → `ASSIGNMENT_TO_CONST`.
- **Posição:** o nome atribuído (identificador simples ou o nome em `x.nome`/`p.nome`).
- **Mensagem:** `There isn't a setter named '{0}' in class '{1}'.` — `{0}` nome da variável; `{1}` `displayName` do dono (classe, mixin, **extensão** — ex. `'E1A'`, `'E6'`).
- **No DartForge:** existe (44 acertos) em `expr.rs::escrita_propriedade`/identificador. Causas: **6 FN `static_extension_getter_setter_conflicts`** e **6 `static_extension_internal_basename_shadowing`** (getter de **extensão** sem setter: dono é a extensão, ou acesso implícito dentro da extensão), **4 FN `getter/no_setter*_test`** (getter **estático** atribuído de dentro de construtor — implícito `nextVar = 1` e `this.nextVar = 1`; hoje sai `UNDEFINED_SETTER`, FP lá).
  **Mudança:** ao não achar setter, procurar o getter também entre os **estáticos** (para `this.x`/implícito) e nas **extensões** aplicáveis, e relatar este código com o nome do dono.

##### `assignment_to_const` (perda 3: FN 3)
- **Emissão:** `assignment_verifier.dart:37-44` (variável `const` como alvo resolvido) e :76-81 (getter de variável const); `error_verifier.dart:2155-2171` (`++`/`--`, `for (x in …)`).
- **Condição exata:** alvo é variável (local, topo, campo estático) `const`.
- **Posição:** o identificador (ou a expressão).
- **Mensagem:** `Constant variables can't be assigned a value.` (no oráculo de 3.13 do grupo analyzer a mensagem é `…after initialization.`; o placar compara com a do 3.6.2).
- **No DartForge:** existe (5). FN: `for (x in …)` com `x` const local (`visitForEachPartsWithIdentifier`, `error_verifier.dart:926-932`, só se `_checkForEachParts` passou) e `case ERROR_B = 1:` (atribuição dentro de expressão de `case` legada). **Mudança:** no cabeçalho de `for-in` com identificador, aplicar a checagem de final/const.

##### `assignment_to_final` (perda 3: FN 3)
- **Emissão:** `assignment_verifier.dart:88-93`, `error_verifier.dart:2178-2183`.
- **Condição exata:** getter cuja variável é final e **não** sintética (campo/variável `final` real), sem setter.
- **Posição:** o nome. **Mensagem:** `'{0}' can't be used as a setter because it's final.`
- **No DartForge:** existe (66). FN `static_extension_internal_basename_shadowing:402-410`: campo `final` estático de **extensão** atribuído por nome implícito dentro de membro da extensão. **Mudança:** procurar os campos estáticos da extensão envolvente no lookup de escrita.

##### `assignment_to_primary_constructor_parameter` (perda 40: FN 40)
- **Não existe no analyzer 3.6.2** (ausente de `analyzer/messages.yaml` e de `codes.g.dart` da 3.6.2). Todos os casos vêm de arquivos com **construtores primários** (sintaxe nova, `primary_constructors/*`, `AssignmentToPrimaryConstructorParameter_*`), classificados em `corpus/diagnosticos/sintaxe-nova.json` e julgados pelo oráculo 3.13.4.
- **Mensagem (3.13):** `A primary constructor parameter can't be assigned to in an initializer.` — o parâmetro de construtor primário (declarado no cabeçalho `class C(int x)`) atribuído (`x = …`, `x++`, `x += …`) dentro de inicializador de campo/inicializador do construtor.
- **Posição (pelo oráculo):** o identificador atribuído (ou a expressão de atribuição; `2:27`, `65:6`/`65:11` = duas atribuições na mesma linha).
- **No DartForge:** inexistente; depende do suporte a construtores primários no parser (`crates/frontend`) e da fonte do analyzer 3.13 para a regra exata (não está em `E:\references\dart-sdk-3.6.2`; usar `E:\references\dart-sdk` só como referência da linguagem nova). **Mudança:** fora do escopo da paridade 3.6.2; tratar junto com a família de construtores primários.

#### Grupo 4 — declarações de extension type, `main`, diretivas de documentação e FFI

Estes códigos são de **declaração** (não de corpo); o lugar natural no DartForge é `crates/analise/src/*.rs` (verificações por declaração), exceto os de FFI que dependem de tipos resolvidos de invocações (`crates/types`). Nenhum deles existe hoje, salvo `EXTENSION_TYPE_DECLARES_MEMBER_OF_OBJECT` (`crates/analise/src/membros.rs:948`).

##### `extension_type_representation_depends_on_itself` (perda 8: FN 8)
- **Emissão:** `ErrorVerifier._checkForExtensionTypeRepresentationDependsOnItself` (`analyzer/lib/src/generated/error_verifier.dart:3491-3500`), de `visitExtensionTypeDeclaration`; o sinal `hasRepresentationSelfReference` é calculado no link (`analyzer/lib/src/summary2/extension_type.dart:120-180`, `_Node`/`_Walker` — grafo de dependência).
- **Condição exata:** o tipo da representação, percorrido por `_DependenciesCollector` (visita argumentos de tipo, tipos de função, records, e o próprio extension type), alcança um extension type da mesma componente fortemente conexa (ciclo, inclusive `extension type A(A it)` e ciclos mútuos A→B→A). Todos os da SCC são marcados, e a representação vira `InvalidType`.
- **Posição:** o nome do extension type (token `name`), em **cada** declaração do ciclo (oráculo: `1:16` e `5:16`).
- **Mensagem:** `The extension type representation can't depend on itself.`
- **Supressões:** com a representação inválida, os demais checks de representação não relatam.
- **No DartForge:** inexistente. **Mudança:** em `crates/analise` (ou no outline de `types`, que já conhece a representação), montar o grafo extension type → extension types citados no tipo da representação, achar SCCs (Tarjan) e relatar no nome de cada membro da SCC (inclusive auto-laço).

##### `extension_type_declares_member_of_object` (perda 7: FN 7)
- **Emissão:** `error_verifier.dart:3384-3390` (método/getter/setter com nome de membro de `Object`: `==`, `hashCode`, `toString`, `noSuchMethod`, `runtimeType`), no token do nome.
- **Mensagem:** `Extension types can't declare members with the same name as a member declared by 'Object'.`
- **No DartForge:** `crates/analise/src/membros.rs:948` (48 acertos). As **7 FN** são de `primary_constructors/header/extension_type_error_test.dart` — **construtor primário com parâmetros nomeados** (`extension type ET12({required final int hashCode})`), sintaxe nova julgada pelo 3.13: o campo declarado no cabeçalho é membro. **Mudança:** quando o parser aceitar o cabeçalho com parâmetros nomeados (3.13), aplicar a regra aos campos do cabeçalho.

##### `extension_type_implements_disallowed_type` (perda 7: FN 7)
- **Emissão:** `ResolutionVisitor._verifyExtensionElementImplements` (`analyzer/lib/src/dart/resolver/resolution_visitor.dart:1786-1800`), para cada tipo da cláusula `implements`.
- **Condição exata:** `!isValidExtensionTypeSuperinterface(type)` (`analyzer/lib/src/dart/element/type_system.dart:1361-1378`): não é `InterfaceType` (parâmetro de tipo `X`, `dynamic`, tipo de função, record…), ou é anulável (`T?`), ou é `FutureOr`, `Function`, `Null`, `Record`.
- **Posição:** o `NamedType` na cláusula. **Mensagem:** `Extension types can't implement '{0}'.` — `{0}` DartType (`'X'`, `'dynamic'`, `'Function'`, `'int?'`).
- **Supressões e ordem:** sai e **para** (não checa supertipo da representação).
- **No DartForge:** inexistente. **Mudança:** em `crates/analise/src/heranca.rs` (cláusula implements de extension type).

##### `extension_type_implements_not_supertype` (perda 6: FN 6)
- **Emissão:** `resolution_visitor.dart:1802-1838`.
- **Condição exata:** tipo válido (acima), a representação **não** é subtipo dele, e ele não é extension type (para extension type, ver `…_REPRESENTATION_NOT_SUPERTYPE`).
- **Posição:** o `NamedType`. **Mensagem:** `'{0}' is not a supertype of '{1}', the representation type.` — `{0}` o tipo implementado, `{1}` a representação declarada (DartTypes).
- **No DartForge:** inexistente; mesmo lugar.

##### `extension_type_inherited_member_conflict` (perda 4: FN 4)
- **Emissão:** `ErrorVerifier._checkForExtensionTypeMemberConflicts` (`error_verifier.dart:3448-3485`) com os `conflicts` da interface (`InheritanceManager3.getInterface`).
- **Condição exata:** algum conflito na interface combinada do extension type: `CandidatesConflict` (dois membros distintos de mesmo nome vindos de supertipos sem sobrescrita local), `HasNonExtensionAndExtensionMemberConflict` (membro de classe e de extension type com mesmo nome), `NotUniqueExtensionMemberConflict` (dois membros de extension types distintos).
- **Posição:** o nome do extension type. **Mensagem:** `The extension type '{0}' has more than one distinct member named '{1}' from implemented types.` (`{0}` nome, `{1}` membro). Um relato por conflito.
- **No DartForge:** inexistente; requer a interface combinada de extension types (`crates/analise/src/heranca.rs`).

##### `extension_type_representation_type_bottom` (perda 4: FN 4)
- **Emissão:** `error_verifier.dart:3503-3513`.
- **Condição exata:** o tipo da representação é bottom (`isBottom`, `analyzer/lib/src/dart/element/type.dart`: `Never` não anulável, parâmetro de tipo cujo limite/promoção é bottom (:1538-...); `Never?` não é bottom).
- **Posição:** o **tipo** do campo de representação (`fieldType`). **Mensagem:** `The representation type can't be a bottom type.`
- **No DartForge:** inexistente; em `crates/analise` com o tipo da representação do outline.

##### `extension_type_constructor_with_super_invocation` (perda 3: FN 3)
- **Emissão:** `error_verifier.dart:3393-3402`, de `visitSuperConstructorInvocation` (:1443-1453).
- **Condição:** inicializador `super(...)`/`super.n(...)` em construtor de extension type. **Posição:** o token `super`. **Mensagem:** `Extension type constructors can't include super initializers.`
- **No DartForge:** inexistente; `crates/analise/src/construtores.rs`.

##### `extension_type_implements_itself` (perda 3: FN 3)
- **Emissão:** `error_verifier.dart:3438-3447`; sinal calculado em `summary2/extension_type.dart:63-110` (`_ImplementsNode`/`_ImplementsWalker`: SCC no grafo extension type → extension types da cláusula `implements`).
- **Posição:** o nome de cada extension type do ciclo. **Mensagem:** `The extension type can't implement itself.`
- **No DartForge:** inexistente; `crates/analise/src/heranca.rs` (há detecção de ciclos de classe que pode ser reaproveitada).

##### `extension_type_constructor_with_super_formal_parameter` (perda 2: FN 2)
- **Emissão:** `ErrorVerifier.visitSuperFormalParameter` (`error_verifier.dart:1456-1466`).
- **Condição:** parâmetro `super.x` em construtor de extension type (antes de qualquer outra checagem de `super.x`). **Posição:** o token `super`. **Mensagem:** `Extension type constructors can't declare super formal parameters.`

##### `extension_type_implements_representation_not_supertype` (perda 2: FN 2)
- **Emissão:** `resolution_visitor.dart:1808-1829`.
- **Condição:** o tipo implementado é extension type cuja representação não é supertipo da representação declarada (e a representação declarada não é subtipo do próprio tipo implementado).
- **Posição:** o `NamedType`. **Mensagem:** `'{0}', the representation type of '{1}', is not a supertype of '{2}', the representation type of '{3}'.` — `{0}` representação implementada (DartType), `{1}` nome do implementado, `{2}` representação declarada, `{3}` nome do declarante.

##### `main_first_positional_parameter_type` (perda 3: FN 3)
- **Emissão:** `ErrorVerifier._checkForMainFunction2` (`analyzer/lib/src/generated/error_verifier.dart:4143-4188`), de `visitFunctionDeclaration` de topo.
- **Condição exata:** função de topo `main` com algum parâmetro posicional; `List<String>` não é subtipo do tipo do primeiro posicional.
- **Posição:** o **tipo** escrito do primeiro posicional (`first.notDefault.typeOrSelf`: o `NamedType`, ou o próprio parâmetro se sem tipo). **Mensagem:** `The type of the first positional parameter of the 'main' function must be a supertype of 'List<String>'.`
- **No DartForge:** inexistente; `crates/analise` (declarações de topo).

##### `main_has_too_many_required_positional_parameters` (perda 3: FN 3)
- **Emissão:** a mesma função (`error_verifier.dart:4160-4166`). **Condição:** mais de 2 posicionais obrigatórios. **Posição:** o nome `main`. **Mensagem:** `The function 'main' can't have more than two required positional parameters.`

##### `main_has_required_named_parameters` (perda 2: FN 2)
- **Emissão:** `error_verifier.dart:4168-4173`. **Condição:** algum nomeado `required`. **Posição:** o nome `main`. **Mensagem:** `The function 'main' can't have any required named parameters.` (sai junto com o anterior se ambos.)

##### `doc_directive_missing_argument` (perda 10: FN 10)
- **Emissão:** `DocCommentVerifier.validateArgumentCount` (`analyzer/lib/src/error/doc_comment_verifier.dart:52-92`), sobre as diretivas `{@tipo …}` montadas por `DocCommentBuilder` (`analyzer/lib/src/fasta/doc_comment_builder.dart`); chamado de `ErrorVerifier.visitComment`/`DocCommentVerifier.docDirective`.
- **Condição exata:** menos argumentos posicionais que os exigidos pelo tipo (`DocDirectiveType`: `animation` (width, height, url), `youtube` (width, height, url), `macro`/`template` (name), `canonicalFor` (element), `category`/`subCategory` (resto), `inject-html`, `tool`, etc.). Variantes por diferença: 1 → `DOC_DIRECTIVE_MISSING_ONE_ARGUMENT [tipo, último exigido]`; 2 → `…_TWO_ARGUMENTS [tipo, penúltimo, último]`; 3 → `…_THREE_ARGUMENTS`.
- **Posição:** a tag inteira (`tag.offset` a `tag.end`: do `{@` ao `}`).
- **Mensagem:** `The '{0}' directive is missing a '{1}' argument.` / `… a '{1}' and a '{2}' argument.` / `… a '{1}', a '{2}', and a '{3}' argument.`
- **No DartForge:** inexistente; exige um analisador de comentários de documentação (`///` e `/** */`) que reconheça as diretivas. Lugar: um módulo novo em `crates/analise`.

##### `doc_directive_argument_wrong_format` (perda 5: FN 5)
- **Emissão:** `doc_comment_verifier.dart:121-150`. **Condição:** argumento posicional com formato esperado `integer` que não é inteiro (`width`, `height` de `animation`/`youtube`), ou `uri` inválida. **Posição:** o argumento. **Mensagem:** `The '{0}' argument must be formatted as {1}.` — `{1}` = `displayString` do formato (`an integer`, `a URI`, …).

##### `doc_directive_has_extra_arguments` (perda 4: FN 4)
- **Emissão:** `doc_comment_verifier.dart:97-108` (e `doc_comment_builder.dart:1241` para blocos). **Condição:** mais posicionais que os exigidos e o tipo não aceita resto. **Posição:** do primeiro excedente ao fim do último argumento. **Mensagem:** `The '{0}' directive has '{1}' arguments, but only '{2}' are expected.` (`{1}`,`{2}` números).

##### `doc_directive_has_unexpected_named_argument` (perda 2: FN 2)
- **Emissão:** `doc_comment_verifier.dart:110-118`. **Condição:** argumento nomeado (`nome=valor`) que o tipo não declara. **Posição:** o argumento nomeado inteiro. **Mensagem:** `The '{0}' directive has an unexpected named argument, '{1}'.`

##### `argument_must_be_a_constant` (perda 9: FN 9)
- **Emissão:** `FfiVerifier` (`analyzer/lib/src/generated/ffi_verifier.dart`): `_validateIsLeafIsConst` (:1606-1624: argumento nomeado `isLeaf` de `asFunction`/`lookupFunction` não constante), `_validateFromFunction` (:1595-1602: `exceptionalReturn` de `Pointer.fromFunction`), e `NativeCallable.isolateLocal` (:1822-1828).
- **Posição:** a expressão do argumento (sem o rótulo). **Mensagem:** `Argument '{0}' must be a constant.` (`{0}` `isLeaf`/`exceptionalReturn`).
- **No DartForge:** sem verificador de FFI; seria em `crates/types` (invocações resolvidas de membros de `dart:ffi`).

##### `argument_must_be_native` (perda 3: FN 3)
- **Emissão:** `ffi_verifier.dart:1715-1755` (`Native.addressOf(arg)`): o argumento não é um identificador/tear-off constante de função ou variável anotada `@Native`. **Posição:** o argumento. **Mensagem:** `Argument to 'Native.addressOf' must be annotated with @Native`.

##### `leaf_call_must_not_return_handle` (perda 7: FN 7)
- **Emissão:** `FfiVerifier._validateFfiLeafCallUsesNoHandles` (`ffi_verifier.dart:1413-1430`), chamado para `asFunction(isLeaf: true)`, `lookupFunction(isLeaf: true)`, e `@Native(isLeaf: true)`/`@FfiNative(isLeaf: true)` em funções/getters.
- **Condição:** tipo nativo é função com retorno `Handle`. **Posição:** depende do chamador: `asFunction(isLeaf: true)` → o token do nome `asFunction` (`ffi_verifier.dart:1239-1240`); `lookupFunction<S, F>(…, isLeaf: true)` → o primeiro argumento de tipo `S` (:1661-1662); anotação `@Native`/`@FfiNative` → o `errorToken` da declaração anotada (nome da função/getter, :606-609). **Mensagem:** `FFI leaf call can't return a 'Handle'.`

##### `leaf_call_must_not_take_handle` (perda 5: FN 5)
- Mesma função (:1424-1429): algum parâmetro normal `Handle` (um relato por parâmetro). **Mensagem:** `FFI leaf call can't take arguments of type 'Handle'.`

##### `non_sized_type_argument` (perda 2: FN 2)
- **Emissão:** `ffi_verifier.dart:1474-1488` (campo `Array<T>` de `Struct`/`Union` com `T` não dimensionado, ex. `Void`). **Posição:** o primeiro argumento de tipo do campo (ou o tipo). **Mensagem:** `The type '{1}' isn't a valid type argument for '{0}'. The type argument must be a native integer, 'Float', 'Double', 'Pointer', or subtype of 'Struct', 'Union', or 'AbiSpecificInteger'.` (`{0}` `'Array'`, `{1}` o tipo).

##### `non_native_function_type_argument_to_pointer` (perda 1: FN 1)
- **Emissão:** `ffi_verifier.dart:1210-1225` (`Pointer<T>.asFunction` com `T` que não é `NativeFunction` de assinatura C válida). **Posição:** `errorNode` (o nome `asFunction`). **Mensagem:** `Can't invoke 'asFunction' because the function signature '{0}' for the pointer isn't a valid C function signature.` (`{0}` o tipo `T`).

##### `use_of_native_extension` (perda 3: FN 3)
- **Emissão:** `LibraryAnalyzer._reportImportDirectiveErrors` (`analyzer/lib/src/dart/analysis/library_analyzer.dart:672-684`) e export (:906-916); também para `@docImport`.
- **Condição:** URI selecionada começa com `dart-ext:`. **Posição:** a URI (string). **Mensagem:** `Dart native extensions are deprecated and aren't available in Dart 2.15.`
- **No DartForge:** inexistente; `crates/analise/src/importacoes.rs` (antes de `uri_does_not_exist`).

#### Grupo 5 — argumentos de tipo (limites, inferência, contagem)

##### `type_argument_not_matching_bounds` (perda 99: FN 97, FP 2)
- **Emissão:** vários pontos, todos com a mesma mensagem:
  - `TypeArgumentsVerifier.checkNamedType` → `_checkNamedTypeArguments` (`analyzer/lib/src/error/type_arguments_verifier.dart:290-400`): tipos nomeados escritos (`C<String>` em anotações de tipo, `extends`/`implements`, literais de tipo, `new`/`const`), com a regra de **bem-limitado**: primeiro regular-bounded (`T_i <: B_i[T/X]`); se falha, e o contexto permite super-bounded (não em `new`/`const`/supertipo — `allowSuperBounded`), testa super-bounded (troca topo↔`Never` em posições covariantes/contravariantes, `isWellBounded`) e só relata se também falha (:340-400);
  - `checkEnumConstantDeclaration` (:91-140): argumentos (explícitos **ou inferidos**, `constructorElement.returnType.typeArguments`) de constante de enum; posição o argumento explícito ou o **nome** da constante;
  - `_checkInvocationTypeArguments` (:404-492): `f<T>()`/`o.m<T>()`/`f<T>` tear-off (`checkFunctionReference`, `checkMethodInvocation`, `checkFunctionExpressionInvocation`): argumento explícito contra o limite;
  - `InvocationInferrer.resolveInvocation` (`analyzer/lib/src/dart/resolver/invocation_inferrer.dart:186-209`): para anotações e construtores **com argumentos explícitos** (`_needsTypeArgumentBoundsCheck`);
  - `ExtensionMemberResolver._checkTypeArgumentsMatchingBounds` (`analyzer/lib/src/dart/resolver/extension_member_resolver.dart:250-270`): `E<String>(x)` — só com argumentos **explícitos** (`typeArgumentList != null`), no argumento `i`; os inferidos do override (`E(x)`) passam pelo `GenericInferrer` com `errorEntity: node.name` (:354) e saem como `COULD_NOT_INFER` no **nome da extensão**;
  - inferidos em geral **não** relatam este código (o `GenericInferrer` usa `COULD_NOT_INFER`).
- **Condição exata:** `!isSubtypeOf(T_i, B_i[T/X])` (limite substituído pelos próprios argumentos), com o desconto de super-bounded onde permitido; argumentos em número errado não checados (outro código).
- **Posição:** o argumento de tipo `i` (o nó do argumento); se não há nó (inferido), o nó do tipo/constante.
- **Mensagem:** `'{0}' doesn't conform to the bound '{2}' of the type parameter '{1}'.` — `{0}` argumento (DartType), `{1}` nome do parâmetro, `{2}` limite substituído (DartType).
- **No DartForge:** `crates/types` (outline/tipos; 1078 acertos). Causas das FN: **34 `generic/super_bounded_types_error*_test`** (o teste de super-bounded: tipos que não são regular- nem super-bounded em contextos onde super-bounded não é aceito — `new`, `extends`, argumentos de construtor), **14 `variance/*`** (experimento), **8 `inference/issue_61370*`** e **`closure/partial_instantiation_static_bounds_check`** (instanciação implícita de tear-off genérico), **3 `extension_methods/static_extension_bounds_error_test`** + `TypeArgumentNotMatchingBounds__extensio_*` (override de extensão), **enum** (`enum_inferred`, `enum_wit…`: argumentos inferidos de constante de enum), **`methodIn_*`** (`o.m<String>()`), **`function*`/`functionReference`** (`f<String>`), **`typeLiteral_*`/`typeLite_*`** (`C<String>` como expressão), **`metadata_*`** (`@A<String>()`), `extends__…` (`T extends U` com `U` não conforme), `null_aware_elements/flow_analysis_test` (3), `type_variable/bounds2-4`, `regress34532/18628`. FP: `type_alias_cannot_reference_itself/*` (alias cíclico: o analyzer não checa limites de alias inválido).
  **Mudança:** completar cada ponto de emissão (invocações com argumentos explícitos em `chamadas.rs::invocar`/`argumentos_de_tipo`, overrides de extensão em `chamada`, constantes de enum em `funcoes.rs`, literais de tipo em `expr.rs`) e o teste de super-bounded no verificador de tipos nomeados.

##### `could_not_infer` (perda 45: FN 26, msg 16, pos 3)
- **Emissão:** `GenericInferrer.chooseFinalTypes`/`_chooseTypes` (`analyzer/lib/src/dart/element/generic_inferrer.dart:250-370`) e `_checkArgumentsNotMatchingBounds` (:390-420), com o `errorEntity` dado pelo chamador (`InvocationInferrer._errorEntity`: o nó da invocação; para `MethodInvocation` o `methodName`; para criação de instância o `constructorName`; tear-off/`FunctionReference` o nó).
- **Condição exata (variantes do `{1}`):**
  1. o tipo inferido não satisfaz as restrições (inclui o limite `extends`): `{1}` = `_formatError(...)`: `"\n\nTried to infer '<T>' for '<X>' which doesn't work:\n  <restrições, uma por linha>\nConsider passing explicit type argument(s) to the generic.\n\n"` (texto exato em `generic_inferrer.dart:~430-520`, `_formatError`/`_messageForConstraint`);
  2. candidato é tipo de função genérico sem `generic-metadata`: `" Inferred candidate type ... has type parameters [...], but a function with type parameters cannot be used as a type argument."`;
  3. `instantiateToBounds` com limite recursivo (`hasError`): `"\nRecursive bound cannot be instantiated: '<bound>'.\nConsider passing explicit type argument(s) to the generic.\n\n'"`;
  4. sem erro anterior, argumentos finais que não respeitam o limite substituído: `"\n'<arg>' doesn't conform to the bound '<bound>', instantiated from '<rawBound>' using type arguments [<args>]."` (lista com `toString` de `List`: `[A, B]`).
- **Posição:** o `errorEntity`.
- **Mensagem:** `Couldn't infer type parameter '{0}'.{1}` — `{1}` como acima (tipos com `_typeStr` = `getDisplayString()`).
- **No DartForge:** `chamadas.rs::invocar` → `GenericInferrer::falhas` (`crates/types/src/constraints.rs`). Causas: **16 msg** — o sufixo `{1}` (a variante 4 com "instantiated from … using type arguments […]" e a variante 1 com a lista de restrições) não está igual; **FN**: `downwardInference_fixes` (contexto de retorno fixa `T` incompatível), `functionType_parameterIs*` (parâmetro de tipo função com variável em posição contravariante), `instanceCreation_viaType…` (via alias), `topLevel`, `variance/*` (experimento), `generic/function_type_as_type_argument` (variante 2); **pos**: `static_extension_bounds_error_test:37/60/127` (`E1(s).e1` com inferência do override falhando: a entidade é o **nome da extensão** `E1` — `extension_member_resolver.dart:354`, `errorEntity: node.name`).
  **Mudança:** reproduzir `_formatError` e as variantes 2–4 literalmente; ligar os pontos que faltam.

##### `not_instantiated_bound` (perda 35: FN 35)
- **Emissão:** `ErrorVerifier.visitTypeParameter` → `node.bound?.accept(_uninstantiatedBoundChecker)` (`analyzer/lib/src/generated/error_verifier.dart:1590`, classe `_UninstantiatedBoundChecker` :7167-7190).
- **Condição exata:** no **limite** de um parâmetro de tipo (de classe, mixin, função, typedef, método…), cada `NamedType` **sem argumentos de tipo** cujo elemento é genérico e **não é simplesmente limitado** (`isSimplyBounded == false`, calculado em `analyzer/lib/src/summary2/simply_bounded.dart`: um parâmetro cujo limite menciona, direta ou indiretamente, tipos genéricos não instanciados que dependem dele — ciclos `class A<T extends A>`). Os `NamedType` com argumentos são visitados recursivamente (só os argumentos).
- **Posição:** o `NamedType` cru. **Mensagem:** `Type parameter bound types must be instantiated.`
- **Supressões:** o TODO diz que relata mesmo com `TYPE_ALIAS_CANNOT_REFERENCE_ITSELF`.
- **No DartForge:** inexistente. **Mudança:** calcular "simplesmente limitado" para classes/aliases (algoritmo da especificação, seção "Simply bounded") no outline e, nos limites de parâmetros de tipo, relatar cada tipo cru não simplesmente limitado (`crates/analise`/`crates/types`).

##### `wrong_number_of_type_arguments` (perda 32: FN 31, FP 1)
- **Emissão:**
  - `NamedTypeResolver._buildTypeArguments` (`analyzer/lib/src/dart/resolver/named_type_resolver.dart:137-148`): qualquer tipo nomeado escrito com contagem errada (inclusive `dynamic<int>`, `Never<int>`, `new C<int>()`, `const C<int>()`); `{0}` = `node.name2.lexeme` (o nome escrito, sem prefixo), posição **o `NamedType` inteiro** (`C<int>`, `dynamic<int>`);
  - `InvocationInferrer` (`invocation_inferrer.dart:316-327`, anotação: `_wrongNumberOfTypeArgumentsErrorCode = WRONG_NUMBER_OF_TYPE_ARGUMENTS` em `AnnotationInferrer` :84-87 e `AugmentedInvocationInferrer` :127-130): `@A<int>()` com contagem errada — `{0}` = o **tipo de função cru do construtor** (`'A Function()'`, `'A<T, U> Function()'`), posição a **lista de argumentos de tipo** `<int>`;
  - `function_reference_resolver.dart:316-325`/`:825-833` (tear-offs de tipo `C<int>.new`? / literal de tipo).
- **Mensagem:** `The type '{0}' is declared with {1} type parameters, but {2} type arguments were given.`
- **No DartForge:** existe para tipos nomeados em anotações de tipo (36). FN: **`new`/`const` com contagem errada** (`new_*`, `const_*`, `factory*_test`, `unsorted/invalid_type_argument_count_test`), **anotações** (`metadata_*`, `messageText_ca92c414`/`afc9f656`), `dynamic<int>`/`Never<int>` (`messageText_dynamic`/`never`), `mixin/type_parameters_errors_test` (em `with`/`on`), `type_object/explicit_instantiated_type_literal…`. FP: `augmentation_type_parameter_count/*` (augmentation — outro código).
  **Mudança:** em `chamadas.rs::instanciacao` (tipo da criação) e no caminho de anotações (`funcoes.rs::anotacao_sem_validar`, com o tipo de função do construtor), e no resolvedor de tipos para `dynamic`/`Never`.

##### `wrong_number_of_type_arguments_function` (perda 13: FN 13)
- **Emissão:** variantes `WRONG_NUMBER_OF_TYPE_ARGUMENTS_FUNCTION` (`'The function '{0}' is declared with {1} type parameters, but {2} type arguments were given.'`) e `…_ANONYMOUS_FUNCTION` (`'This function is declared with {0} type parameters, but {1} type arguments were given.'`), em `FunctionReferenceResolver` (`analyzer/lib/src/dart/resolver/function_reference_resolver.dart:110-130`, :255-300): tear-off `f<int, String>` (função nomeada; `{0}` = nome, inclusive `'call'` para `o.call<…>`) ou expressão de função (`(f)<int>`, `fn<int>` de variável de tipo função → anônima). Também nas constantes (`constant/evaluation.dart:2315-2332`).
- **Posição:** a lista de argumentos de tipo.
- **No DartForge:** inexistente (tear-offs com argumentos de tipo em `expr.rs`, `ExprKind::TypeArguments`). **Mudança:** checar a contagem nas instanciações explícitas de tear-off.

##### `wrong_number_of_type_arguments_constructor` (perda 12: FN 12)
- **Emissão:** `ast_rewrite.dart:430-445` e :600-612 (`C.named<int>()` reescrito para criação: argumentos de tipo **depois** do nome do construtor), `named_type_resolver.dart:333-345` (`p.C<int>` onde `C` é construtor? — `prefix.Class<…>` reinterpretado), `function_reference_resolver.dart:52-62` (`C.named<int>` tear-off).
- **Condição:** argumentos de tipo aplicados ao **construtor** (`C.named<int>`), o que nunca é permitido.
- **Posição:** a lista de argumentos de tipo. **Mensagem:** `The constructor '{0}.{1}' doesn't have type parameters.` (`{0}` classe, `{1}` construtor). Variante de dot shorthand (3.10, não 3.6.2) no oráculo novo.
- **No DartForge:** inexistente; `chamadas.rs::alvo_construtor` (alvo `TypeArguments` sobre `C.named`).

##### `wrong_number_of_type_arguments_method` (perda 6: FN 6)
- **Emissão:** `FullInvocationInferrer._wrongNumberOfTypeArgumentsErrorCode` (`invocation_inferrer.dart:151-155`) via `_reportWrongNumberOfTypeArguments` (:316-327): `f<int>()`/`o.m<int, String>()` com contagem diferente da do tipo invocado.
- **Posição:** a lista de argumentos de tipo. **Mensagem:** `The method '{0}' is declared with {1} type parameters, but {2} type arguments are given.` — `{0}` é o **tipo de função cru** (`'void Function<T, U>()'`, DartType), não o nome.
- **Supressões:** os argumentos viram `dynamic` e a inferência segue sem os explícitos.
- **No DartForge:** `chamadas.rs::invocar` ignora explícitos com contagem errada (`if ex.len() == type_params.len()`). **Mudança:** relatar com o tipo formatado.

##### `wrong_number_of_type_arguments_extension` (perda 4: FN 4)
- **Emissão:** `extension_member_resolver.dart:325-345` (`E<int>(x)` com contagem errada). **Posição:** a lista de argumentos de tipo. **Mensagem:** `The extension '{0}' is declared with {1} type parameters, but {2} type arguments were given.`
- **No DartForge:** `chamadas.rs::chamada` (ramo `RefTipo::Extensao`) usa `ext_args` só se a contagem bate; relatar quando não bate.

##### `wrong_number_of_type_arguments_enum` (perda 3: FN 3)
- **Emissão:** `TypeArgumentsVerifier.checkEnumConstantDeclaration` (`type_arguments_verifier.dart:91-112`). **Posição:** a lista `<…>` da constante. **Mensagem:** `The enum is declared with {0} type parameters, but {1} type arguments were given.`
- **No DartForge:** inexistente; em `funcoes.rs` (constantes de enum, onde `k.type_args` é lido).

##### `expected_one_list_type_arguments` (perda 1: FN 1)
- **Emissão:** `TypeArgumentsVerifier.checkListLiteral` (`type_arguments_verifier.dart:160-172`). **Condição:** literal de lista com ≠ 1 argumento de tipo. **Posição:** a lista de argumentos de tipo. **Mensagem:** `List literals require one type argument or none, but {0} found.`

##### `expected_one_set_type_arguments` (perda 1: FN 1)
- **Emissão:** `checkSetLiteral` (:208-221). **Condição:** literal resolvido como conjunto com ≠ 1 argumento (com mais de 2 argumentos o literal `{}` é decidido conjunto/mapa pelos elementos). **Mensagem:** `Set literals require one type argument or none, but {0} were found.`

##### `expected_two_map_type_arguments` (perda 2: FN 2)
- **Emissão:** `checkMapLiteral` (:175-188). **Condição:** literal de mapa com ≠ 2 argumentos. **Mensagem:** `Map literals require two type arguments or none, but {0} found.`
- **No DartForge (os três):** inexistentes; em `colecoes.rs` ao ler os argumentos explícitos do literal.

##### `invalid_extension_argument_count` (perda 2: FN 2)
- **Emissão:** `ExtensionMemberResolver.resolveOverride` (`extension_member_resolver.dart:189-197`). **Condição:** `E(...)` com número de argumentos ≠ 1 (inclusive nomeados). **Posição:** a lista de argumentos `(...)`. **Mensagem:** `Extension overrides must have exactly one argument: the value of 'this' in the extension method.`
- **No DartForge:** `chamadas.rs::chamada` só trata `args.len() == 1`; relatar nos outros casos.

##### `enum_instantiated_to_bounds_is_not_well_bounded` (perda 1: FN 1)
- **Emissão:** `error_verifier.dart:3239-3255`. **Condição:** o tipo do campo `values` (`List<E<instanciado aos limites>>`) tem argumento não bem-limitado (`isWellBounded(…, allowSuperBounded: true)`). **Posição:** o nome do enum. **Mensagem:** `The result of instantiating the enum to bounds is not well-bounded.`

##### `generic_function_type_cannot_be_type_argument` (perda 2: FN 2)
- **Emissão:** `type_arguments_verifier.dart:300-310` (tipo nomeado) e :465-475 (invocação), **só sem** o recurso `generic-metadata` (linguagem < 2.14). **Posição:** o argumento. **Mensagem:** `A generic function type can't be a type argument.` — os 2 casos do corpus são arquivos de versão de linguagem antiga (`// @dart=2.x`).

##### `disallowed_type_instantiation_expression` (perda 2: FN 2)
- **Emissão:** `function_reference_resolver.dart:270-280`, :336-345, :490-497, :562-570: argumentos de tipo aplicados a uma expressão que não é tipo genérico, função genérica, método genérico nem construtor genérico (ex.: `(f)<int>` com `f` não genérica de tipo não função, getter, etc.). **Posição:** a expressão alvo (`function`). **Mensagem:** `Only a generic type, generic function, generic instance method, or generic constructor can have type arguments.`

##### `instantiate_type_alias_expands_to_type_parameter` (perda 7: FN 7)
- **Emissão:** `NamedTypeResolver._verifyTypeAliasForContext` (`analyzer/lib/src/dart/resolver/named_type_resolver.dart:420-450`). **Condição:** alias cujo `aliasedType` é parâmetro de tipo (`typedef T<X> = X;`) usado em `new`/`const T<…>()` (criação) ou como alvo de redirecionamento de factory. **Posição:** `_ErrorHelper._getErrorRange(node)` — o nome do tipo **com prefixo e argumentos de tipo** (do início do nome ao fim dos argumentos). **Mensagem:** `Type aliases that expand to a type parameter can't be instantiated.`
- **No DartForge:** `chamadas.rs::instanciacao`, ramo `Element::Typedef` (comentário diz "ficam mudos" para `typedef A<X> = X`). **Mudança:** relatar quando o alvo do alias é parâmetro de tipo (também na criação implícita `T<int>()` reescrita).

#### Grupo 6 — aridade de argumentos, `super.x` e construtores indefinidos

Base: `ResolverVisitor.resolveArgumentsToParameters` (`analyzer/lib/src/generated/resolver.dart:4229-4355`) é chamado, com `errorReporter`, por: `InvocationInferrer.resolveInvocation` (`analyzer/lib/src/dart/resolver/invocation_inferrer.dart:281-288`) — `MethodInvocation`, `FunctionExpressionInvocation`, `InstanceCreationExpression` (inclusive a implícita `C()` reescrita pelo `AstRewriter`), anotações; `ElementResolver._resolveArgumentsToFunction` (`analyzer/lib/src/generated/element_resolver.dart:440-458`) — `SuperConstructorInvocation` (com `enclosingConstructor`, :392-399; pulado se a superclasse do `extends` é tipo indefinido, :385-391) e `RedirectingConstructorInvocation`; e as constantes de enum com argumentos (`resolver.dart:2523-2531`). Só há relato quando o alvo resolveu a um tipo de função/elemento executável (`dynamic`, `Function`, membro indefinido: nada).

Algoritmo (`resolver.dart:4234-4352`):
1. parâmetros → `unnamedParameters` (obrigatórios + opcionais posicionais), `requiredParameterCount`, `namedParameters` (mapa por nome);
2. argumentos posicionais na ordem: conta `positionalArgumentCount`; o primeiro além dos posicionais declarados é `firstUnresolvedArgument`; guarda `lastPositionalArgument`; se algum é `SimpleIdentifier` vazio (recuperação do parser), `noBlankArguments = false`;
3. com `enclosingConstructor` (só `super(...)`): `verifySuperFormalParameters` (`analyzer/lib/src/error/super_formal_parameters_verifier.dart:11-37`) soma os `super.x` posicionais ao `positionalArgumentCount` e põe os nomes dos `super.x` nomeados em `usedNames` (e relata `POSITIONAL_SUPER_FORMAL_PARAMETER_WITH_POSITIONAL_ARGUMENT` se há posicionais explícitos);
4. argumentos nomeados, na ordem: nome desconhecido → `UNDEFINED_NAMED_PARAMETER` no rótulo; nome repetido em `usedNames` (inclusive vindo de `super.x`) → `DUPLICATE_NAMED_ARGUMENT` no rótulo;
5. se `positionalArgumentCount < requiredParameterCount && noBlankArguments` → `NOT_ENOUGH_POSITIONAL_ARGUMENTS_*`; senão, se `positionalArgumentCount > unnamedParameterCount && noBlankArguments` → `EXTRA_POSITIONAL_ARGUMENTS(_COULD_BE_NAMED)` **só se houver `firstUnresolvedArgument`** (excesso vindo só de `super.x` não relata).
O `MISSING_REQUIRED_ARGUMENT` é de outra fase (`RequiredParametersVerifier`, no `ErrorVerifier`).

Forma da invocação no analyzer (decide nome citado e entidade):
- `C(...)`, `p.C(...)`, `C<T>(...)`, `C.n(...)`, `new/const C…(...)` → `InstanceCreationExpression` (o `AstRewriter` reescreve as implícitas);
- `f(...)` com `f` função de topo, função local, método (implícito ou `x.m`, `p.f`, `C.m`, `super.m`) → `MethodInvocation`;
- `v(...)` com `v` variável/parâmetro/getter (inclusive de `this` implícito), `x.g(...)` com `g` getter/campo, `(e)(...)`, `e()(...)` → `FunctionExpressionInvocation` (`method_invocation_resolver.dart:575-600`, :856-858: `_rewriteAsFunctionExpressionInvocation`).

##### `extra_positional_arguments` (perda 64: FN 20, FP 23, pos 21)
- **Emissão:** `resolver.dart:4333-4352`.
- **Condição exata:** passo 5, e `namedParameters.length <= usedNames.length` (não sobra nomeado não usado; `usedNames` conta nomes distintos dos argumentos nomeados — inclusive os indefinidos — e os `super.x` nomeados).
- **Posição:** o **primeiro argumento posicional excedente** (`firstUnresolvedArgument`, a expressão inteira).
- **Mensagem:** `Too many positional arguments: {0} expected, but {1} found.` — `{0}` = número de parâmetros posicionais (obrigatórios + opcionais), `{1}` = posicionais passados (incluindo `super.x`).
- **Supressões e ordem:** com argumento "em branco" de recuperação, nada; tipo do alvo `dynamic`/`Function`/não resolvido: nada.
- **No DartForge:** `chamadas.rs::verificar_aridade` relatava texto genérico (`aviso` com molde e "esperava no máximo…") **na lista de argumentos inteira** — daí as 21 posições erradas (o oráculo aponta o excedente) e as 23 FP (os casos que são `_COULD_BE_NAMED` saíam como este código). As 20 FN: `constructor/redirect2_test` (`this(...)`), `regress35258`, `deferred/load_library_wrong_args` (`p.loadLibrary(1)`: tipo `Future<void> Function()`), `enumConstant` (`v(1)` em enum sem construtor que aceite), `field_initializer_outside_constructor…`, `parameter/named_aggregated_test`, `dot_shorthands/*` (3.10, sintaxe nova).
  **Mudança (já escrita no working tree, a validar):** `verificar_aridade` reescrita com o algoritmo acima, emitindo `aviso_com_codigo` com `[sem_nome, total_pos]` no primeiro excedente; `chamar_construtor_de` (`funcoes.rs`) passa a usar a assinatura **inteira** do super construtor com os `super.x` contados no alvo (`AlvoDaAridade.super_posicionais`/`super_nomeados`) em vez de removê-los (`sem_parametros_super` removida); faltam `loadLibrary` e enum sem construtor compatível.

##### `extra_positional_arguments_could_be_named` (perda 21: FN 21)
- **Emissão/condição:** `resolver.dart:4335-4340`: passo 5 com `namedParameters.length > usedNames.length` (há nomeado declarado ainda não passado).
- **Posição/mensagem:** idem ao anterior (mesmo texto `Too many positional arguments: {0} expected, but {1} found.`; muda só o `correctionMessage`).
- **No DartForge:** não existia (saía como `extra_positional_arguments`, FP lá). **Mudança:** na mesma função (feito).

##### `not_enough_positional_arguments` (perda 41: FN 4, FP 2, pos 35)
- **Emissão:** `resolver.dart:4318-4331` → `_reportNotEnoughPositionalArguments` (:4371-4446); constantes de enum **sem** argumentos (`v;`) com construtor sem nome que exige posicionais: `resolver.dart:2531-2544` (só com `enhanced-enums`).
- **Condição exata:** passo 5 (`positionalArgumentCount` inclui `super.x`).
- **Posição:** um **token**: `lastPositionalArgument.endToken.next` (o token logo depois do último posicional — `,` ou `)`), ou, sem posicionais, `argumentList.leftParenthesis.next` (o primeiro token depois do `(`: `)` ou o rótulo do primeiro nomeado), ou o `)`. Enum sem argumentos: o nome da constante.
- **Mensagem (4 variantes):** `isPlural = requiredParameterCount > 1`; com nome: `NAME_PLURAL` `'{0} positional arguments expected by '{2}', but {1} found.'` / `NAME_SINGULAR` `'1 positional argument expected by '{0}', but 0 found.'`; sem nome: `PLURAL` `'{0} positional arguments expected, but {1} found.'` / `SINGULAR` `'1 positional argument expected, but 0 found.'`. O nome (`nameNode` = pai da lista de argumentos):
  - criação: `constructorName.name` (inclusive `'new'` em `C.new(...)`) ou `'<lexema do tipo>.new'` (sem prefixo nem argumentos: `'A.new'`);
  - `this(...)`/`this.n(...)`: o nome, ou `'<returnType.getDisplayString()>.new'` (com parâmetros de tipo: `'A<T>.new'`);
  - `super(...)`/`super.n(...)`: o nome, ou `'<tipo de retorno do construtor da superclasse substituído>.new'` (`'A<int>.new'`);
  - `MethodInvocation`: o nome do método;
  - `FunctionExpressionInvocation`: o identificador se a função é `SimpleIdentifier` (`v(...)`, `getter()` implícito), senão **sem nome** (`(f)()`, `x.g()`, `f()()`);
  - constante de enum: `declaredElement.type.getDisplayString()` (`'E'`, ou `'E<int>'` em enum genérico);
  - anotação: `@A()` → `'A.new'`; `@A.n()` → `'n'`; `@p.A()` → `'A'`; `@p.A.n()` → `'A'` (o `identifier` do nome prefixado, ignorando o construtor); `@A<int>.n()` → `'A.new'`.
- **No DartForge:** texto genérico na lista inteira (35 posições erradas, mensagens sem as variantes). FP: `AugmentationFormalParameterShape…` (augmentation) e `UndefinedMethod__localSetterShadowingEx…` (o analyzer não invoca: `noGetterIsPossible` → `UNDEFINED_METHOD`). FN: `enumConst_35efe1a3` (enum sem argumentos), `dotShorth_*` (3.10).
  **Mudança (escrita no working tree):** `verificar_aridade` calcula o token com `proximo_token` (pula espaços e comentários) e escolhe a variante; o nome vem do `AlvoDaAridade` definido por cada chamador (`chamadas.rs::chamada`/`instanciacao`, `funcoes.rs` para `super`/`this`, constantes de enum e anotações); enum sem argumentos em `chamadas.rs::aridade_sem_argumentos`. Falta o `localSetterShadowing` (é do `undefined_method`).

##### `missing_required_argument` (perda 12: FN 2, pos 10)
- **Emissão:** `RequiredParametersVerifier` (`analyzer/lib/src/error/required_parameters_verifier.dart:21-135`), chamado pelo `ErrorVerifier` (`error_verifier.dart:270`, e :1440-1444 para `super(...)` com `enclosingConstructor`).
- **Condição exata:** para cada parâmetro **nomeado obrigatório** (`required`) do elemento/tipo invocado, sem argumento nomeado com o mesmo nome, e (em `super(...)`) sem `super.x` nomeado com esse nome. (A variante com `@required` legada é `WarningCode.MISSING_REQUIRED_PARAM`.)
- **Posição (`errorEntity`):** `MethodInvocation` → o nome do método; `m.call(...)` com alvo de tipo função → a lista de argumentos; `FunctionExpressionInvocation` → a invocação inteira; `InstanceCreationExpression` → o `constructorName` (tipo com prefixo e argumentos de tipo + `.nome`); `this(...)`/`super(...)` → a invocação inteira (`super.n(...)` inclusive); constante de enum → o nome da constante (com ou sem argumentos); anotação → o identificador do construtor, ou o da classe (`@A()` → `A`; `@A.n()` → `n`; `@p.A()` → `A`).
- **Mensagem:** `The named parameter '{0}' is required, but there's no corresponding argument.`
- **No DartForge:** antes, na lista de argumentos (10 posições erradas). FN: constante de enum sem argumentos (`enumConstant_with_86449ed9`) e `constructor_field_ce0ca2ef` (nome vazio `''`: parâmetro `required` de campo privado/sintaxe nova). **Mudança (escrita):** entidade vinda do `AlvoDaAridade`; enum sem argumentos em `aridade_sem_argumentos`.

##### `duplicate_named_argument` (perda 7: FN 7)
- **Emissão:** `resolver.dart:4308-4315` (passo 4).
- **Condição:** nome de argumento nomeado já usado na mesma lista, ou já coberto por um `super.x` nomeado do construtor corrente (em `super(...)`).
- **Posição:** o rótulo do argumento repetido (a segunda ocorrência). **Mensagem:** `The argument for the named parameter '{0}' was already specified.`
- **No DartForge:** inexistente; e o `super.a` + `a:` explícito saía como `UNDEFINED_NAMED_PARAMETER` (FP) porque `sem_parametros_super` tirava `a` da assinatura. **Mudança (escrita):** em `verificar_aridade`.

##### `undefined_named_parameter` (perda 3: FP 3)
- **Emissão:** `resolver.dart:4297-4303` (passo 4). **Posição:** o rótulo. **Mensagem:** `The named parameter '{0}' isn't defined.`
- **No DartForge:** existe (32 acertos, publicado? não). FP: `DuplicateNamedArgument__constructor_sup…` (ver acima), `ExperimentalMemberUse__incorrectlyNeste…`, `UseOfPrivateParameterName__andVoidLhsError` (parâmetro nomeado privado `_x` de sintaxe nova). **Mudança:** o primeiro já resolvido; os outros dois dependem de sintaxe nova.

##### `positional_super_formal_parameter_with_positional_argument` (perda 2: FN 2)
- **Emissão:** `verifySuperFormalParameters` (`super_formal_parameters_verifier.dart:22-33`), chamado com `errorReporter` em `resolveArgumentsToParameters` (super explícito, `hasExplicitPositionalArguments: positionalArgumentCount != 0`).
- **Condição:** o `super(...)` explícito tem argumento posicional **e** o construtor tem `super.x` posicional. **Posição:** o nome de **cada** `super.x` posicional. **Mensagem:** `Positional super parameters can't be used when the super constructor invocation has a positional argument.`
- **No DartForge:** inexistente. **Mudança:** em `funcoes.rs::chamar_construtor_de` (onde o alvo já conta os `super.x`), ou em `crates/analise/src/construtores.rs`.

##### `super_formal_parameter_without_associated_positional` (perda 12: FN 12)
- **Emissão:** `ErrorVerifier.visitSuperFormalParameter` (`analyzer/lib/src/generated/error_verifier.dart:1456-1493`).
- **Condição:** (não em extension type; construtor gerador não redirecionador) `superConstructorParameter == null` (`analyzer/lib/src/dart/element/element.dart:9291-9311`): o construtor da superclasse **invocado** (`superConstructor`: o do `super.n(...)` explícito, ou o sem nome implícito) existe, e o índice deste `super.x` **entre os `super.x` do construtor** (`indexIn`, :9318-...) não é menor que o número de parâmetros posicionais (obrigatórios + opcionais) daquele construtor — os posicionais explícitos do `super(...)` **não** deslocam o índice. Sem `superConstructor` (superclasse sem o construtor) não relata.
- **Posição:** o nome do parâmetro. **Mensagem:** `No associated positional super constructor parameter.`
- **No DartForge:** existe em `crates/analise/src/construtores.rs:533-551`, mas o placar mostra **nosso 0** — o bloco não é alcançado (ver `implicit_super_initializer_missing_arguments`). FN `primary_constructors/cycle_error_test` são sintaxe nova.

##### `super_formal_parameter_without_associated_named` (perda 6: FN 6)
- **Emissão/condição:** a mesma (`error_verifier.dart:1484-1488`): `super.x` nomeado sem parâmetro nomeado `x` no construtor da superclasse. **Posição:** o nome. **Mensagem:** `No associated named super constructor parameter.`
- **No DartForge:** idem (`construtores.rs:543-546`, nosso 0).

##### `implicit_super_initializer_missing_arguments` (perda 22: FN 22)
- **Emissão:** `ErrorVerifier._checkForUndefinedConstructorInInitializerImplicit` (`analyzer/lib/src/generated/error_verifier.dart:5444-5552`), de `visitConstructorDeclaration` (:606).
- **Condição exata:** construtor gerador, não `external`, sem inicializador `super(...)`/`this(...)`; superclasse com algum construtor gerador; o construtor sem nome da superclasse existe e não é factory; com `super-parameters` habilitado: `requiredPositional(super ctor) > #super.x posicionais` ou algum nomeado obrigatório do super ctor não coberto pelos `super.x` nomeados.
- **Posição:** do `returnType` (nome da classe no construtor) ao fim do nome do construtor (`B.named` → `B.named`; `B` → `B`). Para construtor **primário** (sintaxe nova, 3.13) o oráculo aponta o nome da classe no cabeçalho.
- **Mensagem:** `The implicitly invoked unnamed constructor from '{0}' has required parameters.` — `{0}` = `superType` (DartType: `'A'`, `'A<int>'`).
- **Supressões e ordem:** sem `super-parameters` → `NO_DEFAULT_SUPER_CONSTRUCTOR_EXPLICIT`; sem construtor sem nome → `UNDEFINED_CONSTRUCTOR_IN_INITIALIZER_DEFAULT`; factory → `NON_GENERATIVE_CONSTRUCTOR`. Classe **sem construtores** usa outro caminho (`_checkForNoDefaultSuperConstructorImplicit`, :4634-4678: `NO_DEFAULT_SUPER_CONSTRUCTOR_IMPLICIT`).
- **No DartForge:** `crates/analise/src/construtores.rs:552-588` implementa a regra, mas **nosso 0** no placar: o trecho não é alcançado ou o diagnóstico não chega à paridade (investigar `cx.superclasse`/`cx.construtor`/`cx.assinatura` e se o passe de construtores roda para classes comuns). As 22 FN incluem 13 de `no_default_super_constructor/*` simples (`B.named();` com `A({required int? a})`). `{0}` usa só o nome da classe (`nome_sup`): deveria ser o tipo com argumentos.

##### `new_with_undefined_constructor_default` (perda 26: FN 26)
- **Emissão:** `ErrorVerifier._checkForNewWithUndefinedConstructor` (`error_verifier.dart:4596-4626`), de `visitInstanceCreationExpression` (não const, :1117-1119); e `C.new(...)` em `method_invocation_resolver.dart:917-929`.
- **Condição exata:** criação (explícita `new C()` ou implícita `C()`) cujo construtor não resolveu; o tipo não é enum nem mixin (já relatado); sem nome de construtor → este código.
- **Posição:** o `constructorName` (para `new C()`/`C()`: o tipo nomeado com prefixo e argumentos de tipo; `C.new()` implícito: o `new`). **Mensagem:** `The class '{0}' doesn't have an unnamed constructor.` — `{0}` = `namedType.qualifiedName` (`'p.C'` com prefixo; o nome do **alias** quando via typedef: `'T'`); no caminho `C.new`, `receiver.displayName`.
- **No DartForge:** só `C.new` (`expr.rs:1681-1683`). FN: `new A()`/`A()` sem construtor sem nome (`call/nonexistent_constructor_error_test`, `constructor/unresolved_default_constructor_test`), **aplicação de mixin** sem construtor encaminhado (`mixin_constructor_forwarding/*`, 8: só construtores geradores da superclasse são encaminhados, e com parâmetros opcionais não… regra de encaminhamento), **alias de `FutureOr`/`Null`** (`T()` com `typedef T = FutureOr<…>`/`Null`, 16: o elemento aliasado não tem construtor).
  **Mudança:** em `chamadas.rs::instanciacao`, no `let Some(f) = f else` (construtor não achado e classe com construtores declarados ou alias de classe sem construtor), relatar; em `chamada` (criação implícita via `referencia_a_tipo`) idem. **Código publicado** (`verificados.txt`): cuidado com FP.

##### `new_with_undefined_constructor` (perda 14: FN 14)
- **Emissão:** a mesma função (`error_verifier.dart:4612-4618`) com nome de construtor. **Posição:** o nome do construtor. **Mensagem:** `The class '{0}' doesn't have a constructor named '{1}'.` — `{0}` `qualifiedName` (`'private.Class'` com prefixo), `{1}` o nome.
- **Condição extra:** a forma implícita `C.n()` sem construtor `n` **não** é reescrita para criação (o `AstRewriter` só reescreve se o construtor existe) — sai `UNDEFINED_METHOD`; logo este código vem de `new`/`const`, de `C<T>.n()` (sempre criação) e de `p.C.n()`.
- **No DartForge:** inexistente fora de `C<T>.new`. FN: `new A.bar()`, `mixin/illegal_constructor_test` (`new C0.named()` em aplicação de mixin), `with_two_implicit_constructors`, aliases, construtor privado de outra biblioteca (`new Class._constructor()`). **Mudança:** idem em `instanciacao`. **Publicado.**

##### `instantiate_abstract_class` (perda 4: FN 4)
- **Emissão:** `error_verifier.dart:2950-2973`. **Condição:** `ClassElement` abstrata, construtor resolvido não factory. **Posição:** o `NamedType`. **Mensagem:** `Abstract classes can't be instantiated.`
- **No DartForge:** existe (29). As 4 FN são `dot_shorthands/*` (3.10, sintaxe nova) — fora do 3.6.2.

##### `mixin_instantiate` (perda 2: FN 2)
- **Emissão:** `error_verifier.dart:2976-2984`. **Condição:** criação cujo tipo é `MixinElement` — `new M()`/`const M()`, e também a implícita `M()`/`M.n()` (o `AstRewriter` reescreve qualquer `InterfaceElement`, `analyzer/lib/src/dart/resolver/ast_rewrite.dart:139`, :168, :228). O construtor indefinido do mixin fica mudo (`_checkForNewWithUndefinedConstructor` pula mixin). **Posição:** o `NamedType`. **Mensagem:** `Mixins can't be instantiated.`
- **No DartForge:** inexistente; `chamadas.rs::instanciacao` (classe de tipo mixin) antes do construtor indefinido (que fica mudo para mixin).

##### `no_annotation_constructor_arguments` (perda 9: FN 9)
- **Emissão:** `ConstantVerifier.visitAnnotation` (`analyzer/lib/src/dart/constant/constant_verifier.dart:103-128`).
- **Condição:** a anotação resolve a um **construtor** `const` e não tem lista de argumentos (`@A` com `A` classe, `@A.named` sem `()`, `@Native` sem `()`). Construtor não const → `NON_CONSTANT_ANNOTATION_CONSTRUCTOR` antes.
- **Posição:** a anotação inteira (do `@` ao fim do nome). **Mensagem:** `Annotation creation must have arguments.`
- **No DartForge:** inexistente; `funcoes.rs::validar_anotacao` (quando o nome resolve a classe/construtor e `m.arguments` é `None`).

## B. Fluxo, nulidade, padrões, código morto

### Especificação — família B (fluxo, nulidade, padrões) — r3-b

Fonte: `E:\references\dart-sdk-3.6.2\pkg` (analyzer 6.11 = SDK 3.6.2). Citações `analyzer/lib/...:linha` e
`_fe_analyzer_shared/lib/...:linha` desse diretório. Perdas de `E:\dftemp\analise\trab\familia-B.txt`
(colunas oráculo/nosso/acerto/msg/pos/FP/FN). Amostras de `placar-base-r3.txt` (ver com
`python E:/dftemp/r3-b/am.py <codigo>`).

Convenções usadas abaixo:
- **potencialmente anulável** = `!TypeSystemImpl.isNonNullable(T)` (`analyzer/lib/src/dart/element/type_system.dart:1194-1217`, `:1287`):
  `dynamic`, `InvalidType`, `_` (desconhecido), **`void`**, `Null`, `T?`, `FutureOr<S>` com `S` anulável, parâmetro de tipo sem
  limite não anulável (ou promovido para um anulável), tipo de extensão sem `implements` (representação qualquer) — todos são
  potencialmente anuláveis. No DartForge `e_nao_anulavel(t) = t <: Object` (`crates/types/src/inferencia/tipos.rs:59`) é a mesma
  coisa exceto pelo tipo de extensão (ver abaixo) e pelo inválido.
- **dynamic-bounded** = `isDynamicBounded` (`type_system.dart`): `dynamic`, ou parâmetro de tipo/interseção cujo limite
  (transitivamente) é dynamic-bounded. Idem **invalid-bounded**.
- O fluxo de padrões do analyzer (`_fe_analyzer_shared/lib/src/flow_analysis/flow_analysis.dart`) é descrito uma vez em
  **§ Apêndice A** (usado por `dead_code`, `constant_pattern_never_matches_value_type`, `unnecessary_*_pattern` etc.).
- O verificador de código morto do analyzer é descrito em **§ Apêndice B** (usado por `dead_code`).


#### Índice

| código | perda | FN | FP | msg | pos | seção |
|---|---|---|---|---|---|---|
| `unchecked_use_of_nullable_value` | 178 | 151 | 12 | 0 | 15 | §1 Nulidade de receptor |
| `dead_code` | 127 | 118 | 5 | 0 | 4 | §2 Código morto |
| `referenced_before_declaration` | 29 | 29 | 0 | 0 | 0 | §3 Referência antes da declaração |
| `pattern_type_mismatch_in_irrefutable_context` | 22 | 22 | 0 | 0 | 0 | §4 Padrões |
| `relational_pattern_operand_type_not_assignable` | 18 | 18 | 0 | 0 | 0 | §4 Padrões |
| `not_initialized_non_nullable_instance_field` | 15 | 12 | 2 | 0 | 1 | §6 Nulidade e verificadores |
| `pattern_variable_assignment_inside_guard` | 15 | 15 | 0 | 0 | 0 | §4 Padrões |
| `unnecessary_cast_pattern` | 13 | 13 | 0 | 0 | 0 | §4 Padrões |
| `invalid_use_of_null_value` | 12 | 12 | 0 | 0 | 0 | §5 Fluxo e atribuição definitiva |
| `missing_variable_pattern` | 12 | 12 | 0 | 0 | 0 | §4 Padrões |
| `unnecessary_null_check_pattern` | 12 | 12 | 0 | 0 | 0 | §4 Padrões |
| `unnecessary_set_literal` | 12 | 12 | 0 | 0 | 0 | §6 Nulidade e verificadores |
| `assignment_to_final_local` | 11 | 11 | 0 | 0 | 0 | §5 Fluxo e atribuição definitiva |
| `invalid_pattern_variable_in_shared_case_scope` | 11 | 11 | 0 | 0 | 0 | §4 Padrões |
| `late_final_field_with_const_constructor` | 10 | 10 | 0 | 0 | 0 | §6 Nulidade e verificadores |
| `switch_case_completes_normally` | 10 | 10 | 0 | 0 | 0 | §5 Fluxo e atribuição definitiva |
| `unnecessary_wildcard_pattern` | 10 | 10 | 0 | 0 | 0 | §4 Padrões |
| `body_might_complete_normally` | 8 | 8 | 0 | 0 | 0 | §5 Fluxo e atribuição definitiva |
| `dead_null_aware_expression` | 8 | 6 | 2 | 0 | 0 | §6 Nulidade e verificadores |
| `equal_keys_in_map_pattern` | 8 | 8 | 0 | 0 | 0 | §4 Padrões |
| `not_assigned_potentially_non_nullable_local_variable` | 8 | 8 | 0 | 0 | 0 | §5 Fluxo e atribuição definitiva |
| `not_initialized_non_nullable_variable` | 8 | 8 | 0 | 0 | 0 | §6 Nulidade e verificadores |
| `pattern_never_matches_value_type` | 8 | 5 | 2 | 1 | 0 | §4 Padrões |
| `unnecessary_null_comparison` | 8 | 8 | 0 | 0 | 0 | §6 Nulidade e verificadores |
| `unnecessary_type_check` | 8 | 3 | 5 | 0 | 0 | §6 Nulidade e verificadores |
| `non_nullable_equals_parameter` | 7 | 7 | 0 | 0 | 0 | §6 Nulidade e verificadores |
| `null_argument_to_non_null_type` | 7 | 7 | 0 | 0 | 0 | §6 Nulidade e verificadores |
| `pattern_assignment_not_local_variable` | 7 | 7 | 0 | 0 | 0 | §4 Padrões |
| `unnecessary_null_assert_pattern` | 7 | 7 | 0 | 0 | 0 | §4 Padrões |
| `cast_from_null_always_fails` | 6 | 6 | 0 | 0 | 0 | §6 Nulidade e verificadores |
| `duplicate_pattern_field` | 6 | 6 | 0 | 0 | 0 | §4 Padrões |
| `not_null_aware_null_spread` | 6 | 6 | 0 | 0 | 0 | §6 Nulidade e verificadores |
| `unnecessary_nan_comparison` | 5 | 5 | 0 | 0 | 0 | §6 Nulidade e verificadores |
| `body_might_complete_normally_nullable` | 4 | 4 | 0 | 0 | 0 | §5 Fluxo e atribuição definitiva |
| `break_label_on_switch_member` | 4 | 4 | 0 | 0 | 0 | §4 Padrões |
| `case_expression_type_is_not_switch_expression_subtype` | 4 | 4 | 0 | 0 | 0 | §4 Padrões |
| `duplicate_pattern_assignment_variable` | 4 | 4 | 0 | 0 | 0 | §4 Padrões |
| `null_check_always_fails` | 4 | 4 | 0 | 0 | 0 | §6 Nulidade e verificadores |
| `nullable_type_in_catch_clause` | 4 | 4 | 0 | 0 | 0 | §6 Nulidade e verificadores |
| `nullable_type_in_implements_clause` | 4 | 4 | 0 | 0 | 0 | §6 Nulidade e verificadores |
| `refutable_pattern_in_irrefutable_context` | 4 | 4 | 0 | 0 | 0 | §4 Padrões |
| `unnecessary_question_mark` | 4 | 4 | 0 | 0 | 0 | §6 Nulidade e verificadores |
| `body_might_complete_normally_catch_error` | 3 | 3 | 0 | 0 | 0 | §5 Fluxo e atribuição definitiva |
| `dead_code_on_catch_subtype` | 3 | 3 | 0 | 0 | 0 | §5 Fluxo e atribuição definitiva |
| `definitely_unassigned_late_local_variable` | 3 | 1 | 0 | 0 | 2 | §5 Fluxo e atribuição definitiva |
| `duplicate_variable_pattern` | 3 | 3 | 0 | 0 | 0 | §4 Padrões |
| `unnecessary_final` | 3 | 3 | 0 | 0 | 0 | §6 Nulidade e verificadores |
| `cast_from_nullable_always_fails` | 2 | 2 | 0 | 0 | 0 | §6 Nulidade e verificadores |
| `expected_two_map_pattern_type_arguments` | 2 | 2 | 0 | 0 | 0 | §4 Padrões |
| `late_final_local_already_assigned` | 2 | 1 | 1 | 0 | 0 | §5 Fluxo e atribuição definitiva |
| `read_potentially_unassigned_final` | 2 | 2 | 0 | 0 | 0 | §5 Fluxo e atribuição definitiva |
| `relational_pattern_operator_return_type_not_assignable_to_bool` | 2 | 2 | 0 | 0 | 0 | §4 Padrões |
| `rest_element_in_map_pattern` | 2 | 2 | 0 | 0 | 0 | §4 Padrões |
| `unnecessary_no_such_method` | 2 | 2 | 0 | 0 | 0 | §6 Nulidade e verificadores |
| `dead_code_catch_following_catch` | 1 | 1 | 0 | 0 | 0 | §5 Fluxo e atribuição definitiva |
| `duplicate_rest_element_in_pattern` | 1 | 1 | 0 | 0 | 0 | §4 Padrões |
| `empty_map_pattern` | 1 | 1 | 0 | 0 | 0 | §4 Padrões |
| `expected_one_list_pattern_type_arguments` | 1 | 1 | 0 | 0 | 0 | §4 Padrões |
| `invalid_null_aware_operator` | 1 | 1 | 0 | 0 | 0 | §6 Nulidade e verificadores |
| `non_exhaustive_switch_expression` | 1 | 0 | 0 | 1 | 0 | §4 Padrões |

#### Resumo e ordem sugerida de implementação

1. **`unchecked_use_of_nullable_value` + `invalid_use_of_null_value` (~190):** mecanismo `NullableDereferenceVerifier` para
   iterável/espalhamento/`yield*` (68+8), invocação de valor anulável (38), prefixo/posfixo/composto (~17+5), `this` implícito
   em extensão, escrita em propriedade, campo de padrão objeto; e as supressões que hoje dão FP (`new`, `T extends dynamic`,
   `super` em extensão). Implementação de teste já escrita (revertida): `E:\dftemp\r3-b\meu-instrucoes.diff` e as chamadas em
   `colecoes.rs`/`chamadas.rs`/`expr.rs` descritas no §1.
2. **Fluxo de padrões (Apêndice A) (~100 em vários códigos):** referência do valor casado + `_unmatched`; destrava
   `dead_code` de padrões (~46), `constant_pattern_never_matches_value_type` (casos com tipo promovido), `pattern_never_matches_value_type`,
   `unnecessary_null_check/assert_pattern`, `unnecessary_wildcard_pattern`, `unnecessary_cast_pattern` (tipo casado certo).
3. **`dead_code` no nível de expressão (Apêndice B) (~40):** gancho de "nó inalcançável" em toda expressão e na lista de
   argumentos; operando direito de binário começa no operador; `?.` sobre `Null`; variáveis de condição com literal booleano
   (18); elementos de coleção; `is` com fator `Never` não torna o ramo falso inalcançável (FP).
4. **Códigos de padrão não implementados (~110 somados):** `pattern_type_mismatch_in_irrefutable_context`,
   `relational_pattern_operand_type_not_assignable`, `pattern_variable_assignment_inside_guard`, `missing_variable_pattern`,
   `invalid_pattern_variable_in_shared_case_scope`, `equal_keys_in_map_pattern`, `pattern_assignment_not_local_variable`,
   `duplicate_*`, `refutable_pattern_in_irrefutable_context`… — cada um local em `padroes::tipar`.
5. **`referenced_before_declaration` (29, publicado):** pré-escaneamento de `PatternVariables` no bloco e dos corpos de
   `case`; nome de tipo que é local adiante; variáveis de padrão referidas no próprio padrão.
6. **Fluxo/atribuição:** `for (x in …)` com `x` final (sempre relata), formais `this.x`/`super.x` são `final`, `??=` escreve só
   no ramo nulo (FP de `late_final_local_already_assigned`), leitura em código inalcançável mantém o estado de atribuição,
   `T` sem limite é potencialmente não anulável; `body_might_complete_normally*` e `switch_case_completes_normally` (legado)
   não implementados.
7. **Achado transversal:** escrita de valor de tipo inválido promove o local ao tipo de interesse não anulável (§6).

#### §1 Nulidade de receptor

##### `unchecked_use_of_nullable_value` (perda 178: FN 151, FP 12, msg 0, pos 15)

Nome compartilhado de 8 códigos (`analyzer/messages.yaml`, `CompileTimeErrorCode.UNCHECKED_*`):
`UNCHECKED_INVOCATION_OF_NULLABLE_VALUE` ("The function can't be unconditionally invoked because it can be 'null'.", sem
argumentos), `UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE` ("The method '{0}' …"), `UNCHECKED_OPERATOR_INVOCATION_OF_NULLABLE_VALUE`
("The operator '{0}' …"), `UNCHECKED_PROPERTY_ACCESS_OF_NULLABLE_VALUE` ("The property '{0}' …"), `…_AS_CONDITION`,
`…_AS_ITERATOR`, `…_IN_SPREAD`, `…_IN_YIELD_EACH` (sem argumentos). `{0}` é o nome do membro como String (`'unary-'`, `'[]'`,
`'+'` para `++`, `'-'` para `--`).

- **Emissão (dois mecanismos):**
  1. `NullableDereferenceVerifier.expression` → `_check` (`analyzer/lib/src/error/nullable_dereference_verifier.dart:33-37`,
     `:72-90`): relata no **nó da expressão inteira**. Chamado por:
     - `for-in` comum: `ForResolver._forEachParts` (`analyzer/lib/src/dart/resolver/for_resolver.dart:164-169`), depois de
       inferir o iterável com contexto `Iterable<T>`/`Stream<T>`; `for` com padrão: `_analyzePatternForIn` (`:85-91`). Vale
       para instrução e para elemento de coleção (o mesmo resolvedor).
     - espalhamento sem `?`: `ResolverVisitor.visitSpreadElement` (`analyzer/lib/src/generated/resolver.dart:3673-3692`,
       só `if (!node.isNullAware)`).
     - `yield*`: `YieldStatementResolver._resolve_generator` (`analyzer/lib/src/dart/resolver/yield_statement_resolver.dart:171-175`,
       só com `star`).
     - invocação de expressão cujo tipo é `FunctionType`: `FunctionExpressionInvocationResolver.resolve`
       (`analyzer/lib/src/dart/resolver/function_expression_invocation_resolver.dart:52-58`) → `UNCHECKED_INVOCATION…` no nó
       `function` (ex.: `f()` com `f` local; `this._f()` → `this._f`; `super._f()` → `super._f`; `x.first()` → `x.first`).
       Antes: `_checkForUseOfVoidResult` (`:47`, `void` sai como `use_of_void_result` e mais nada).
     - condição: `BoolExpressionVerifier` (`analyzer/lib/src/error/bool_expression_verifier.dart:61`, já implementado).
     `_check` (`:77-80`): sai sem relatar se o tipo é `DynamicType`, `InvalidType` ou **não** potencialmente anulável.
     `report` (`:39-64`): se o tipo do receptor é exatamente `Null` (`typeProvider.nullType`; `Never?` NÃO é) troca para
     `INVALID_USE_OF_NULL_VALUE` sem argumentos.
  2. `TypePropertyResolver.resolve` (`analyzer/lib/src/dart/resolver/type_property_resolver.dart:62-179`), chamado em toda
     busca de membro: invocação de método (`method_invocation_resolver.dart:805`, entidade = nome), leitura/escrita de
     propriedade (`property_element_resolver.dart:481`, entidade = nome da propriedade), índice (`property_element_resolver.dart:109`,
     entidade = `[`), binário (`binary_expression_resolver.dart:434`, entidade = operador), prefixo `-x ~x ++x --x`
     (`prefix_expression_resolver.dart:185-191`, entidade = token do operador), posfixo `x++ x--`
     (`postfix_expression_resolver.dart:136-141`, entidade = operador), atribuição composta `x += 1`
     (`assignment_expression_resolver.dart:241-247`, entidade = operador), `this` implícito (`this_lookup.dart:28`, `:69`,
     receptor `null`, entidade = o identificador), `call` de valor não-função (`function_expression_invocation_resolver.dart:73-79`,
     entidade = `function`), padrão relacional (`generated/resolver.dart:1627-1634`, entidade = operador, `parentNode: node`),
     campo de padrão objeto (`generated/resolver.dart:1574-1580`, receptor `null`, entidade = `objectPattern.type`), referência
     a função (`function_reference_resolver.dart:683`).
- **Condição exata** (`type_property_resolver.dart`):
  ```
  if name == 'new' → needsGetterError, sem relato de nulo (:75-79)
  if isDynamicBounded(R) || isInvalidBounded(R) → busca em Object, sem erro (:81-91)
  isNullable = R.isExtensionType ? R.nullabilitySuffix == '?' : isPotentiallyNullable(R)   (:93-97)
  if isNullable:
     acha em Object (getter/setter/método do nome)  → OK, sem relato (:100-103)
     acha em extensão aplicável a R (o tipo anulável) → OK (:105-108)
     parentNode ??= receiver?.parent ?? (propertyErrorEntity as AstNode).parent   (:110-122)
     if parentNode is CascadeExpression: parentNode = cascadeSections.first        (:128-130)
     código = parentNode is BinaryExpression|RelationalPattern → OPERATOR [name]  (:131-134)
              parentNode is MethodInvocation|MethodReferenceExpression → METHOD [name] (:135-139)
                 (MethodReferenceExpression = Assignment, Index, Prefix, Postfix, ImplicitCallReference — ast.dart:850,9789,10128,14230,14441)
              parentNode is FunctionExpressionInvocation → INVOCATION []          (:140-143)
              senão → PROPERTY [name]                                             (:144-147)
     nullableDereferenceVerifier.report(código, propertyErrorEntity, R)  (Null → INVALID_USE_OF_NULL_VALUE)  (:165-166)
     recuperação: R = resolveToBound(R); se interface, busca nela (:172-176) — por isso não sai undefined_* depois
  ```
  `void` é potencialmente anulável: `-x` com `x` `void` dá `UNCHECKED_METHOD…('unary-')` no `-` **e** `use_of_void_result` no
  operando (amostra `use_of_void_result/UseOfVoidResult__prefixExpression_minus_2247a990.dart`, `void/void_type_usage_test.dart:180`).
  Leitura de propriedade/invocação em `void` sai antes como `use_of_void_result` (outros caminhos), sem este código.
  `Never` (não anulável) vai para `RECEIVER_OF_TYPE_NEVER` antes (`postfix…:128`, `prefix…:176`); `Never?` não é `Never` nem
  `Null` → este código (amostras `receiver_of_type_never/InvalidUseOfNever__*neverQ*`).
- **Posição:** o `propertyErrorEntity` acima (nome do método/propriedade; `[` do índice, inclusive em cascata `x..[0] = 1`;
  token do operador em binário/prefixo/posfixo/composto; identificador do `this` implícito; tipo do padrão objeto
  `case A(foo: 0)` → `A`); para o mecanismo 1, a expressão inteira (iterável, operando do espalhamento, valor do `yield*`,
  função invocada).
- **Mensagem:** só nome (String); nenhuma formatação de tipo.
- **Supressões e ordem:** `name == 'new'`; receptor `dynamic`/inválido/dynamic-bounded (inclusive `T extends dynamic`);
  `Object` ou extensão aplicável ao tipo anulável tem o membro; sobreposição explícita de extensão (`E(x).m`, não passa por
  aqui); `super` numa extensão já é `super_in_extension` com tipo inválido (sem este código); receptor `Never`.
- **No DartForge:**
  - Mecanismo 2 existe parcialmente em `BodyInferrer::exige_checagem_de_nulo` (`crates/types/src/inferencia/membros.rs:343-356`),
    usado em `expr::propriedade` (leitura), `chamadas::chamada` (método), `expr::operador_binario` (binário e índice),
    `expr::verificar_bool` (condição). Causas das divergências:
    - **FN iterável/espalhamento/`yield*` (68):** mecanismo 1 não implementado. Entrar em `instrucoes::cabecalho_for_in`
      (logo depois de `inferir` do iterável — serve instrução, elemento de coleção e `for` com padrão), em
      `colecoes::visitar` ramo `CollectionElement::Spread` quando `!null_aware`, e em `instrucoes` `StmtKind::Yield{star:true}`.
      Regra: `t` não `dynamic`, não desconhecido, não inválido e `!e_nao_anulavel(t)` → `aviso_de_nulo(t, código, span da expressão)`.
      Amostras: `patterns/for_in_nullable_error_test.dart:9:19`, `why_not_promoted/nullable_spread_error_test.dart`,
      `nnbd/operator_type_test.dart:115:12`. A confirmar com a sonda: `for (x in v)` com `v` `void` (o analyzer conta `void`
      como anulável aqui, sem `_checkForUseOfVoidResult` antes no `for`). (Implementação de teste já escrita e guardada em
      `E:\dftemp\r3-b\meu-instrucoes.diff`, revertida do repositório.)
    - **FN invocação de função anulável (38):** `chamadas::invocar_valor` não relata. Regra: se o tipo `t` do alvo é função
      (após tirar `?`) e potencialmente anulável → relatar no span do alvo (`ExprKind::Call.target`), código INVOCATION;
      se não é função e é potencialmente anulável (`Function?`, `C?` com `call`, `T` com limite anulável, `Never?`, `Null`) e
      nenhuma extensão aplicável ao tipo anulável tem `call` → idem. Amostras `inference_update_2/super_this_distinction_error_test.dart:26:5`
      (`_f()`), `:31:5` (`this._f()`, length 7), `:36:5` (`super._f()`), `use_of_nullable_value/UncheckedUseOfNullableValue__member_pot_7cd99204.dart`
      (`x.first()` com `T?`), `receiver_of_type_never/InvalidUseOfNever__functionExpressionIn_d97a3872.dart` (`Never?`),
      `why_not_promoted/nullable_expression_call_error_test.dart` (5).
    - **FN prefixo/posfixo/composto (~17):** `expr::unario` (`UnaryOp::Neg/BitNot` e `PrefixInc…PostfixDec`) não checa nulo.
      Regra: depois de inferir o operando (leitura), se não há sobreposição de extensão no operando, tipo não `dynamic`/`Never`/
      desconhecido e potencialmente anulável **incluindo `void`**, e nem `Object` nem extensão aplicável ao tipo anulável tem o
      operador → `aviso_de_nulo(t, METHOD, token, [nome])`, token = `[início, início+len)` (prefixo: 1 para `-`/`~`, 2 para
      `++`/`--`) ou `[fim-2, fim)` (posfixo); a busca segue no tipo não anulável (recuperação). Amostras
      `UncheckedUseOfNullableValue__operatorPo_020f2a12.dart` (`x++`), `null_aware/prefix_not_shortening_test.dart:13:3`
      (`-c?.e()`: o `?.` não encurta através do prefixo), `void/void_type_usage_test.dart:180:3` (6 vezes nesse arquivo),
      `null_safety_read_write/ReadWrite__lateFinalNullable_*` (4), `super/conditional_operator_test.dart:48:5`.
      Atribuição composta (`x += 1`, `a.b += 1`, `a[i] += 1`): conferir `expr::atribuicao` — o operador usa METHOD
      (pai é `AssignmentExpression`), hoje `PosicoesDeOperador.composta`.
    - **FN `this` implícito em extensão sobre tipo anulável (5):** `RefNome::ThisImplicito` (`expr.rs`, leitura e escrita) não
      passa por `exige_checagem_de_nulo`. Código pelo pai do identificador: chamada `foo()` → METHOD; `foo = 0`
      (pai `AssignmentExpression`) → METHOD; `foo;` → PROPERTY. Amostras `UncheckedUseOfNullableValueInsideExtens_6b6a2f6f.dart:9:5`
      (`foo = 0` → "The method 'foo'"), `_c616a27b.dart:9:5`, `_8231c303.dart:7:5`, `_7fd67550.dart:14:9` (`this[0]` → `[`),
      `_2fa23de1.dart:7:5` (`-this`, prefixo).
    - **FN escrita em propriedade de receptor anulável:** `b.a.x = 2` (`UncheckedUseOfNullableValue__assignment_b2ba37a8.dart:13:7`)
      — o caminho de escrita (`expr::atribuicao` → alvo `Property`) não chama `exige_checagem_de_nulo(…, setter=true)`.
      PROPERTY no nome (pai `PropertyAccess`). Cascata `x..[0] = 1` (`…__cascade_nu_b9551aeb.dart:2:6`): METHOD `'[]'` no `[`.
    - **FN padrão objeto sobre tipo anulável:** `case A(foo: 0)` com `A` alias de tipo anulável ou `T?` — em
      `padroes::tipar` `PatternKind::Object` a busca dos campos não checa nulo. PROPERTY [campo] no span do tipo do padrão.
      Amostras `patterns/issue52202_error_test.dart:11:10`, `patterns/nullable_object_pattern_error_test.dart:13:10`, `:25:10`.
    - **FN de inferência (tipo do receptor errado no DartForge; consertar a inferência, não este código):**
      `class/override_inference_error_test.dart:237` (parâmetro sem tipo herdando `Q` → `Object?`),
      `extension_type/combined_member_signature_error_test.dart:46:35` (campos de registro da assinatura combinada →
      `Object?`), `primary_constructors/type_inference/extension_type_error_test.dart:16:20` (representação sem tipo = `Object?`),
      `records/type_inference_error_test.dart:132:20`, `:139:20` (`(d,)` com `d` dynamic em contexto `(List<_>,)` → `List<Object?>`),
      `extension_methods/static_extension_this_not_promoted_error_test.dart:21:9` (**`this` nunca é promovido** por
      `this != null`; o DartForge promove — `expr::alvo_de_promocao`/`Base::This`).
    - **FP (12):** `t.new` (`UndefinedGetter__new_*`: nome `new` nunca exige checagem); `super + 1` numa extensão
      (`SuperInExtension__binaryOperator_withGe_2100d7fc.dart`: `super` tem tipo inválido depois de `super_in_extension`);
      `T extends dynamic` (`type_variable/bound_access_test.dart:11:22`: dynamic-bounded não exige checagem);
      local inicializado de expressão de tipo inválido (`InvalidNullAwareOperator__invalid_nonNullable.dart:5:5`: `int? i =
      o.nonNull` com `o` de classe indefinida — a variável fica promovida ao tipo inválido e nada sai depois);
      cascata `c?.._field.f(…)` (`inference_update_2/cascaded_field_promotion_unnecessary_null_aware_error_test.dart:107:15`:
      o alvo da cascata guarda a promoção do campo, mesmo com escrita em `c` dentro da cascata).
    - **Conferido com o oráculo vivo** (`E:\dftemp\r3-r3-b\c1\a.dart`): `for (final x in n)` com `n: Null` →
      `invalid_use_of_null_value` (3:19); `<int,int>{...null}` → `invalid_use_of_null_value` **e** `not_null_aware_null_spread`
      no `null`; `yield* l` com `l` anulável → este código **e** `yield_of_invalid_type` no mesmo nó; `f()` (1 col), `(f)()`
      (3 cols, os parênteses entram), `c.cb()` (4), `c.gg()` getter (4), `cc()` com `C?` que tem `call` (2), `(c.cb)()` (6);
      `x++`/`++x` com `int?` → METHOD '+' no `++` (e a escrita promove `x` para `int`, por isso `-x` logo depois não relata);
      `-this` numa extensão em `int?` → METHOD 'unary-'; `foo()` implícito → METHOD 'foo'; `foo;` implícito → PROPERTY 'foo'
      **e também `undefined_identifier`** quando o tipo não anulável também não tem o membro (o `this_lookup` não acha getter
      e o `SimpleIdentifierResolver` relata o nome indefinido; na invocação não há `undefined_method`).
    - **pos (15) + FP top_merge (6):** `nnbd/top_merge/*_error_test.dart`: a interface de `A` numa classe que implementa `A<X>`
      e `A<Y>` com `X`, `Y` mutuamente subtipos é `NNBD_TOP_MERGE(NORM(X), NORM(Y))` (`analyzer/lib/src/dart/element/class_hierarchy.dart:116-212`,
      `top_merge.dart:27`); o DartForge escolhe uma das duas e erra o tipo de `deconstruct(this)` (sai `dynamic`/`void` onde é
      `Object?` e vice-versa). Não é deste código: consertar no cálculo das superinterfaces (crates/types, hierarquia).
      As 3 `posição` de `nnbd/operator_type_test.dart:98/103/110` são o mesmo FN de iterável/espalhamento pareado com outra linha.

#### §2 Código morto

##### `dead_code` (perda 127: FN 118, FP 5, msg 0, pos 4)

- **Emissão:** `WarningCode.DEAD_CODE` ("Dead code.", sem argumentos; correção "Try removing the code, or fixing the code
  before it so that it can be reached."), só pelo `NullSafetyDeadCodeVerifier` durante a resolução — modelo completo no
  **Apêndice B** (`analyzer/lib/src/error/dead_code_verifier.dart:186-475`). Também `_verifyUnassignedSimpleIdentifier`
  (`:443-475`). (O `DeadCodeVerifier` antigo de `:28-184` só cuida de rótulos/`ignore`, não emite este código em código
  null-safe.)
- **Condição exata:** trecho [primeiro nó inalcançável, fim do nó que fecha o bloco básico] (Apêndice B); a alcançabilidade é a
  da análise de fluxo (inclui expressões `Never`, variáveis de condição, `?.` sobre `Null` e padrões — Apêndice A).
- **Posição:** offset do primeiro nó morto (ou do operador `&&`/`||`/padrão `||`; ou o token `case`/`default`), até o fim do
  nó que fecha, aparado na última instrução do bloco/membro. Casos especiais: `do-while`, inicializador de construtor,
  atualizadores de `for` (`_reportForUpdaters`), mensagem de `assert` (nunca relatada).
- **Mensagem:** fixa.
- **Supressões e ordem:** um trecho por vez (enquanto `_firstDeadNode` existe, nada novo abre); dentro de `catch` morto já
  relatado (`dead_code_on_catch_subtype`/`…_following_catch`) não sai; sem fluxo (código fora de corpo) não sai.
- **No DartForge:** `Corpo::trecho_morto`/`fins_de_fluxo`/`origem_do_morto` (`crates/types/src/inferencia/corpo.rs`),
  aberto só no nível de **instrução** (`instrucoes::inferir_instrucao` `StmtKind::Block`, `ramo_de_fluxo`) e em
  `expr::operando_de_fluxo` (ramos do condicional e operando direito de `&&`/`||`); fechado por `instrucoes::sair_fluxo`.
  Grupos de FN (118) e a mudança de cada um:
  1. **Expressão `Never` no meio de uma expressão (~24):** `x.foo(1 + 2)`/`x()`/`x[0]`/`x[0] += 1`/`x.foo = 0`/`(throw '') + (1+2)`
     com `x: Never` (`receiver_of_type_never/InvalidUseOfNever__*`, `void/issue_flutter_161479_test.dart:26:13`,
     `nnbd/never/never_error_test.dart:73:26`). Falta o gancho de "nó inalcançável" no nível de expressão: ao começar a
     inferir **qualquer** expressão (`expr::inferir_no`) e **a lista de argumentos** de qualquer invocação (antes do primeiro
     argumento; offset = `Arguments.span.start`, inclusive em todos os caminhos de recuperação de `chamadas.rs` que fazem
     `inferir_livre` dos argumentos), se `!fluxo.alcancavel && trecho_morto.is_none()`: abrir trecho de `início` até
     `fins_de_fluxo.last()` (o fim aparado do bloco básico de fora) e marcar `trecho_morto`. Exceção: operando direito de
     binário (não lógico) começa no **operador** (`x + (1+2)` → `+ (1 + 2);`; `x == 1 + 2` → `== 1 + 2;`). Amostras:
     `InvalidUseOfNever__methodInvocation_never.dart` (25, 8: `(1 + 2);`), `__indexExpression_never_read.dart` (22, 3: `0];`),
     `__binaryExpression_plus.dart` (24, 9), `__functionExpressionIn_cc1f7897.dart` (`x();` → `();`).
  2. **`?.`/`?[` com alvo do tipo `Null` (6):** o lado direito é inalcançável (`nullAwareAccess_rightBegin`,
     `flow_analysis.dart:5041-5052`): `n?[i]`, `n?.foo(i)`, `n?.p = i`, `(n)?.p = i`, `x?[0]` com `x: Never?`… Trecho a partir
     do primeiro nó do lado direito (índice, argumentos, valor atribuído) até o fim do bloco básico. Amostras
     `dead_code/DeadCode__nullAware*.dart`, `InvalidUseOfNever__indexExpression_neve_ca39e5f4.dart`. No DartForge as cadeias
     (`Corpo::cadeias`, `expr::fechar_cadeia`) não tornam o fluxo inalcançável quando o alvo é `Null`.
  3. **Variável de condição com literal booleano (18):** `bool x = true; x || false` (`|| false` morto), `!x`, `Object b =
     true; b ? 1 : 2` (`2` morto), `bool condition = true; condition ? A() : B()` (10 em `variance/variance_upper_lower_bounds_error_test.dart`),
     `for (…; b; i++)`/`while (b)` (`implicit_downcast_during/*`), `const isTrue = true; if (isTrue) … else …`
     (`NonConstantMapElement__ifElementWithEls_988e819a.dart`). Regra (`_initialize` `flow_analysis.dart:5931-5960`,
     `_write` `:6133-6146`, `variableRead` `:5536-5552`): toda escrita/inicialização de local **não `late`** guarda no nó SSA a
     `ExpressionInfo` não trivial do valor escrito (literal `true`/`false` tem uma: o ramo oposto inalcançável;
     `booleanLiteral`); a leitura posterior sem escrita no meio a reaplica (`rebaseForward`). Vale para qualquer tipo
     declarado (`Object`, `bool`, `const`, `final`). No DartForge `instrucoes::declaracao_de_variaveis` só guarda
     (`Corpo::condicoes`) quando `expr::e_forma_de_condicao(init)` — falta o literal booleano (e parênteses dele) e a
     atribuição simples `x = true` (`expr::atribuicao`), e a leitura só é consultada em `expr::condicao` (correto).
  4. **Padrões (~46):** (a) operando direito de padrão `||` inalcançável porque o esquerdo sempre casa (`int() || 0` sobre
     `int`, `_ || 0`, `final a || final a`, `var (_ || _) = 0`): trecho do operador `||` até o fim do operando direito
     (`dead_code/DeadCode__deadPattern_ifCase_logicalOrP_*`, `missing_variable_pattern/*`, `patterns/switch_trivial_exhaustiveness_error_test.dart:274:12`);
     (b) `case`/`default` de `switch` instrução inalcançável (um caso anterior sempre casa, ou `null` depois de `case null`, ou
     tipo selado já coberto): só o token `case`/`default` (length 4/7) — e, se nenhum membro do grupo é alcançável, o corpo
     do grupo da 1ª à última instrução (`unreachable_switch_case/UnreachableSwitchCaseTest_SwitchStateme_10b7df07.dart`:
     `case` 64/4, `case` 195/4, `break;` 328/6; `patterns/exhaustiveness/sealed_class_switch_test.dart:168:5` + `:169:7`);
     (c) caso de `switch` expressão inalcançável: o caso inteiro `padrão => expr` (`DeadCode__deadPattern_switchExpression__c29a5c55.dart`:
     `int() => 1` 110/10, `_ => 2` 260/6); (d) campo de padrão objeto de tipo `Never` (`DeadCode__objectPattern_neverTypedGetter.dart`:
     o `then` `{}` morto). Requer o Apêndice A em `padroes.rs` (hoje `irrefutavel`/`tipo_casado`) e, em `instrucoes`
     (`StmtKind::Switch`) e `padroes::expressao_switch`, o gancho antes de cada `case` (token: depois dos rótulos; `SwitchCase.span`
     começa nos rótulos) e o relato do corpo morto do grupo (hoje o laço do corpo não relata nada: a variável `avisou` não é
     usada).
  5. **Elementos de coleção (9):** `if (false)` elemento (`spread_collections/null_spread_test.dart:77:34`: o `throw 1` do
     `then`), `[if (false) (a) = 0]`, elemento `for` com condição `false` (`why_not_promoted/argument_type_not_assignable_nullability_error_test.dart:429:32`),
     `[for (; ; y) 0]` (`nnbd/definite_assignment/definite_assignment_error_test.dart:330:5`), `[for (var (i) = throw 0; true; 1) 0]`
     (`DeadCode__flowEnd_forElementParts_initi_e2dbb486.dart`). `colecoes::visitar` (ramos `If`/`For`/`ForIn`) não abre trecho:
     falta o equivalente de `ramo_de_fluxo` (abrir no elemento se inalcançável; `flowEnd` nos ramos do `if` elemento
     `resolver.dart:1027-1040`).
  6. **`for` com declaração por padrão e `throw` no inicializador** (`DeadCode__flowEnd_forParts_initializer__3779cb5c.dart:2:27`):
     a condição/atualizadores ficam mortos (regra `ForParts`: até o último atualizador).
  7. **Legado e `do-while` (8):** `do break; while (true);` (`switch/case_fallthrough_legacy_error_test.dart:219`, `@dart=2.19`):
     regra `DoStatement` do Apêndice B (relata `do` e `while (…);` separadamente); `switch/switch1_test.dart:13:5` (`default`
     seguido de `case`, com erro de sintaxe) e `label/label8_test.dart:31:21` (`while (false) {` dentro de laço rotulado) — a
     conferir com a sonda.
  8. **Mensagem de `assert` com `throw` na interpolação** (`ConstEvalThrowsException__assertInitial_5db8e50b.dart:2:45`):
     o pedaço `}'` depois do `throw` (nó `InterpolationString`) é o primeiro morto; trecho até o fim do construtor.
  9. **Experimento `anonymous-methods`** (5, `DeadCodeTest_AnonymousMethodsExperiment_*`): sintaxe experimental (`never.=> 1`);
     fora de escopo (depende do parser).
  **FP (5):** (i) `is/not_class2_test.dart:25:5` e `nnbd/syntax/nullable_type_ambiguous_test.dart:32:31`: `expr::teste_de_tipo`
  promove o ramo falso para `fator(v, t)` com `promover_testado`, que torna o fluxo inalcançável quando o fator é `Never`; o
  analyzer (`tryPromoteForTypeCheck`, `flow_analysis.dart:2540-2551`) **não promove** quando o fator é `Never` (nem quando o
  tipo testado é inválido) — o ramo falso continua alcançável. (ii) `DeadCode__forLoop_noUpdaters.dart` (2 FP): `for (foo(); (i = 42) < 0;) {}`
  com `foo()` `Never` — o oráculo 3.6.2 **não** relata nada (o comentário do teste espera, mas o oráculo é a verdade); conferir
  com a sonda se o inicializador de `for` de expressão não propaga o `Never` (provável: `ForPartsWithExpression` analisa o
  inicializador sem `handleExit`). (iii) `void/void_type_usage_test.dart:1215:40`: `for (true ? x : x; false; true ? x : x) {}`
  — o oráculo relata `x` (1215:19), os atualizadores inteiros (1215:29, `_reportForUpdaters`) e `{}` (1215:43); o DartForge
  abre um trecho a mais dentro dos atualizadores (o condicional do atualizador não pode abrir trecho enquanto o dos
  atualizadores está aberto).
  **pos (4):** `DeadCode__deadBlock_conditionalElse_rec_ba8a97a0.dart` (`true ? p.x : p.y` com `p` registro: oráculo 44/4 =
  `p.y;` — o `PrefixedIdentifier` `p.y` é **reescrito** para `PropertyAccess` (campo de registro), o `flowEnd(elseExpression)`
  recebe o nó novo e não acha o primeiro morto (o antigo), então o trecho só fecha no fim do bloco, aparado na última
  instrução; o mesmo fenômeno que `expr::operando_de_fluxo` já trata para `f()` com `f` local (`chamada_de_local`) — estender
  a `p.campo` de registro (e demais reescritas de `PrefixedIdentifier`); o DartForge hoje erra o fim);
  `ForInOfInvalidType__forIn_never.dart`/`__awaitForIn_never.dart` (iterável `Never`: o primeiro morto é a **variável do laço**
  `var id` (`visitDeclaredIdentifier`), e o `flowEnd(body)` não a contém → o trecho vai até o fim do bloco de fora: 20/77;
  o DartForge começa no corpo); `never_error_test.dart:73:26` (argumentos `()` de `x.neverMethod()`, grupo 1).

#### §3 Referência antes da declaração

##### `referenced_before_declaration` (perda 29: FN 29, FP 0, msg 0, pos 0) — publicado

- **Emissão:** `CompileTimeErrorCode.REFERENCED_BEFORE_DECLARATION` ("Local variable '{0}' can't be referenced before it is
  declared.", correção "Try moving the declaration to before the first use, or renaming the local variable so that it
  doesn't hide a name from an enclosing scope."), construída por `DiagnosticFactory.referencedBeforeDeclaration`
  (`analyzer/lib/src/diagnostic/diagnostic_factory.dart:341-368`, com mensagem de contexto "The declaration of '{0}' is
  here."). Dois chamadores:
  1. `ErrorVerifier._checkForReferenceBeforeDeclaration` (`analyzer/lib/src/generated/error_verifier.dart:5155-5169`), de
     `visitSimpleIdentifier` (`:1410-1420`) e `visitImportPrefixReference` (`:1080-1085`), fase ErrorVerifier (depois da
     resolução): o elemento resolvido do identificador está em `_hiddenElements`.
  2. `NamedTypeResolver` (`analyzer/lib/src/dart/resolver/named_type_resolver.dart:610-621`), fase de resolução de tipos:
     um **nome de tipo** que resolve para uma variável local ou função local (elemento `LocalVariableElement` ou
     `FunctionElement` cujo dono é um executável) — depois das checagens de `as`/`is`/redirecionamento/argumento de tipo
     (`:544-608`, que dão `CAST_TO_NON_TYPE`, `TYPE_TEST_WITH_NON_TYPE`… antes) e fora de `extends`/`implements`/`with`.
- **Condição exata (`_hiddenElements`):**
  ```
  visitBlock / visitSwitchCase / visitSwitchDefault / visitSwitchPatternCase:      (error_verifier.dart:404-409, 1505-1534)
     HiddenElements(outer, statements) = BlockScope.elementsInStatements(statements)  (analyzer/lib/src/dart/resolver/scope.dart:19-42)
        = variáveis de `VariableDeclarationStatement`, variáveis de `PatternVariableDeclarationStatement`
          (`var [v] = …`), funções locais; atravessa um `LabeledStatement`
  declara (deixa de esconder): função local ao visitar a declaração (:950-953, antes do corpo), variável comum ao
     visitar a `VariableDeclaration` (:1617-1625 — depois do inicializador), variáveis de declaração por padrão DEPOIS de
     visitar a instrução inteira (:1300-1306 — `var [v] = [v]` relata)
  visitGuardedPattern (:1057-1062): HiddenElements.forGuardedPattern = TODAS as variáveis do padrão do `case`/`if-case`
     (guardedPattern.variables), escondidas durante a visita do padrão inteiro e NUNCA declaradas dentro dele; a guarda
     `when` é visitada fora (sem esconder)
  relata quando o identificador (leitura OU escrita) resolve para um elemento escondido
  ```
  Como a resolução de nomes do analyzer já põe no escopo do bloco todas as declarações do bloco, um nome usado antes da sua
  declaração local resolve para o local (e não para o de fora) e cai aqui.
- **Posição:** o token do identificador (`nameToken`); no tipo, `node.name2` (só o nome, sem prefixo/argumentos).
- **Mensagem:** `{0}` = lexema do identificador.
- **Supressões e ordem:** referência dentro de `extends`/`implements`/`with` (outra regra); no tipo, os erros de `as`/`is`
  vêm antes (`CAST_TO_NON_TYPE`, `TYPE_TEST_WITH_NON_TYPE`); `when` não é coberto pelo esconder do padrão.
- **No DartForge:** `expr.rs` (`RefNome::Local` com `n.span.start < local.offset`; `RefNome::Adiante` vindo de
  `Corpo::declarar_adiante`) e `crates/types/src/scope.rs:242-256` (outro resolvedor). Os nomes adiante só são
  pré-declarados em `instrucoes::inferir_instrucao` `StmtKind::Block` para `StmtKind::Variables` e `StmtKind::Function`.
  Causas das 29 FN:
  - **declaração por padrão no bloco** (`ReferencedBeforeDeclaration__block_patt_1d1cf29a.dart:3:3`: `v; var [v] = [0];`):
    o pré-escaneamento do bloco não inclui `StmtKind::PatternVariables` (precisa coletar os nomes das variáveis do padrão);
  - **corpo de `case`/`default` de `switch`** (8 amostras `ReferencedBeforeDeclaration__hideInSwit_*.dart`, `v;` antes de
    `void v() {}`/`var v` no mesmo `case`): `instrucoes` `StmtKind::Switch` infere `cases[j].body` sem o pré-escaneamento
    (fazer o mesmo de `StmtKind::Block` sobre `body`, num escopo por grupo);
  - **nome de tipo que é um local declarado adiante** (`ReferencedBeforeDeclaration__type_localVariable.dart:2:3`,
    `__type_localFunction.dart:2:3`, `variable/ref_before_declaration_test.dart:88:3`: `String s = ''; … var String = '';`):
    a resolução da anotação de tipo de uma declaração local (`tipo_de_anotacao`) não consulta os locais/adiante do bloco;
    relatar no nome do tipo quando ele resolve para `Nome::Local`/`Nome::Adiante` (local variável ou função local);
  - **variável de padrão referida dentro do próprio padrão** (16 em `patterns/pattern_variable_constant_scope_test.dart`:
    `case var a && == a`, `case == b && var b`, também em `if-case`, switch expressão e elementos `if-case`): o operando do
    relacional/constante que nomeia uma variável do mesmo padrão (antes OU depois dela) relata; hoje `padroes::tipar` não
    tem a lista das variáveis do padrão em curso — coletar as variáveis do padrão do `case` antes de tipá-lo e, ao
    resolver um identificador dentro do padrão (fora da guarda), relatar se ele é uma delas;
  - `prefix_shadowed_by_local_declaration/PrefixShadowedByLocalDeclaration__local_cdc6ef0b.dart:5:3` (`a.Future? x` com
    `a` local declarado adiante e prefixo de import `a`): o caminho 1 via `visitImportPrefixReference` — o nome do prefixo
    resolve para o local escondido.
  Não há FP hoje; manter a condição estrita (o local tem de estar no mesmo bloco/grupo/padrão e ainda não declarado).

#### §4 Padrões

##### `pattern_type_mismatch_in_irrefutable_context` (perda 22: FN 22)

- **Emissão:** `SharedTypeAnalyzerErrors.patternTypeMismatchInIrrefutableContext` (`analyzer/lib/src/dart/resolver/shared_type_analyzer.dart:174-185`),
  chamado pelo analisador de padrões compartilhado (`_fe_analyzer_shared/lib/src/type_inference/type_analyzer.dart`) durante a
  resolução, só quando `context.irrefutableContext != null` (declaração `var/final p = e`, atribuição por padrão `p = e`,
  `for (var p in e)`; nunca em `case`/`if-case`).
- **Condição exata** (com `M` = tipo casado **promovido**, `flow.getMatchedValueType()`):
  - variável declarada `T x` (`type_analyzer.dart:499-512`): `M` não `dynamic`/inválido e `!(M <: T)` (subtipo, não
    atribuível); variável sem tipo nunca (T = M);
  - variável atribuída `(x) = e` (`:330-341`): idem contra o tipo declarado da variável;
  - curinga com tipo `T _` (`:2078-2089`): `!isAssignableTo(M, T)`;
  - lista (`:845-855`), mapa (`:1140-1150`), objeto (`:1312-1322`), registro (`:1630-1640`): `!isAssignableTo(M, requerido)`
    (requerido = `List<E>`, `Map<K,V>`, o tipo do objeto, o formato do registro com `Object?` nos campos sem tipo:
    `(Object?,)`); avaliado DEPOIS dos subpadrões.
  `isAssignableTo` aceita `dynamic` (e não relata quando `M` é `dynamic`).
- **Posição:** o nó do padrão (`atNode(pattern)`): `int a` em `var (int a) = x`; a lista `[a]`; o objeto `String(length: a)`
  inteiro; em `(a) = x` o `a` (a variável atribuída).
- **Mensagem:** "The matched value of type '{0}' isn't assignable to the required type '{1}'." — `{0}` = `M`, `{1}` =
  requerido, ambos `DartType` (exibição padrão; tipo de função `int Function(int)`, registro `({int foo})`/`(Object?,)`).
- **Supressões e ordem:** em contexto irrefutável, um padrão refutável (`?`, constante, relacional, `||`) relata
  `refutable_pattern_in_irrefutable_context` e o resto do padrão é analisado como refutável (sem este código abaixo dele);
  tipo requerido inválido.
- **No DartForge:** não implementado. Entrar em `padroes::tipar` quando `!cx.padrao_refutavel` (chamado por
  `padroes::declaracao`, `declarar_por_tipo` (for-in), `atribuicao_de_padrao`): nos ramos `Variable{ty}`, `Wildcard{ty}`,
  `List`, `Map`, `Object`, `Record` e na atribuição a variável. Amostras: `PatternTypeMismatchInIrrefutableContext_0f7a306d.dart`
  (`var (int a) = x` com `num`), `_192863be.dart` (`var (a,) = x` com `({int foo})` → requerido `(Object?,)`), `_45088ae5.dart`
  (`(a) = x`).

##### `relational_pattern_operand_type_not_assignable` (perda 18: FN 18)

- **Emissão:** `relationalPatternOperandTypeNotAssignable` (`shared_type_analyzer.dart:196-207`), de
  `TypeAnalyzer.analyzeRelationalPattern` (`type_analyzer.dart:1691-1760`), resolução.
- **Condição exata:** `operator = resolveRelationalPatternOperator(node, M)` (`analyzer/lib/src/generated/resolver.dart:1610-1660`:
  busca `==` (para `==` e `!=`) ou o operador no tipo casado `M` via `TypePropertyResolver`); `parameterType` = tipo do 1º
  parâmetro do operador, feito anulável se é igualdade; o operando é inferido com esse contexto; se `operator != null` e
  `parameterType != null` e `!isAssignableTo(tipoDoOperando, parameterType)` → relata.
- **Posição:** o operando (`pattern.operand`, expressão inteira).
- **Mensagem:** "The constant expression type '{0}' is not assignable to the parameter type '{1}' of the '{2}' operator." —
  `{0}` tipo do operando, `{1}` = `operator.parameterType` **não anulável** (o original, sem o `?` acrescentado), `{2}` lexema
  do operador do padrão (`==`, `!=`, `>`, `>=`, `<`, `<=`).
- **Supressões:** operador não resolvido (outro erro: `undefined_operator` no operador); o operando que não é constante
  (`constant_pattern_with_non_constant_expression`/`non_constant_relational_pattern_expression`, independentes).
- **No DartForge:** `padroes::tipar` `PatternKind::Relational` já busca o membro e infere o operando com o contexto do
  parâmetro, mas não confere. Acrescentar a conferência (`atribuivel(tipo_do_operando, anulável-se-igualdade(param))`).
  Amostras `RelationalPatternArgumentTypeNotAssigna_00a67d12.dart` (`case > 0` com `A.operator >(A)`), `extension_type/relational_pattern_error_test.dart:29:19`
  (6, operando de tipo de extensão `E` contra `int`).

##### `pattern_variable_assignment_inside_guard` (perda 15: FN 15)

- **Emissão:** `ScopeResolverVisitor.visitSimpleIdentifier` (`analyzer/lib/src/generated/resolver.dart:5151-5199`, relato em
  `:5186-5193`), fase de resolução de escopo (antes da inferência).
- **Condição exata:** o identificador está em contexto de escrita (`inSetterContext`: alvo de `=`, `op=`, `++`/`--`
  prefixo/posfixo) e resolve para um `PatternVariableElementImpl` com `isVisitingWhenClause` — ligado em
  `visitGuardedPattern` (`:4988-5005`) para TODAS as variáveis do padrão guardado (`node.variables`, inclusive as unidas de
  `||`) só durante a visita da cláusula `when` (inclui closures e `if-case` aninhados dentro da guarda).
- **Posição:** o identificador.
- **Mensagem:** "Pattern variables can't be assigned inside the guard of the enclosing guarded pattern." (sem argumentos).
- **No DartForge:** não implementado. Em `padroes::caso`, durante `expr::condicao_verificada(guarda)`, marcar no `Corpo` o
  conjunto de `LocalId` das variáveis do padrão (`Corpo::variaveis_em_guarda`, pilha); em `expr::atribuicao`/`unario`
  (inc/dec) ao escrever num `Local` desse conjunto, relatar no span do identificador. Amostras
  `PatternVariableAssignmentInsideGuard__e_22ae0963.dart` (`when (a = 1) > 0`), `__c_3eec891b.dart` (`a = 0;` dentro de
  closure na guarda), `patterns/guard_error_test.dart:8:22`.

##### `unnecessary_cast_pattern` (perda 13: FN 13)

- **Emissão:** `matchedTypeIsSubtypeOfRequired` (`shared_type_analyzer.dart:140-150`), de `analyzeCastPattern`
  (`type_analyzer.dart:370-395`).
- **Condição exata:** `isSubtypeOf(M, T)` e `T` não inválido, com `M` o tipo casado promovido (`p as T`). Em contexto
  refutável e irrefutável.
- **Posição:** o token `as`.
- **Mensagem:** "Unnecessary cast pattern." (WarningCode).
- **No DartForge:** não implementado; `padroes::tipar` `PatternKind::Cast` (precisa do offset do `as`: entre o fim do
  subpadrão e o início do tipo). Amostras `UnnecessaryCastPattern__matchedIsSameAsRequired.dart`, `__matchedIsSubtyp_edba307c.dart`,
  `CastFromNullAlwaysFails__castPattern_Nu_67892450.dart` (`(m as int?) = n` em atribuição), `patterns/exhaustiveness/nullable_cast_error_test.dart`
  (5, com `FutureOr`/typedef `Nullable<T> = T?` — `M` é o tipo do subpadrão objeto anterior via `&&`/caso).

##### `missing_variable_pattern` (perda 12: FN 12)

- **Emissão:** `_VariableBinderErrors.logicalOrPatternBranchMissingVariable` (`analyzer/lib/src/dart/resolver/resolution_visitor.dart:1992-2004`),
  chamado por `VariableBinder` (`_fe_analyzer_shared/lib/src/type_inference/variable_bindings.dart:100-140`), na
  `ResolutionVisitor` (antes da inferência).
- **Condição exata:** num `p1 || p2`, cada variável declarada só em `p1` relata no `p2`; cada uma só em `p2` relata no `p1`
  (lados = conjuntos de nomes de variáveis declaradas, já unidos recursivamente: `a || b || c` é `(a || b) || c`).
- **Posição:** o operando do lado onde falta (o nó inteiro do padrão: `2` em `final int a || 2`; `1` em `1 || final a`).
- **Mensagem:** "Variable pattern '{0}' is missing in this branch of the logical-or pattern." — `{0}` nome.
- **No DartForge:** não implementado; `padroes::tipar` `PatternKind::Or` (coletar nomes declarados de cada lado).
  Amostras `MissingVariablePattern__ifCase_logicalOr2_left.dart:2:29`, `…logicalOr3_1.dart` (dois relatos: `2` e `3`).

##### `unnecessary_null_check_pattern` / `unnecessary_null_assert_pattern` (perda 12 / 7: FN 12 / 7)

- **Emissão:** `matchedTypeIsStrictlyNonNullable` (`shared_type_analyzer.dart:120-138`), de `analyzeNullCheckOrAssertPattern`
  (`type_analyzer.dart:1215-1245`).
- **Condição exata:** `flow.nullCheckOrAssertPattern_begin` devolve `true` ⇔ o tipo casado `M` é **não anulável** pela
  classificação do fluxo (`_nullCheckPattern` devolve `null`, `flow_analysis.dart:5998-6010`; `InvalidType` conta como não
  anulável — por isso `getValue() case final x?` com retorno de classe indefinida relata). E: para `?`, só fora de contexto
  irrefutável (no irrefutável é `refutable_pattern_in_irrefutable_context` e nada mais); para `!`, também em contexto
  irrefutável (`(x!) = 2`).
- **Posição:** o token `?`/`!` do padrão.
- **Mensagem:** "The null-check pattern will have no effect because the matched type isn't nullable." /
  "The null-assert pattern will have no effect because the matched type isn't nullable." (StaticWarningCode).
- **No DartForge:** não implementado; `padroes::tipar` `PatternKind::NullCheck`/`NullAssert` (token = último caractere do
  span do padrão). Usar o tipo casado promovido (Apêndice A). Amostras `UnnecessaryNullCheckPattern__interfaceT_1800d04e.dart`,
  `__typeParame_9799ccca.dart` (`T extends Object`), `__invalidTyp_57995ed9.dart` (tipo inválido),
  `patterns/relational_pattern_expression_precedence_error_test.dart:52:22` (`== true ? 1 : 2` é `(== true)?` seguido de erro
  de sintaxe — depende do parser produzir o padrão `?`), `UnnecessaryNullAssertPattern__interface_e948cb68.dart`,
  `AssignmentToPrimaryConstructorParameter_fcdbb29b.dart:2:13` (`(x!) = 2`).

##### `invalid_pattern_variable_in_shared_case_scope` (perda 11: FN 11)

- **Emissão:** `ResolverVisitor.finishJoinedPatternVariable` (`analyzer/lib/src/generated/resolver.dart:878-915`): códigos
  `PATTERN_VARIABLE_SHARED_CASE_SCOPE_NOT_ALL_CASES` ("The variable '{0}' is available in some, but not all cases that share
  this body."), `…_HAS_LABEL` ("…is not available because there is a label or 'default' case."),
  `…_DIFFERENT_FINALITY_OR_TYPE` ("…doesn't have the same type and/or finality in all cases that share this body."), todos com
  nome compartilhado `INVALID_PATTERN_VARIABLE_IN_SHARED_CASE_SCOPE` (`analyzer/messages.yaml:13443`, `:13597`, `:13605`).
- **Condição exata:** para `switch` instrução com vários `case` dividindo um corpo (`location == sharedCaseScope`): a
  variável de junção com nome `x` tem inconsistência: falta em algum `case` do grupo (ou o grupo tem `default`) →
  NOT_ALL_CASES; há rótulo → HAS_LABEL; tipos (exatos) ou `final` diferentes → DIFFERENT_FINALITY_OR_TYPE. Relata em **cada
  referência** à variável no corpo (`variable.references`, coletadas em `visitSimpleIdentifier` `resolver.dart:5196-5198`).
- **Posição:** cada identificador que usa a variável no corpo (leitura ou escrita).
- **Mensagem:** `{0}` = nome.
- **No DartForge:** não implementado. Em `instrucoes` `StmtKind::Switch` o bloco "Casos que dividem o corpo" já declara as
  variáveis de junção; precisa guardar, por nome, a consistência (presença em todos os casos, tipo, `final`, rótulo,
  `default`) e relatar em cada uso no corpo. Amostras `unused_local_variable/UnusedLocalVariable__switchStatement_sh_20f8b113.dart:7:7`
  (`case 0: case [var a]: a = 1;`), `patterns/shared_case_variable_error_test.dart:12:13` (tipos diferentes).

##### `unnecessary_wildcard_pattern` (perda 10: FN 10)

- **Emissão:** `unnecessaryWildcardPattern` (`shared_type_analyzer.dart:241-253`), de `analyzeWildcardPattern`
  (`type_analyzer.dart:2068-2110`).
- **Condição exata:** o curinga é operando direto (através de parênteses) de um `&&` (`withUnnecessaryWildcardKind(logicalAndPatternOperand)`,
  `analyzeLogicalAndPattern`, `type_analyzer.dart:912-935`; anulado em subpadrões de lista/mapa/registro/objeto/`?`/`!`/`as`/`||`) e **sempre casa**:
  sem tipo, ou com tipo `T` e `promoteForPattern(M, T)` devolve `covers` (`M <: T`).
- **Posição:** o padrão curinga (`_` ou `int _`).
- **Mensagem:** "Unnecessary wildcard pattern." (WarningCode).
- **No DartForge:** não implementado; `padroes::tipar` `PatternKind::And` (marcar os operandos). Amostras
  `UnnecessaryWildcardPattern__ifCase_notR_b90d4636.dart` (`_ && 0`), `…_9a6928b7.dart` (`int _ && > 0`), `…_09d48b9f.dart` (`(_) && 0`).

##### `equal_keys_in_map_pattern` (perda 8: FN 8)

- **Emissão:** `ConstantVerifier.visitMapPattern` (`analyzer/lib/src/dart/constant/constant_verifier.dart:311-358`) →
  `DiagnosticFactory.equalKeysInMapPattern` (`analyzer/lib/src/diagnostic/diagnostic_factory.dart:247-262`), fase do
  verificador de constantes.
- **Condição exata:** cada chave avaliada como constante (sem erro); duas chaves são iguais se `identical` (`isIdentical2`) ou,
  com igualdade primitiva nas duas, `==` (`:310-325`). Relata a chave repetida (a posterior), com contexto na original.
- **Posição:** a chave duplicada (expressão).
- **Mensagem:** "Two keys in a map pattern can't be equal." (sem argumentos).
- **No DartForge:** não implementado (o equivalente `equal_keys_in_const_map` existe para literais — reaproveitar a igualdade
  de constantes). Entrar em `padroes::tipar` `PatternKind::Map` ou no verificador de constantes (`crates/types/src/constantes`).
  Amostras `EqualKeysInMapPattern__identical_int.dart` (`{0: 1, 0: 2}`), `__identical_double.dart`, `__identical_type.dart`
  (`int`/`int`), `__identical_type_e_9e60da74.dart` (`int` e typedef `E = int`), `__recordType_primi_38d24c91.dart` (`()`).

##### `pattern_never_matches_value_type` (perda 8: FN 5, FP 2, msg 1)

- **Emissão:** `ResolverVisitor.checkPatternNeverMatchesValueType` (`analyzer/lib/src/generated/resolver.dart:614-640`),
  chamado na resolução de padrão objeto, variável declarada, curinga, cast, lista, mapa e registro (só fora de contexto
  irrefutável).
- **Condição exata:** `!typeSystem.canBeSubtypeOf(M, requerido)` (`analyzer/lib/src/dart/element/type_system.dart:147-310`)
  com `M` promovido.
- **Posição:** o tipo escrito para cast, variável declarada, objeto e curinga; senão o padrão inteiro (lista, mapa,
  registro).
- **Mensagem:** "The matched value type '{0}' can never match the required type '{1}'." (DartType).
- **No DartForge:** `padroes::nunca_casa` + `pode_ser_subtipo` (já implementado). Divergências: FN de mapa
  (`PatternNeverMatchesValueType__refutable_81d6a7ca.dart:2:14`, o ramo `Map` de `tipar` não chama `nunca_casa`), de registro
  (`…_df47f7f4.dart:3:10`, `(int f1, int f2)` contra `(int,)`: o ramo `Record` não chama), `null_type_test.dart:24:5` e
  `switch_trivial_exhaustiveness_error_test.dart:264:19`/`491:10` (tipo casado promovido: `int _ && String _` — o `String _`
  vê `M = int`; `Null _` depois de `int`… — Apêndice A); FP `PatternNeverMatchesValueType__matchedEn_9107b52c.dart`
  (`x: A` enum com `v2<int>()` e `A<T> implements R<T>`, padrão `R<num> _`: `canBeSubtypeOf` de enum testa o **tipo de cada
  constante** (`A<String>`, `A<int>`), não `A<dynamic>`, `type_system.dart:219-228`).

##### `pattern_assignment_not_local_variable` (perda 7: FN 7)

- **Emissão:** `ResolutionVisitor.visitAssignedVariablePattern` (`analyzer/lib/src/dart/resolver/resolution_visitor.dart:190-210`).
- **Condição exata:** num padrão de atribuição, o nome resolve (escopo léxico) para algo que não é variável local nem
  parâmetro (campo, getter, variável de topo, classe, tipo, função, parâmetro de tipo); se não resolve: `undefined_identifier`.
- **Posição:** o nome.
- **Mensagem:** "Only local variables can be assigned in pattern assignments."
- **No DartForge:** não implementado; `padroes::tipar` com `atribuicao` (ramo `Variable`): hoje só procura `Nome::Local`. Amostras
  `PatternAssignmentNotLocalVariable__class.dart` (`(int) = 0`), `__dynamic.dart`, `__function.dart`, `…__clas_1ef17e2e.dart`
  (campo `x`), `…__clas_41aeb1f2.dart` (`T`).

##### `duplicate_pattern_field` (perda 6: FN 6)

- **Emissão:** `duplicateRecordPatternField` (`shared_type_analyzer.dart:63-81`) → `DiagnosticFactory.duplicatePatternField`
  (`diagnostic_factory.dart:151-178`), de `_reportDuplicateRecordPatternFields` (`type_analyzer.dart:2571-2590`) em padrões
  registro e objeto.
- **Condição exata:** dois campos com o mesmo nome (explícito `foo:` ou implícito `:var foo`).
- **Posição:** o campo duplicado (nome; para `:var foo`, o nó do campo).
- **Mensagem:** "The field '{0}' is already matched in this pattern."
- **No DartForge:** não implementado; ramos `Record`/`Object` de `padroes::tipar`. Amostras `DuplicatePatternField__objectPattern.dart`,
  `__recordPattern_dy_6adabf74.dart` (`(:var foo, foo: 1)`).

##### `refutable_pattern_in_irrefutable_context` (perda 4: FN 4)

- **Emissão:** `refutablePatternInIrrefutableContext` (`shared_type_analyzer.dart:187-194`), em `analyzeConstantPattern`
  (`type_analyzer.dart:426-433`), `analyzeLogicalOrPattern` (`:964-971`), `analyzeNullCheckOrAssertPattern` (só `?`, `:1229-1235`),
  `analyzeRelationalPattern` (`:1700-1705`), quando `irrefutableContext != null`; depois o contexto vira refutável (sem
  cascata).
- **Posição:** o padrão refutável.
- **Mensagem:** "Refutable patterns can't be used in an irrefutable context."
- **No DartForge:** não implementado; `padroes::tipar` em contexto de declaração/atribuição. Amostras `RefutablePatternInIrrefutableContext__d_850ea7cf.dart`
  (`var (0) = 0`), `__d_98ce1c3c.dart` (`var (_?) = x`), `__d_b8d27d9f.dart` (`var (_ || _) = 0`, que também tem `dead_code`
  no `|| _`), `__d_c219e5ec.dart` (`var (> 0) = 0`).

##### `duplicate_pattern_assignment_variable` (perda 4: FN 4)

- **Emissão:** `duplicateAssignmentPatternVariable` (`shared_type_analyzer.dart:47-61`) → `diagnostic_factory.dart:22-45`,
  de `analyzeAssignedVariablePattern` (`type_analyzer.dart:314-322`).
- **Condição exata:** a mesma variável atribuída duas vezes no mesmo padrão de atribuição (`(a, a) = …`).
- **Posição:** a segunda ocorrência (o padrão de variável atribuída).
- **Mensagem:** "The variable '{0}' is already assigned in this pattern."
- **No DartForge:** não implementado; `padroes::atribuicao_de_padrao` (conjunto de `LocalId` já atribuídos).

##### `duplicate_variable_pattern` (perda 3: FN 3)

- **Emissão:** `_VariableBinderErrors.duplicateVariablePattern` (`resolution_visitor.dart:1974-1989`) →
  `DiagnosticFactory.duplicateDefinitionForNodes`.
- **Condição exata:** o mesmo nome declarado duas vezes no mesmo padrão fora de lados opostos de `||` (`var (a, a)`, `[a, a]`).
- **Posição:** o nome da segunda declaração.
- **Mensagem:** "The variable '{0}' is already defined in this pattern."
- **No DartForge:** não implementado; coletar nomes em `padroes::tipar` (por padrão de topo; `||` junta).

##### `expected_two_map_pattern_type_arguments` / `expected_one_list_pattern_type_arguments` (perda 2 / 1)

- **Emissão:** `ResolverVisitor.resolveMapPattern…` (`analyzer/lib/src/generated/resolver.dart:1520-1538`) e
  `ListPatternResolver` (`analyzer/lib/src/dart/resolver/list_pattern_resolver.dart:20-33`).
- **Condição:** lista de argumentos de tipo presente com número ≠ 2 (mapa) / ≠ 1 (lista).
- **Posição:** a lista de argumentos de tipo `<…>`.
- **Mensagem:** "Map patterns require two type arguments or none, but {0} found." / "List patterns require one type argument
  or none, but {0} found." — `{0}` = número (int).
- **No DartForge:** não implementado; ramos `Map`/`List` de `padroes::tipar` (hoje usa `type_args.first()` / exige `len()==2`).

##### `relational_pattern_operator_return_type_not_assignable_to_bool` (perda 2: FN 2)

- **Emissão:** `shared_type_analyzer.dart:209-219`, de `analyzeRelationalPattern` (`type_analyzer.dart:1741-1748`).
- **Condição:** operador resolvido cujo tipo de retorno não é atribuível a `bool`.
- **Posição:** o token do operador. **Mensagem:** "The return type of operators used in relational patterns must be
  assignable to 'bool'."
- **No DartForge:** não implementado; `padroes::tipar` `Relational` (o retorno de `m.tipo`).

##### `rest_element_in_map_pattern` / `duplicate_rest_element_in_pattern` / `empty_map_pattern` (perda 2 / 1 / 1)

- **Emissão:** `restPatternInMap` (`shared_type_analyzer.dart:221-230`, de `analyzeMapPattern` `type_analyzer.dart:1100-1110`,
  na posição do elemento `...`); `duplicateRestPattern` (`shared_type_analyzer.dart:83-96` → `diagnostic_factory.dart:181-200`,
  de `analyzeListPattern` `type_analyzer.dart:816-825`, no segundo `...`); `emptyMapPattern` (`shared_type_analyzer.dart:98-106`,
  `type_analyzer.dart:1152-1155`, no padrão `{}` inteiro, depois dos elementos).
- **Mensagens:** "A map pattern can't contain a rest pattern." / "At most one rest element is allowed in a list or map
  pattern." / "A map pattern must have at least one entry."
- **No DartForge:** não implementado; ramos `Map`/`List` de `padroes::tipar` (o parser precisa preservar o `...` em mapa).

##### `non_exhaustive_switch_expression` (perda 1: msg 1)

- **Emissão:** `ConstantVerifier._validateSwitchExhaustiveness` (`analyzer/lib/src/dart/constant/constant_verifier.dart:940-985`,
  código em `:969`). **Mensagem:** "The type '{0}' is not exhaustively matched by the switch cases since it doesn't match
  '{1}'." — `{1}` é o espaço testemunha formatado pelo motor de exaustividade.
- **No DartForge:** `crates/types/src/constantes/verificador.rs` + `exaustividade.rs` (do coordenador). A 1 mensagem errada é a
  formatação da testemunha; fora desta família.

##### `case_expression_type_is_not_switch_expression_subtype` (perda 4: FN 4)

- **Emissão:** `caseExpressionTypeMismatch` (`shared_type_analyzer.dart:32-45`), de `analyzeConstantPattern`
  (`type_analyzer.dart:436-457`) **só sem padrões** (`!options.patternsEnabled`, linguagem < 3.0) e num `switch`.
- **Condição:** com null safety: `!(tipoDoCase <: tipoDoEscrutínio)`; sem: `!isAssignableTo`.
- **Posição:** a expressão do `case`. **Mensagem:** "The switch case expression type '{0}' must be a subtype of the switch
  expression type '{1}'." (DartType).
- **No DartForge:** não implementado; `instrucoes` `StmtKind::Switch` quando a biblioteca é `@dart < 3.0`.

##### `break_label_on_switch_member` (perda 4: FN 4)

- **Emissão:** `ErrorVerifier.visitBreakStatement` (`analyzer/lib/src/generated/error_verifier.dart:424-436`).
- **Condição:** `break L;` cujo rótulo `L` está num membro de `switch` (`case`/`default`, `LabelElementImpl.isOnSwitchMember`).
- **Posição:** o rótulo no `break`. **Mensagem:** "A break label resolves to the 'case' or 'default' statement."
- **No DartForge:** não implementado; `instrucoes::alvo_de_salto` (os rótulos de `SwitchCase.labels` não são alvos de
  `break`; hoje não são registrados como tal).

#### §5 Fluxo e atribuição definitiva

##### `invalid_use_of_null_value` (perda 12: FN 12)

- **Emissão:** `NullableDereferenceVerifier.report` (`analyzer/lib/src/error/nullable_dereference_verifier.dart:39-64`): é a
  troca de qualquer `UNCHECKED_*` (§ `unchecked_use_of_nullable_value`) quando o tipo do receptor/expressão é **exatamente**
  `Null` (`receiverType == typeProvider.nullType`, `:43-46`); sem argumentos.
- **Mensagem:** "An expression whose value is always 'null' can't be dereferenced." (correção "Try changing the type of the
  expression.").
- **Posição:** a mesma do `UNCHECKED_*` que ela substitui (expressão inteira para iterável/espalhamento; token do operador
  para `++`/`--`/`-`).
- **No DartForge:** `BodyInferrer::aviso_de_nulo` (`crates/types/src/inferencia/mod.rs:528-541`) já faz a troca; as 12 FN são
  dos mecanismos que faltam no `unchecked_use_of_nullable_value`: espalhamento `[...null]`, `[...a]` com `Null a`, em lista,
  conjunto e mapa (6, `not_null_aware_null_spread/*` — sai junto com `not_null_aware_null_spread`), `for (var y in x)` com
  `x: Null` (`InvalidUseOfNullValue__forLoop.dart:3:17`), e prefixo/posfixo `x++`, `x--`, `++x`, `--x`, `-x` com `x: Null`
  (`undefined_operator/UndefinedOperator__{postfixInc,postfixDec,prefixInc,prefixDec,unaryMinus}_null.dart`, no token
  do operador). Conferido no oráculo vivo (`c1/a.dart`: `for (final x in n)` e `{...null}`).

##### `assignment_to_final_local` (perda 11: FN 11) — publicado

- **Emissão:** `AssignmentExpressionShared.checkFinalAlreadyAssigned` (`analyzer/lib/src/dart/resolver/assignment_expression_resolver.dart:355-385`)
  — chamado por atribuição (simples, composta, `??=`), prefixo/posfixo `++`/`--` e identificador de `for-in`
  (`for_resolver.dart:121-124`, `isForEachIdentifier: true`); e `resolveAssignedVariablePattern`
  (`analyzer/lib/src/generated/resolver.dart:1365-1393`) para atribuição por padrão.
- **Condição exata:** alvo `SimpleIdentifier` cujo elemento é `PromotableElement` (variável local **ou parâmetro**), `final`,
  não `late`, e (`isForEachIdentifier` **ou** não definitivamente não atribuído). Parâmetros `final` incluem os formais de
  campo `this.x` e formais de super `super.a` (são implicitamente `final`), e o parâmetro já está atribuído → toda escrita
  relata (inclusive dentro de closure no inicializador).
- **Posição:** o identificador alvo. **Mensagem:** "The final variable '{0}' can only be set once." — `{0}` nome.
- **Supressões:** `late final` vai para `late_final_local_already_assigned`.
- **No DartForge:** `expr::check_final_local` (`crates/types/src/inferencia/expr.rs`), usado na atribuição e no inc/dec.
  FN: (a) `for (i in …)` com `i` `final` (`AssignmentToFinalLocal__localVariable_forEach.dart:3:8`, `__inForEach.dart`,
  `null_safety_read_write/ReadWrite__final_definitelyAssigned_wri_2a7a09cf.dart`) — `instrucoes::cabecalho_for_in`
  `ForInTarget::Expression` escreve sem checar (e a regra é "sempre", não "se atribuída"); (b) parâmetro formal de campo
  `this.x`/super `super.a` (`AssignmentToFinalLocal__parameter_fieldFormal.dart:5:5`, `__parameter_superFormal.dart:6:26`,
  `initializing_formal/final_test.dart:13:9`) — o `Local` do parâmetro é declarado com `final_: p.final_` (o escrito), devia
  ser `true` para `this.`/`super.`; (c) `??=` sobre `final` possivelmente atribuída (`if_null/assignment_behavior_test.dart:187:5`,
  `nnbd/definite_assignment/read_error_test.dart:890:5`, `write_error_test.dart:550:5`) — o caminho de `AssignOp::IfNull`
  não passa por `check_final_local`.

##### `late_final_local_already_assigned` (perda 2: FN 1, FP 1)

- **Emissão:** mesmos dois pontos (`assignment_expression_resolver.dart:366-374`, `resolver.dart:1375-1383`).
- **Condição:** `late final` e (`isForEachIdentifier` ou **definitivamente atribuída**); padrão: `flow.isAssigned`.
- **Posição:** o identificador. **Mensagem:** "The late final local variable is already assigned." (sem argumentos).
- **No DartForge:** `expr::check_final_local`. FN `LateFinalLocalAlreadyAssigned__localVar_db2b2bf2.dart:3:8` (`for (i in …)`
  com `late final i` — regra "sempre" do for-each). FP `nnbd/definite_assignment/definite_assignment_error_test.dart:465:5`:
  depois de `y ??= 3;` com `y` só **potencialmente** atribuída, a escrita do `??=` acontece só no ramo nulo — o fluxo depois
  do `??=` é a junção (não atribuída ∪ atribuída) = não definitivamente atribuída; o DartForge marca atribuída
  incondicionalmente na escrita de `??=` (modelar `x ??= e` como `x == null ? x = e : x`: `ifNull_rightBegin`/`ifNull_end`).

##### `not_assigned_potentially_non_nullable_local_variable` (perda 8: FN 8) — publicado

- **Emissão:** `ResolverVisitor.checkReadOfNotAssignedLocalVariable` (`analyzer/lib/src/generated/resolver.dart:643-692`,
  código em `:683-688`), na resolução de cada `SimpleIdentifier` lido.
- **Condição exata:** em contexto de leitura (`inGetterContext`), elemento `VariableElement` (local ou parâmetro) não `late`,
  **não definitivamente atribuído** (pelo estado de fluxo corrente — **também em código inalcançável**: o estado de
  atribuição sobrevive ao `setUnreachable`), não `final` (senão é `read_potentially_unassigned_final`), e
  `isPotentiallyNonNullable(tipo)` = **não anulável** (`T` sem limite, `T extends Object?`, `FutureOr<T>`… são
  potencialmente não anuláveis: só `dynamic`, `void`, `Null`, `T?` e parâmetro de tipo com limite anulável **e** sufixo `?`
  ficam de fora).
- **Posição:** o identificador. **Mensagem:** "The non-nullable local variable '{0}' must be assigned before it can be used."
- **No DartForge:** existe (publicado). FN: (a) parâmetro de tipo sem limite (`NotInitializedPotentiallyNonNullableLoc_fff714db.dart:3:3`,
  `ReadWrite__potentiallyNonNullable_defin_1c74dbe1.dart:3:3`, `…_neither_read.dart:4:3`: `T v; v;`) — a condição usa
  "não anulável" estrito (`e_nao_anulavel`) em vez de `!isNullable` (potencialmente não anulável); (b) leitura em código
  inalcançável (`NotInitializedPotentiallyNonNullableLoc_50076a65.dart:7:3`, depois de `while (true) {}`;
  `nnbd/definite_assignment/definite_assignment_error_test.dart:50:9` depois de `return;`): o DartForge não relata quando
  `!fluxo.alcancavel` (o modelo deve manter `atribuida`/`nao_atribuida` ao tornar inalcançável e consultá-los mesmo assim).
  Cuidado: no analyzer `join` de um ramo inalcançável devolve o outro (`juntar` já faz).

##### `read_potentially_unassigned_final` (perda 2: FN 2) — publicado

- **Emissão:** o mesmo `checkReadOfNotAssignedLocalVariable` (`resolver.dart:671-679`).
- **Condição:** leitura de local `final` não `late` não definitivamente atribuído (qualquer tipo).
- **Posição/Mensagem:** o identificador; "The final variable '{0}' can't be read because it's potentially unassigned at this
  point."
- **No DartForge:** FN `variable/ref_before_declaration_test.dart:43:11` e `:96:13`: o identificador refere-se a um local
  `final` **declarado adiante** no mesmo bloco (`use(x); … final x = …;`, `math.pi` com `final math` adiante) — o analyzer
  relata **as duas coisas**: `referenced_before_declaration` e `read_potentially_unassigned_final` (o local existe e não está
  atribuído). Hoje o caminho `RefNome::Adiante` (`expr.rs`) só relata o primeiro. Regra: se o local adiante é `final` (não
  `const`, não `late`) → também este código no mesmo span; se não é `final` e o tipo **escrito** é potencialmente não
  anulável → `not_assigned_potentially_non_nullable_local_variable` (conferir com a sonda; com `var x = …` o tipo ainda não
  foi inferido).

##### `definitely_unassigned_late_local_variable` (perda 3: FN 1, pos 2)

- **Emissão:** `checkReadOfNotAssignedLocalVariable` (`resolver.dart:660-668`).
- **Condição:** leitura de local `late` **definitivamente não atribuído** (também em código inalcançável).
- **Posição/Mensagem:** o identificador; "The late local variable '{0}' is definitely unassigned at this point."
- **No DartForge:** existe. FN `nnbd/definite_assignment/definite_assignment_error_test.dart:315:14` (`for (; ; y) {}`: o
  atualizador é lido com o estado do fim do corpo (inalcançável → herda "não atribuída"); o DartForge não infere/relata
  atualizadores inalcançáveis — `instrucoes` `StmtKind::For` marca `trecho_morto` e/ou pula). As 2 `posição` (54:9, 252:11)
  são o mesmo fenômeno de código inalcançável (relato numa linha, falta na outra): ler `use(y)` depois de `return;`.

##### `body_might_complete_normally` / `body_might_complete_normally_nullable` / `body_might_complete_normally_catch_error` (perda 8 / 4 / 3: só FN)

- **Emissão:** `ResolverVisitor.checkForBodyMayCompleteNormally` (`analyzer/lib/src/generated/resolver.dart:522-603`), chamado
  ao fim de declaração de função (`:2800-2803`, `errorNode: node.name`), método (`:3194-3197`, `node.name`), construtor
  **factory** (`:2333-2337`, `errorNode: node` → `atConstructorDeclaration`, `analyzer/lib/src/error/listener.dart:72-95`:
  do tipo de retorno ao fim do nome, `C.named`) e expressão de função (`function_expression_resolver.dart:63-66`,
  `errorNode: body` → `{`). `_CATCH_ERROR` em `_checkForFutureCatchErrorOnError` (`resolver.dart:3971-4006`).
- **Condição exata:** fim do corpo **alcançável**; o corpo tem contexto de retorno (`bodyContext.contextType`); corpo de
  bloco, não gerador; se `async`, o tipo imposto aceita `Future<Never>`; então `BODY_MIGHT_COMPLETE_NORMALLY` se o tipo de
  retorno (de contexto: `flatten` para async) é **potencialmente não anulável**; senão, se a base `futureOrBase` não é
  `dynamic`/inválido/`_`/`void`/`Null` → `BODY_MIGHT_COMPLETE_NORMALLY_NULLABLE` (WarningCode). Sem contexto de retorno e o
  corpo é o `onError` de `Future<T>.catchError`: `_CATCH_ERROR` com `futureOrBase(FutureOr<T>)` se não é
  `dynamic`/`_`/`void`/`Null`.
- **Mensagens:** "The body might complete normally, causing 'null' to be returned, but the return type, '{0}', is a
  potentially non-nullable type." / "This function has a nullable return type of '{0}', but ends without returning a value."
  / "This 'onError' handler must return a value assignable to '{0}', but ends without returning a value." (`{0}` DartType).
- **Posição:** nome da função/método/getter (para `get b, c` recuperado, o nome `b`); construtor factory: do tipo de retorno
  ao fim do nome (`C`, `E.named`); expressão de função: o `{`.
- **Supressões:** retorno inválido (`async` com retorno não-`Future`); corpo `=>`; gerador; fim inalcançável; `external`
  NÃO suprime (amostras `ConstructorBody__class_secondaryConstru_5085df45.dart`: `external factory C() {}` relata).
- **No DartForge:** não implementado (nenhum emissor). Entrar em `funcoes::corpo_de_funcao`/`funcoes::expressao_de_funcao`
  (já sabem `cx.fluxo.alcancavel` no fim e o retorno declarado/de contexto). Amostras: `constructor/unresolved_in_factory_test.dart:8:11`,
  `type_variable/static_context_test.dart:8:14`, `BodyMightCompleteNormallyNullable__func_6872f662.dart` (`int? f() {}` → no
  nome `f`), `…_a6a31a8d.dart` (`FutureOr<int?> f(Future f) async {}`), `getter/syntax_get_set_syntax_test.dart:113:13`,
  `BodyMayCompleteNormallyCatchError__noRe_67f177f2.dart:2:29` (`future.catchError((e, st) {})` → `{`).

##### `switch_case_completes_normally` (perda 10: FN 10)

- **Emissão:** `switchCaseCompletesNormally` (`analyzer/lib/src/dart/resolver/shared_type_analyzer.dart:232-239`), de
  `analyzeSwitchStatement` (`_fe_analyzer_shared/lib/src/type_inference/type_analyzer.dart:2005-2012`).
- **Condição exata:** null safety ligado **e padrões desligados** (biblioteca com `@dart` < 3.0); o grupo de `case` não é o
  último (`caseIndex < numCases - 1`); o fim do corpo do grupo é alcançável a partir do seu começo
  (`!flow.switchStatement_afterCase()` = `locallyReachable`). Grupo vazio que cai no seguinte (`case 1: case 2:`) é o mesmo
  grupo (não relata). Em 3.0+ não existe (o `case` pode completar).
- **Posição:** a palavra-chave `case`/`default` do **primeiro** membro do grupo (`node.members[caseIndex].keyword` — o
  `caseIndex` é o índice do grupo; conferir com as amostras: `case 9:` 203:9).
- **Mensagem:** "The 'case' shouldn't complete normally." (sem argumentos).
- **No DartForge:** não implementado; `instrucoes` `StmtKind::Switch` (laço dos grupos, `cx.fluxo.alcancavel` depois do corpo)
  quando a versão da linguagem da biblioteca < 3.0. Amostras `switch/case_fallthrough_legacy_error_test.dart:203:9` (7),
  `switch/fallthru_legacy_test.dart:15:5`, `switch/empty_block_case_test.dart:20:7` (`{}` vazio completa),
  `switch/fallthru_test.dart:16:5` — conferir a versão (`@dart=2.19`).

##### `dead_code_on_catch_subtype` / `dead_code_catch_following_catch` (perda 3 / 1: só FN)

- **Emissão:** `NullSafetyDeadCodeVerifier.verifyCatchClause` → `_CatchClausesVerifier.nextCatchClause`
  (`analyzer/lib/src/error/dead_code_verifier.dart:373-378`, `:509-544`), chamado de `ResolverVisitor.visitTryStatement`
  (`analyzer/lib/src/generated/resolver.dart:3828-3847`).
- **Condição exata:** percorrendo os `catch` em ordem: um `catch` sem `on` ou `on Object` que não é o último →
  `DEAD_CODE_CATCH_FOLLOWING_CATCH` do `catch` seguinte até o fim do último, e para; senão, se o tipo `on T` é subtipo de um
  `on S` anterior → `DEAD_CODE_ON_CATCH_SUBTYPE` [T, S] deste `catch` até o fim do último, e para. Tipo não resolvido é
  `InvalidType` e conta (`on Unavailable` depois de `on String` → `'InvalidType' is a subtype of 'String'`).
- **Posição:** do início do `catch` problemático (o `on`/`catch`) ao fim do último `catch` do `try`.
- **Mensagens:** "Dead code: This on-catch block won't be executed because '{0}' is a subtype of '{1}' and hence will have been
  caught already." / "Dead code: Catch clauses after a 'catch (e)' or an 'on Object catch (e)' are never reached." (correções
  "Try reordering the catch clauses so that this block can be reached, or removing the unreachable catch clause." / "Try
  reordering the catch clauses so that they can be reached, or removing the unreachable catch clauses.").
- **No DartForge:** não implementado; `instrucoes` `StmtKind::Try` (os tipos de `c.on_type` já são resolvidos) e o intervalo
  vira "catch morto" (nenhum `dead_code` dentro). Amostras `exception/try_catch_test.dart:37:7`, `:168:7`,
  `exception/on_catch_malformed_type_test.dart:16:5`, `:36:7` (exibição `InvalidType`).

#### §6 Nulidade e verificadores

##### Achado transversal: local inicializado/atribuído com valor de tipo **inválido**

Vários FN/FP abaixo e no `unchecked_use_of_nullable_value` vêm da mesma regra do fluxo: escrever num local de tipo declarado
`T?` um valor de tipo `InvalidType` (expressão que não resolve: `o.nonNull` com `o` de classe indefinida, chamada a função
com retorno de classe indefinida) **promove** o local ao tipo de interesse `NonNull(T?) = T`, porque `InvalidType` é subtipo
de tudo (`_fe_analyzer_shared/lib/src/flow_analysis/flow_analysis.dart:5931-5960` `_initialize` → `FlowModel.write` com
`promoteToTypeOfInterest`). Efeitos no oráculo: `int? i = o.nonNull; i.isEven;` não relata `unchecked…`
(`InvalidNullAwareOperator__invalid_nonNullable.dart`); `i == null` relata `unnecessary_null_comparison` "never null"
(`UnnecessaryNullComparisonTrue__equal_in_5aade068.dart`, 8 FN); `i?.isEven` relata `invalid_null_aware_operator`
(`InvalidNullAwareOperator__invalid_nullable.dart:5:4`). No DartForge, `BodyInferrer::escrever_fluxo`
(`crates/types/src/inferencia/fluxo.rs`) trata o escrito `dynamic` como "o declarado" (sem promoção); o tipo inválido é
representado como `dynamic` + a marca `UnitBodyTypes::tipos_invalidos`/`locais_invalidos`. Regra a implementar: se a
expressão escrita está marcada inválida, o tipo escrito efetivo é `Never`-like para a promoção: promover ao primeiro tipo de
interesse (`nao_nulo_promocao(declarado)` quando o declarado é anulável).

##### `not_initialized_non_nullable_instance_field` (perda 15: FN 12, FP 2, pos 1)

- **Emissão:** dois códigos com o mesmo nome compartilhado: `NOT_INITIALIZED_NON_NULLABLE_INSTANCE_FIELD` em
  `ErrorVerifier._checkForNotInitializedNonNullableInstanceFields` (`analyzer/lib/src/generated/error_verifier.dart:4837-4864`),
  chamado de `_checkForFinalNotInitializedInClass` (`:3625-3645`) **só quando a classe não tem construtor gerador não
  sintético**; e `…_CONSTRUCTOR` em `ConstructorFieldsVerifier.reportNotInitializedNonNullable`
  (`analyzer/lib/src/error/constructor_fields_verifier.dart:175-194`), no `returnType` do construtor, um por campo (nomes
  ordenados).
- **Condição (sem construtor):** campo não `static`, não `late`, não `final` (o `const` não é `final`: `const int v;` de
  instância relata), classe não é `Struct`/`Union` do FFI, campo não `abstract` nem `external`, sem inicializador, tipo
  potencialmente não anulável (`T`, `int`, `var v` com tipo inferido/herdado não anulável).
- **Posição:** o `VariableDeclaration` (nome até o fim do declarador); no construtor: o nome do tipo de retorno (`P` em `P._()`).
- **Mensagem:** "Non-nullable instance field '{0}' must be initialized." / "…_CONSTRUCTOR": "Non-nullable instance field '{0}'
  must be initialized." (correção diferente).
- **No DartForge:** `crates/analise/src/inicializacao.rs` (~l. 51 e ~202). FN: `T v;` (`VariableNotInitialized__class_instanceF_29bac040.dart`),
  `const int v;` de instância em classe/enum/extensão (`…_aa07ca15.dart`, `…enum_instanceFi_f37cbf12.dart`, `…extension_insta_0857cd32.dart`),
  `var v;` herdando tipo não anulável (`…_ac43854d.dart`), `augment abstract int foo;` (3, augmentations — fora de escopo),
  `could_not_infer/CouldNotInfer__constructors_inferenceFBounded.dart:7:3` (dois campos `t`, `u` no construtor `P._()`).
  Ver as 2 FP e a posição no placar (não analisadas aqui: fora do fluxo).

##### `unnecessary_set_literal` (perda 12: FN 12)

- **Emissão:** `BestPracticesVerifier._checkForUnnecessarySetLiteral` (`analyzer/lib/src/error/best_practices_verifier.dart:1283-1318`),
  de `visitFunctionExpression`.
- **Condição:** corpo `=> e` cujo `e` é um literal de **conjunto** (`SetOrMapLiteral.isSet`); o tipo de retorno (do parâmetro
  de função ao qual a closure é passada — `staticParameterElement.type` — ou o escrito da `FunctionDeclaration`) é `void`,
  `Future<void>` ou `FutureOr<void>`.
- **Posição:** o literal `{…}`. **Mensagem:** "Braces unnecessarily wrap this expression in a set literal." (WarningCode).
- **No DartForge:** não implementado; entrar em `funcoes::expressao_de_funcao`/`inferir_funcao_declarada` (corpo `=>`), com
  o literal já decidido conjunto. Amostras `UnnecessarySetLiteral__functionDeclarat_14130680.dart` (`void f() => {1, 2};`),
  `__expressionFuncti_12dcc4e6.dart` (`g(() => {1, 2})` com parâmetro `void Function()`).

##### `late_final_field_with_const_constructor` (perda 10: FN 10)

- **Emissão:** `ErrorVerifier._checkForLateFinalFieldWithConstConstructor` (`analyzer/lib/src/generated/error_verifier.dart:4068-4091`).
- **Condição:** campo de instância `late final` numa classe/enum (não extensão) com algum construtor `const` não factory.
- **Posição:** a palavra `late`. **Mensagem:** "Can't have a late final field in a class with a generative const constructor."
- **No DartForge:** não implementado (crates/analise, junto das regras de campo). Amostras `VariableNotInitialized__class_instanceF_0b3e114e.dart:2:3`,
  enums (construtores de enum são `const`), `InvalidReferenceToThis__enum_instanceFi_2e3283ae.dart:3:3`.

##### `dead_null_aware_expression` (perda 8: FN 6, FP 2)

- **Emissão:** `ErrorVerifier._checkForDeadNullCoalesce` (`analyzer/lib/src/generated/error_verifier.dart:3045-3052`), de
  `visitAssignmentExpression` (`??=`, `:361`, com `node.readType`) e `visitBinaryExpression` (`??`, `:394`).
- **Condição:** tipo do lado esquerdo **estritamente não anulável** (`TypeSystemImpl.isStrictlyNonNullable`,
  `analyzer/lib/src/dart/element/type_system.dart:1290-1309`: falso para `dynamic`, inválido, `_`, `void`, `Null`, qualquer
  `?`; `FutureOr<T>` segue `T`; **tipo de extensão só se tem `implements`** (`interfaces.isNotEmpty`); parâmetro de tipo
  segue o limite; o resto — inclusive `Never` — verdadeiro).
- **Posição:** o lado direito. **Mensagem:** "The left operand can't be null, so the right operand is never executed."
- **No DartForge:** `expr.rs` (`DEAD_NULL_AWARE_EXPRESSION`, ~l. 1274). FN: `dot_shorthands/*_if_null_error_test.dart` (6) — o
  lado esquerdo é um atalho de ponto cujo tipo é um **tipo de extensão com `implements num`** (`extension type IfNullConstructorExt(int x) implements num`):
  estritamente não anulável pela regra acima; o DartForge trata tipo de extensão como anulável. FP: `x[0] ??= 1` com `x: Never`
  (`InvalidUseOfNever__indexExpression_neve_e8cb085e.dart:4:12`): com receptor `Never` o `PropertyElementResolver` sai cedo
  (`receiver_of_type_never`) sem tipo de leitura — o `readType` não é `Never` e nada sai; `C ??= null` com `C` classe
  (`if_null/assignment_behavior_test.dart:195:9`): alvo não é variável (erro de atribuição) e não há `readType` válido.

##### `not_initialized_non_nullable_variable` (perda 8: FN 8)

- **Emissão:** `ErrorVerifier._checkForNotInitializedNonNullableVariable` (`analyzer/lib/src/generated/error_verifier.dart:4875-4915`),
  para variáveis de topo e campos `static` (`_checkForNotInitializedNonNullableStaticField`, `:4867-4872`).
- **Condição:** não `const`, (topo) não `final`, não `late`, não `external`, tipo **escrito** potencialmente não anulável, sem
  inicializador. `abstract` NÃO suprime aqui (`static abstract int foo;` relata).
- **Posição:** o nome. **Mensagem:** "The non-nullable variable '{0}' must be initialized."
- **No DartForge:** `crates/types/src/sobrescritas.rs:1196-1241`. FN: os 8 são `static abstract int foo;`/`static abstract final int foo;`
  (`executable_body/ExecutableBody__class_staticField_abstr_*`): o DartForge pula campos `abstract` (só deve pular `external`).

##### `unnecessary_null_comparison` (perda 8: FN 8)

- **Emissão:** `BestPracticesVerifier._checkForInvariantNullComparison` (`analyzer/lib/src/error/best_practices_verifier.dart:1059-1092`;
  "never null" `NEVER_NULL_TRUE/FALSE`) e `BinaryExpressionResolver` (`analyzer/lib/src/dart/resolver/binary_expression_resolver.dart:123-150`;
  "always null").
- **Condição/posição:** `null == e` com `e` estritamente não anulável → de `null` até o operador; `e == null` → do operador
  até o fim de `null` (`!=` → `…_TRUE`).
- **Mensagem:** "The operand can't be 'null', so the condition is always 'false'." / "…'true'.".
- **No DartForge:** `expr.rs` (~l. 3248-3280). As 8 FN são o achado transversal (local promovido ao tipo de interesse por
  escrita de valor inválido).

##### `unnecessary_type_check` (perda 8: FN 3, FP 5)

- **Emissão:** `BestPracticesVerifier._checkAllTypeChecks` (`analyzer/lib/src/error/best_practices_verifier.dart:771-820`).
- **Condição:** esquerdo inválido → nada; `is dynamic` → relata; direito inválido → nada; `is Null` → `type_check_is_null`
  (só literal `null is Null` é desnecessário); senão `isSubtypeOf(esquerdo, direito)` → relata.
- **Posição:** a expressão `is` inteira. **Mensagem:** "Unnecessary type check; the result is always 'true'." / "…'false'."
- **No DartForge:** `expr::teste_de_tipo_desnecessario` (~l. 3383-3430). FN `function/type_alias6_test.dart:24:18`,
  `prefix/prefix16_test.dart:29:17`/`30:17` (tipos de função via `typedef` e closures: o tipo estático da closure
  `(Object x) => x` é subtipo do typedef); FP `generic/function_typedef2_test.dart:54:17`, `function_typedef3_test.dart:46:17`
  (`foo is M` com typedef genérico — subtipagem de funções genéricas/limites), `interface/injection1_test.dart:21:18`,
  `injection2_test.dart:19:18` (`new C() is S` — hierarquia com erro), `nnbd/syntax/nullable_type_ambiguous_test.dart:32:13`
  (`a is bool?` com `a` promovido a `bool` pela atribuição `a = true` de `dynamic`… o analyzer não promove `dynamic` por
  atribuição: promoção só a tipos de interesse — `escrever_fluxo`).

##### `non_nullable_equals_parameter` (perda 7: FN 7)

- **Emissão:** `BestPracticesVerifier._checkForNullableEqualsParameterType` (`best_practices_verifier.dart:1150-1184`).
- **Condição:** método `operator ==` com exatamente 1 parâmetro (de qualquer espécie: `[a]`, `{a}`, genérico `==<T>(a)`), cujo
  tipo é `Object?` ou `dynamic` (inclusive herdado: `==(other)` sem tipo herda `dynamic`/`Object?` da superclasse).
- **Posição:** o nome do operador (`==`). **Mensagem:** "The parameter type of '==' operators should be non-nullable."
- **No DartForge:** não implementado (crates/analise ou `sobrescritas.rs`, onde o tipo herdado do parâmetro já é calculado).

##### `null_argument_to_non_null_type` (perda 7: FN 7)

- **Emissão:** `NullSafeApiVerifier` (`analyzer/lib/src/error/null_safe_api_verifier.dart:29-80`): `methodInvocation`
  (`Completer<T>.complete`, alvo real, inclusive cascata) e `instanceCreation` (`Future<T>.value`).
- **Condição:** no máximo 1 argumento; `T` não anulável; argumento ausente ou de tipo `Null`-equivalente.
- **Posição:** o argumento, ou a invocação inteira se não há argumento (`Completer<int>().complete()` → do início do alvo;
  em cascata `c..complete()` → a seção `..complete()`). **Mensagem:** "'{0}' shouldn't be called with a 'null' argument for
  the non-nullable type argument '{1}'." — `{0}` "Completer.complete"/"Future.value", `{1}` `T.getDisplayString()`.
- **No DartForge:** não implementado; `chamadas::chamada`/`instanciacao` (classe de `dart:async`).

##### `cast_from_null_always_fails` (perda 6: FN 6)

- **Emissão:** `BestPracticesVerifier.visitAsExpression` (`best_practices_verifier.dart:135-150`) e `visitCastPattern` (`:168-182`).
- **Condição:** tipo alvo não anulável (`isNonNullable`: inclui `Never`, `T extends Object`) e o operando (ou o tipo casado
  do padrão) é `Null`.
- **Posição:** a expressão `as` inteira / o padrão cast inteiro. **Mensagem:** "This cast always throws an exception because
  the expression always evaluates to 'null'."
- **No DartForge:** não implementado; `expr.rs` `ExprKind::As` e `padroes::tipar` `Cast`.

##### `not_null_aware_null_spread` (perda 6: FN 6)

- **Emissão:** `LiteralElementVerifier` (`analyzer/lib/src/error/literal_element_verifier.dart:180-200` lista/conjunto,
  `:275-292` mapa).
- **Condição:** espalhamento sem `?` cujo tipo é subtipo de `Null` (e não de `Never`).
- **Posição:** a expressão espalhada. **Mensagem:** "The Null-typed expression can't be used with a non-null-aware spread."
- **No DartForge:** não implementado; `colecoes::visitar` `Spread` (sai junto com `invalid_use_of_null_value`, conferido no
  oráculo vivo).

##### `unnecessary_nan_comparison` (perda 5: FN 5)

- **Emissão:** `BestPracticesVerifier._checkForInvariantNanComparison` (`best_practices_verifier.dart:1030-1057`) e no padrão
  constante (`:250-265`, `case double.nan`).
- **Condição/posição:** um operando é `double.nan` (`isDoubleNan`: `double.nan` prefixado/identificador do `dart:core`);
  `==` → `…_FALSE`, `!=` → `…_TRUE`; intervalo do `double.nan` até o operador (nan à esquerda) ou do operador até o fim
  (nan à direita); padrão: a constante.
- **Mensagem:** "A double can't equal 'double.nan', so the condition is always 'false'." / "…'true'."
- **No DartForge:** não implementado; `expr::binario` (igualdade) e `padroes::tipar` `Constant`.

##### `null_check_always_fails` (perda 4: FN 4)

- **Emissão:** `BestPracticesVerifier.visitPostfixExpression` (`best_practices_verifier.dart:686-697`).
- **Condição:** `e!` com tipo de `e` exatamente `Null` (`null!`, `(null)!`, `g()!` com retorno `Null`, `(await g())!`).
- **Posição:** a expressão `e!` inteira. **Mensagem:** "This null-check will always throw an exception because the
  expression will always evaluate to 'null'."
- **No DartForge:** não implementado; `expr::unario` `UnaryOp::NullAssert`.

##### `nullable_type_in_catch_clause` (perda 4: FN 4)

- **Emissão:** `BestPracticesVerifier._checkForNullableTypeInCatchClause` (`best_practices_verifier.dart:1186-1204`).
- **Condição:** `on T` com `T` não inválido e potencialmente anulável (`dynamic`, `T?`, função `?`, parâmetro de tipo de
  limite anulável).
- **Posição:** a anotação de tipo. **Mensagem:** "A potentially nullable type can't be used in an 'on' clause because it
  isn't valid to throw a nullable expression."
- **No DartForge:** não implementado; `instrucoes` `StmtKind::Try` (o tipo de `c.on_type` já é resolvido).

##### `nullable_type_in_implements_clause` (perda 4: FN 4) — publicado (`nullable_type_in_implements_clause`)

- **Emissão:** `NamedTypeResolver._verifyNullability` (`analyzer/lib/src/dart/resolver/named_type_resolver.dart:381-410`).
- **Condição:** o tipo resolvido (já com o alias expandido) tem sufixo `?` — inclusive por um `typedef X = A?`.
- **Posição:** o tipo nomeado. **Mensagem:** "A class, mixin, or extension type can't implement a nullable type."
- **No DartForge:** `crates/analise/src/clausulas.rs` (~l. 1247): FN em **tipo de extensão** (`extension type E(A _) implements A?`,
  `implements B` com `typedef B = A?`; `ExtensionTypeImplementsDisallowedType___c43dde67.dart`) — o ramo dos tipos de
  extensão não confere a nulidade (nem a do alias).

##### `unnecessary_question_mark` (perda 4: FN 4)

- **Emissão:** `BestPracticesVerifier.visitNamedType` (`best_practices_verifier.dart:654-675`).
- **Condição:** tipo nomeado com `?` cujo tipo é `Null` (o da `dart:core`) ou `dynamic` escrito literalmente, e sem alias.
- **Posição:** o token `?`. **Mensagem:** "The '?' is unnecessary because '{0}' is nullable without it." — `{0}` = nome
  qualificado escrito.
- **No DartForge:** não implementado (resolução de anotações, crates/types `resolve`).

##### `unnecessary_final` (perda 3: FN 3)

- **Emissão:** `BestPracticesVerifier._checkFinalParameter` (`best_practices_verifier.dart:828-835`), de
  `visitFieldFormalParameter`/`visitSuperFormalParameter`.
- **Condição:** `final this.x` / `final super.x`. **Posição:** a palavra `final`. **Mensagem:** "The keyword 'final' isn't
  necessary because the parameter is implicitly 'final'."
- **No DartForge:** não implementado (crates/analise, parâmetros de construtor).

##### `cast_from_nullable_always_fails` (perda 2: FN 2)

- **Emissão:** `ResolverVisitor.visitAsExpression` (`analyzer/lib/src/generated/resolver.dart:1866-1886`).
- **Condição:** `x as T` com `x` identificador de local/parâmetro (`PromotableElement`) de tipo declarado anulável, tipo de
  `x` no ponto não `Null`, `T` não anulável, e `x` **definitivamente não atribuído** (o valor é sempre `null`).
- **Posição:** o identificador `x`. **Mensagem:** "This cast will always throw an exception because the nullable local
  variable '{0}' is not assigned."
- **No DartForge:** não implementado; `expr.rs` `ExprKind::As` (o fluxo já sabe `nao_atribuida`).

##### `unnecessary_no_such_method` (perda 2: FN 2)

- **Emissão:** `BestPracticesVerifier._checkForUnnecessaryNoSuchMethod` (`best_practices_verifier.dart:1234-1280`).
- **Condição:** método `noSuchMethod` cujo corpo é só `super.noSuchMethod(x)` (=> ou `{ return …; }`) e o `noSuchMethod`
  chamado não é o de `Object`.
- **Posição:** o nome do método. **Mensagem:** "Unnecessary 'noSuchMethod' declaration."
- **No DartForge:** não implementado.

##### `invalid_null_aware_operator` (perda 1: FN 1) — publicado

- **Emissão:** `ErrorVerifier._checkForUnnecessaryNullAware` (`analyzer/lib/src/generated/error_verifier.dart:5553-5600`).
- **No DartForge:** `expr.rs` (~l. 1319-1345). A 1 FN (`InvalidNullAwareOperator__invalid_nullable.dart:5:4`) é o achado
  transversal (`int? i = o.nullable` com `o` inválido → `i` promovido a `int`).

#### §7 Extra

##### (extra, fora da lista B) `constant_pattern_never_matches_value_type` (perda 52: FN 52) — publicado

- **Emissão:** `ConstantVerifier.visitConstantPattern` (`analyzer/lib/src/dart/constant/constant_verifier.dart:130-160`) com
  `_canBeEqual` (`:517-546`).
- **Condição:** a constante avalia sem erro; `value.hasPrimitiveEquality` (`analyzer/lib/src/dart/constant/value.dart`:
  bool/int/double/String/Null/Symbol/Type/função/lista/mapa/conjunto sempre; registro se todos os campos; instância genérica
  se `==` e `hashCode` concretos vêm de `Object`, `:1496-1523`); tipo da constante = **tipo do valor** (`A<num>` em
  `const A<num>()`, `bool` em `const (1 == 2)`/`!false`, `Symbol`, `Type`, `List<dynamic>` em `const []`); tipo casado =
  `matchedValueType` (o **promovido** pelo fluxo de padrões, Apêndice A) com apagamento de tipo de extensão; e
  `!_canBeEqual(c, M)`: `M` interface → `c <: eliminateToGreatest(M)` (e `int` × `double` pode); `M` parâmetro de tipo →
  usa `promotedBound ?? bound` (com `?` se `M` tem `?`) quando o limite não referencia parâmetros (cobre `T & int`);
  `M` função → só `null` pode (se `M` anulável); outros → pode.
- **Posição:** o padrão constante. **Mensagem:** "The matched value type '{0}' can never be equal to this constant of type
  '{1}'." (DartType).
- **No DartForge:** `padroes::constante_nunca_casa`/`pode_ser_igual` (`crates/types/src/inferencia/padroes.rs`), restrito a
  literais e enums. FN: (a) `T & int` (interseção) não tratada em `pode_ser_igual` (`ConstantPatternNeverMatchesValueType__b_412c86ec.dart`,
  `patterns/invalid_const_pattern_binary_test.dart` 11×) — usar o `bound` da interseção; (b) tipo casado promovido pelos
  casos anteriores (`case null:` depois de `case null:` sobre `bool?`, `null => …` depois de `Null _ => …`:
  `bool_switch_test.dart:165:10`, `enum_switch_test.dart:249:10`, `null_type_test.dart:33:5`, `object_pattern_switch_test.dart:204:10`)
  — Apêndice A; (c) constantes não literais: `const A()`/`const A<num>()`/`const G()`, `const []`, `const {}`, `const #a`,
  `G<int>` (literal de tipo → `Type`), `!false`, `1 == 2` (`ConstantPatternNeverMatchesValueType__c_*`, `patterns/invalid_const_pattern_test.dart`
  ~30×) — o tipo do valor (estático da criação/literal) e a igualdade primitiva por classe (busca concreta de `==`/`hashCode`).
  Cuidado (publicado): só relatar quando a constante avalia sem erro (o avaliador `crate::constant::ConstantEvaluator`).

#### Apêndice A — análise de fluxo de padrões (o que o DartForge não modela)

Fonte: `_fe_analyzer_shared/lib/src/flow_analysis/flow_analysis.dart` (implementação `_FlowAnalysisImpl`) e as chamadas em
`_fe_analyzer_shared/lib/src/type_inference/type_analyzer.dart`.

**Estado.** Durante um casamento há (a) o fluxo corrente `_current` = "o padrão casou até aqui", (b) o acumulador
`_unmatched` = junção de todos os estados em que o casamento já falhou, (c) a **referência do valor casado** (uma variável
temporária com chave de promoção própria, `_pushScrutinee` `:6077-6100`, `createReference`), cujo tipo promovido é o
`matchedValueType` que os padrões veem (`flow.getMatchedValueType()`), e (d) `_scrutineeReference`: se o escrutínio é uma
variável local (ou propriedade promovível) **e** `allowScrutineePromotion` (só `switch` e `if-case`; não em declaração,
atribuição por padrão nem `for` com padrão), as promoções do valor casado de topo também valem para ela enquanto a versão SSA
for a mesma (`:5217-5245`).

**Entrada/saída.** `_pushPattern` (`:6060-6066`): `_unmatched = _current.setUnreachable()`. `_popPattern(guard)`
(`:6037-6050`): devolve `_unmatched` (com o ramo falso da guarda juntado); o `_current` vira o ramo verdadeiro da guarda.
Subpadrões de lista/mapa/registro/objeto ganham **referência nova** (tipo = tipo do elemento/campo); `&&`, `||`, `(p)`, `p?`,
`p!`, `p as T` reusam a mesma.

**Regras por padrão** (com `t` = tipo promovido corrente da referência):
- `promoteForPattern(matchedType: t, knownType: K, matchFailsIfWrongType = true, matchMayFailEvenIfCorrectType = false)`
  (`:5191-5260`): se `K` inválido → `_unmatched ∪= _current`, devolve false. Se `t` não anulável, `K = promoteToNonNull(K)`.
  `covers = t <: K` (com apagamento de tipo de extensão). `ifTrue/ifFalse = tryPromoteForTypeCheck(ref, K)` (`:2525-2560`:
  `ifTrue` promove para `K` (ou `t & K`); `ifFalse` promove para `factor(t, K)` **exceto quando o fator é `Never` ou igual a
  `t`: então não promove nem torna inalcançável**). O mesmo no escrutínio (se aplicável). `_current = ifTrue`; se
  `matchFailsIfWrongType && !covers` → `_unmatched ∪= ifFalse`; se `matchMayFailEvenIfCorrectType` → `_unmatched ∪= ifTrue`.
  Chamado por: variável com tipo (`type_analyzer.dart:515`), curinga com tipo (`:2094`), objeto (`:1301`), registro
  (`:1606`, depois `:1645` com `matchFailsIfWrongType: false`), lista (`:810`, `matchMayFail… = !(um único elemento e ele é
  resto)`), mapa (`:1097`, `matchMayFail… = true`), cast (`:377`, `matchFailsIfWrongType: false`), variável atribuída
  (`:344`, `matchFailsIfWrongType: false`).
- variável/curinga sem tipo: nada (sempre casa).
- `p?` (`nullCheckOrAssertPattern_begin`, `:5084-5110`): `_unmatched ∪= _current` **sempre** (mesmo com `t` não anulável);
  `ifNotNull = _nullCheckPattern` (`:5998-6035`: `null` se `t` não anulável; senão promove a referência (e o escrutínio)
  para não nulo, e se `t` é `Null` torna inalcançável); `_current = ifNotNull ?? _current`. `p!`: igual sem o `∪=`.
- constante `c` e `== c`/`!= c` (`constantPattern_end` `:4489-4505`, `equalityRelationalPattern_end` `:4626-4631` →
  `_handleEqualityCheckPattern` `:5832-5915` com `_equalityCheck` `:5642-5673`): se `t` e `c` são ambos Null-equivalentes →
  garantidamente igual (constante: nada muda; `!=`: `_unmatched ∪= _current`, `_current` inalcançável); se um é Null e o outro
  não anulável → sem informação; se `c` é o literal `null` (isNull) e `t` anulável → é checagem de nulo: `ifNotNull` como
  acima; constante (`==`): `_unmatched ∪= ifNotNull`; `!=`: `_unmatched ∪= _current`, `_current = ifNotNull`. Qualquer
  outro caso: sem informação → `_unmatched ∪= _current`. (Para bibliotecas sem padrões, `constantPattern_end` faz sempre
  `_unmatched ∪= _current`.)
- relacional `<`, `<=`, `>`, `>=` (`nonEqualityRelationalPattern_end` `:5016-5021`): `_unmatched ∪= _current`.
- `p1 && p2`: `p1` depois `p2`, mesma referência.
- `p1 || p2` (`logicalOrPattern_begin/afterLhs/end` `:4995-5014`, `:4978-4993`): guarda `prev = _unmatched`;
  `_unmatched = _current.setUnreachable()`; analisa `p1`; `lhsCasou = _current`; `_current = _unmatched` (falhas de `p1`);
  `_unmatched = prev`; **`checkUnreachableNode(p2)`** (`analyzer/lib/src/generated/resolver.dart:1062-1065`); analisa `p2`;
  `_current = lhsCasou ∪ _current`; `flowEnd(p2)` (`analyzer/lib/src/dart/ast/ast.dart:11595`).

**`switch` instrução** (`type_analyzer.dart:1907-2035`, flow `:5335-5400`): o escrutínio vira a referência; para cada
`case`/`default` de cada grupo: `_current = _unmatched do switch` (estado "nenhum caso anterior casou");
`checkUnreachableNode(member)`; padrão; `_unmatched do switch = _popPattern(guard)`; `flowEnd(member)` (cabeça). O corpo do
grupo começa na junção dos "casou" dos membros (`switchStatement_endAlternatives`); com rótulo, junção conservadora do estado
antes do `switch`. `default` casa sempre (`_unmatched` fica inalcançável). No fim (`switchStatement_end` `:5350-5374`):
saída = breaks ∪ fim dos corpos que completam ∪ (`_unmatched` se não exaustivo).
**`switch` expressão** (`type_analyzer.dart:1779-1900`): igual, com `checkUnreachableNode(case)` antes e `flowEnd(case)` depois de cada caso.
**`if-case`/elemento `if-case`**: `ifCaseStatement_afterExpression` (`flow_analysis.dart:4747-4765`, `type_analyzer.dart:592`, `:657`) empurra escrutínio (com promoção) e
padrão; `thenBegin(guard)`: then = casou ∧ guarda; else = `_unmatched`.

**Consequências observáveis** (todas FN hoje):
1. tipo casado mais estreito nos casos seguintes: `switch (b) { case true: … case false: … case null: … case null: }` com
   `b` `bool?` → o 4º `null` é casado contra `bool` → `constant_pattern_never_matches_value_type`;
2. trechos mortos: `int() || 0` sobre `int` (o `|| 0` é inalcançável); `case 1:` depois de um caso que sempre casa (o `case`
   é morto e o seu corpo também, se só ele entra no corpo);
3. variáveis de padrão com o tipo promovido (`case int? x?` → `x: int`; `case num() && var y` → `y: num`).

**No DartForge:** `padroes::caso` (`crates/types/src/inferencia/padroes.rs`) usa `irrefutavel` (aproximação: curinga,
variável, cast, `!`, `&&`, `||`) para decidir o "não casou" e `tipo_casado` para promover só o escrutínio no ramo "casou";
`tipar` usa o tipo não promovido `t` em todos os subpadrões; `instrucoes` (`StmtKind::Switch`) e
`padroes::expressao_switch` passam o mesmo `t` a todos os casos. Proposta: uma referência sintética por casamento
(`Corpo::declarar_sintetico` com o tipo do valor casado), um campo `Corpo::padrao_nao: Option<Fluxo>` com o `_unmatched`, e
`tipar` aplicando as regras acima a cada nó (promoção via `promover`/`fator` com a exceção do fator `Never`), mais
`Corpo::escrutinio: Option<(LocalId, versão)>` para a promoção do escrutínio. O tipo `t` de cada caso passa a ser
`fluxo.tipo_atual(ref)`.

#### Apêndice B — o verificador de código morto (`NullSafetyDeadCodeVerifier`)

`analyzer/lib/src/error/dead_code_verifier.dart:186-493`. Estado: `_firstDeadNode` (o primeiro nó inalcançável do trecho em
curso). Dois ganchos, chamados pelo `ResolverVisitor` durante a resolução (com a análise de fluxo em curso):

1. `visitNode(node)` (`:392-419`), chamado por `ResolverVisitor.checkUnreachableNode` (`analyzer/lib/src/generated/resolver.dart:694-696`):
   se já há `_firstDeadNode`, nada; se `flow.isReachable`, nada; se `node` está num intervalo de `catch` morto já relatado
   (`_deadCatchClauseRanges`), nada; senão `_firstDeadNode = node`.
   **Quem chama** (é o que decide onde o trecho COMEÇA):
   - quase todo `visitX` de expressão e instrução no começo da visita (`resolver.dart`: literais, identificadores
     (`simple_identifier_resolver.dart:37`), `this`, `super`, `as`, `is`, `await`, `throw`, `rethrow`, atribuição, binário,
     índice, invocação de método, acesso a propriedade, prefixo/posfixo, criação de instância, listas/conjuntos/mapas,
     entradas de mapa, espalhamento, registro, interpolação e cada pedaço de string (`visitInterpolationString`), `switch`,
     condicional, bloco, instruções, `catch`, etc.);
   - **a lista de argumentos** inteira (`analyzer/lib/src/dart/resolver/invocation_inferrer.dart:530`, `_visitArguments`), em
     TODA invocação resolvida pelo `InvocationInferrer` (método, função, construtor, inclusive receptor `Never`/inválido) —
     por isso `x.foo(1 + 2)` com `x` `Never` começa no `(`; `x()` começa no `(`;
   - lista de argumentos de tipo (`visitTypeArgumentList`);
   - operando direito de `&&`/`||` (`binary_expression_resolver.dart:231`, `:260`), ramos do condicional
     (`resolver.dart:2266`, `:2277`), operando direito de padrão `||` (`resolver.dart:1062-1065`,
     `handle_logicalOrPattern_afterLhs`), cada `case`/`default` de `switch` (instrução: o `SwitchMember`; expressão: o
     `SwitchExpressionCase`) antes do padrão (`resolver.dart:1144-1157`, `handleSwitchBeforeAlternative`).
   Padrões (fora o `||`) não chamam: o trecho que começa num padrão começa no `case`/no próximo nó de expressão/instrução.

2. `flowEnd(node)` (`:214-337`) — fecha o trecho **se `node` contém `_firstDeadNode`** (`_containsFirstDeadNode`, sobe pelos
   pais). Chamado em (o que decide onde o trecho TERMINA):
   - `then`/`else` de `if` instrução e elemento (`resolver.dart:1027-1060`); `then`/`else` do condicional (`:2270`, `:2289`);
     operando direito de `&&`/`||` (`binary_expression_resolver.dart:239`, `:268`); operando direito de padrão `||`
     (`analyzer/lib/src/dart/ast/ast.dart:11595`); cabeça de `case`/`default` (`resolver.dart:1083`, `:1098`); corpo do grupo
     de `case` (`:1126`, `handleMergedStatementCase` → último membro); caso de expressão `switch` (`:874`); corpo de `for`/`for-in`
     (`:2754`); corpo de `while` (`:3951`); corpo do `try` (`:3827`) e de cada `catch` (`:3842`); declaração de função
     (`:2814`), método (`:3201`), construtor (`:2341`); expressão de função (`function_expression_resolver.dart:68`);
     inicializador de variável de topo/campo (`variable_declaration_resolver.dart:69`).
   - **NÃO** chamam: `do-while` (o trecho do corpo segue até o `flowEnd` de fora), bloco solto `{ }`, rótulo, instrução de
     expressão, `return`, inicializador de local.

   Cálculo do intervalo quando fecha (`:252-333`):
   ```
   offset = firstDead.offset
   if node != firstDead:            // fechou num nó que CONTÉM o primeiro morto
      node = corpo (FunctionDeclaration/FunctionExpression/MethodDeclaration → body; BlockFunctionBody → block)
      if node is Block com instruções: node = última instrução
      if node is SwitchMember com instruções: node = última instrução
   else if parent(firstDead) is BinaryExpression: offset = operador          // `&&`/`||`: começa no operador
   if parent is SwitchMember && node == firstDead → relata o token `case`/`default` (keyword) e para (:228-235)
   casos especiais pelo pai do primeiro morto:
      Assertion e firstDead é a mensagem → não relata (:238-241)
      ConstructorDeclaration e firstDead é `;` (EmptyFunctionBody) ou bloco vazio → não relata (:242-250)
      ConstructorInitializer → relata o inicializador inteiro e offset = node.end (:278-284)
      DoStatement → relata `do` (ou `do {`) e `}` while…`;` separados; o resto começa depois do `;` (:285-308)
      ForParts (firstDead é a condição/atualizador) → node = último atualizador (:309-310)
      ForStatement (firstDead é o corpo) → `_reportForUpdaters` (:311-312, `:428-441`: relata do 1º ao último atualizador)
      Block cujo pai é ForStatement → `_reportForUpdaters` (:313-317)
      BinaryExpression → offset = operador, node = operando direito (:318-320)
      LogicalOrPattern e firstDead é o direito → offset = operador (:321-324)
   length = node.end - offset; relata DEAD_CODE se length > 0
   ```
   Depois de relatar, `_firstDeadNode = null`: o próximo nó inalcançável abre outro trecho.

3. `catch`: `tryStatementEnter`/`verifyCatchClause` (`:339-378`, `_CatchClausesVerifier` `:495-545`) — ver
   `dead_code_on_catch_subtype` e `dead_code_catch_following_catch`. O intervalo relatado vai do `catch` problemático até o
   fim do último `catch`, e entra em `_deadCatchClauseRanges` (nenhum `dead_code` dentro dele).

4. `?.`/`?[`/`?..` sobre variável **definitivamente não atribuída** de tipo anulável (`_verifyUnassignedSimpleIdentifier`,
   `:443-475`): relata DEAD_CODE no acesso inteiro (subindo por cadeias de invocação/propriedade/índice).

**Alcançabilidade (de onde vem o "inalcançável" dentro de expressões):** além de `return`/`throw`/`break`/`continue`/
`rethrow`, a análise de fluxo marca inalcançável depois de qualquer expressão de tipo estático `Never` (invocação, leitura de
variável ou getter `Never`, `x!` com `x` `Null`…), no ramo `false` de condição que é literal `true` ou variável cujo valor
escrito foi um literal booleano/condição (§ `dead_code`), no lado direito de `?.`/`?[`/`?..` quando o alvo é do tipo `Null`
(`nullAwareAccess_rightBegin`, `_fe_analyzer_shared/lib/src/flow_analysis/flow_analysis.dart:5041-5055`), em padrões
(Apêndice A), e depois de `switch` exaustivo sem saída.


## C. Declarações, herança, construtores, limites, FFI

### Parte A_heranca — herança, sobrescrita e conflitos de membros (analyzer 3.6.2)

Fonte citada: `E:\references\dart-sdk-3.6.2\pkg\analyzer\lib` (prefixo `analyzer/lib/` omitido abaixo:
`src/...:linha`). DartForge: `E:\MyRustProjects\dartforge\crates\...`. Placar base:
`E:\dftemp\analise\trab\placar-base-r3.txt`.

#### 0. Achados transversais (valem para quase todos os códigos do grupo)

##### 0.1 Declarações homônimas compartilham caches no 3.6.2 (`ElementImpl.==` por localização)

`src/dart/element/element.dart:2872` — `ElementImpl.operator ==` compara `kind` e `location`
(`hashCode` = `location.hashCode`, `:2472`). Dois `class A`/`enum E`/`mixin M` de mesmo nome na
mesma biblioteca (o caso de todo `augment class X ...` **sem** o experimento: o parser recupera
`augment` como variável de topo — `expected_token` + `missing_const_final_var_or_type` — e o resto
vira uma **segunda declaração** com `duplicate_definition`) são elementos **iguais**. Consequências:

- `InheritanceManager3._interfaces` (`src/dart/element/inheritance_manager3.dart:69`, consulta em
  `getInterface` `:239`) devolve para a 2ª declaração a interface já calculada para a 1ª (na prática
  a 1ª — prova: `unused_element/UnusedElement__parameter_isUsed_overrid_83bf2fbf.dart`: o 1º `B` relata
  `non_abstract_class_inherits_abstract_member` 'A._m' e o 2º, que declara `_m` concreto, relata
  `concrete_class_with_abstract_member` no próprio `_m` — a interface/implemented é a do 1º).
  Isso gera, **na posição do nome da 2ª declaração**, cópias de `non_abstract_class_inherits_abstract_member`,
  `inconsistent_inheritance_getter_and_method`, `conflicting_field_and_method`, `conflicting_method_and_field`,
  `conflicting_static_and_instance` etc., calculadas com a interface da 1ª mas com os **membros (AST)
  da 2ª**.
- `ClassHierarchy._map` (`src/dart/element/class_hierarchy.dart:14,44`) idem (`conflicting_generic_interfaces`).
- `_checkForRecursiveInterfaceInheritance` (`src/error/inheritance_override.dart:592`) compara
  `classElement == element`: `augment class A extends A {}` (2º A referindo o 1º, porque o escopo
  resolve o nome para a **primeira** declaração) é "A herda de si mesmo" — ver §recursive.
- `DuplicationDefinitionContext._instanceElementContexts` (`src/error/duplicate_definition_verifier.dart:307`)
  junta membros das duas (o DartForge já faz isso em `analise/src/duplicatas.rs` com `juntar`).
- O que **não** é cache por elemento continua separado: `element.augmented.constants`
  (`enum_without_constants`) usa só as constantes de cada declaração.

**No DartForge** quase todo verificador de herança pula essas classes, e a maioria dos FN do grupo vem
daí:
- `crates/analise/src/clausulas.rs:1114` (`verificador_de_heranca_prossegue`): classe com nome repetido
  na biblioteca fica fora da lista `classes` usada por `sobrescritas_invalidas`, `membros_de_enum`,
  `membros_abstratos`, `getters_e_setters` (`crates/paridade/src/analise.rs:503-523`) — inclusive a **1ª**
  declaração (FN em `NonAbstractClassInheritsAbstractMember__9a2979d9.dart:5:7`,
  `InconsistentInheritance__enum_returnTyp_221d07c1.dart:9:6`).
- `crates/types/src/sobrescritas.rs:1358-1364` (`membros_em_conflito`) e `:1525-1534`
  (`estaticos_de_enum`): pulam a classe se **qualquer linha da unidade** começa com `augment ` ou se o
  nome é repetido.
- `crates/types/src/sobrescritas.rs:1029-1042` (`membros_abstratos`): pula se algum membro tem
  `augment` ou se o texto da declaração tem linha `augment `.
- `getters_e_setters` (`sobrescritas.rs:699-710`): campo com `augment` → biblioteca inteira fora.

Proposta: (1) ligar esses filtros só quando a biblioteca tem `Feature::Augmentations`/`Macros`
(sem o experimento `augment` nem é modificador — `frontend/src/parser/declarations.rs:269`
`parse_augment_opt` já devolve `false`); (2) para homônimos, calcular a interface/hierarquia uma vez
pela **primeira** declaração (chave = (biblioteca, nome, espécie)) e reutilizá-la para as seguintes,
relatando no nome de cada uma com os membros AST dela; (3) referências de tipo a um nome repetido
resolvem para a primeira declaração (hoje `clausulas.rs:731-736` marca `incerta`).

##### 0.2 Ordem dos verificadores e supressões gerais

`src/dart/analysis/library_analyzer.dart:420-456` (`_computeVerifyErrors`): `ConstantVerifier` →
`InheritanceOverrideVerifier.verifyUnit` (`:432`) → `ErrorVerifier` (`:441-451`). Antes disso, por
biblioteca, `MemberDuplicateDefinitionVerifier.checkLibrary`
(`src/error/duplicate_definition_verifier.dart:841`: todos os `_checkUnit` de instância `:750`, depois
todos os `_checkUnitStatic` `:773`). `OverrideVerifier` roda em `_computeWarnings` (`:483`).

`InheritanceOverrideVerifier.verifyUnit` (`src/error/inheritance_override.dart:38-129`) só visita
`ClassDeclaration`, `ClassTypeAlias`, `EnumDeclaration`, `MixinDeclaration` (não extension type nem
extension). Fragmento de augmentation (`element.isAugmentation`, só com o experimento) roda só
`_checkDirectSuperTypes` e para (`:58-61`, `:77-80`, `:96-99`, `:115-118`). `_ClassVerifier.verify`
(`:187`) **para** (retorna `true`, sem nada depois) em: supertipo direto proibido/`Enum` em classe
concreta/mixin de enum com campo (`_checkDirectSuperTypes` `:486`), `CONCRETE_CLASS_HAS_ENUM_SUPERINTERFACE`
transitivo (`:196-205`), herança recursiva (`:207`). Depois disso não para mais: conflitos (`:215`),
membros de mixins (`:233-239`), membros declarados (`:245-274`), enum (`:276-277`),
`GetterSetterTypesVerifier.checkInterface` (`:279-282`), membros abstratos/implementação (`:284-349`).

Os registros do oráculo são deduplicados (`crates/paridade/src/main.rs:383` `regs.dedup()`), e o
nosso lado também (`crates/paridade/src/analise.rs:640-655`, por código+intervalo+mensagem): dois
relatos iguais do analyzer contam um.

---

##### `enum_without_constants` (perda 62: FN 62, FP 0, msg 0, pos 0)
- **Emissão:** `ErrorVerifier.visitEnumDeclaration` (`src/generated/error_verifier.dart:670`), em
  `:702-708`.
- **Condição exata:**
  ```
  if (!element.isAugmentation) {             // fragmento declaração
    if (element.augmented.constants.isEmpty) // constantes da declaração + das augmentations REAIS
      report ENUM_WITHOUT_CONSTANTS at node.name
  }
  ```
  Sem o experimento `augmentations`, `augment enum E {...}` não é augmentation: é um 2º `enum E`
  (duplicado) com as **próprias** constantes (`augmented` não é cache por igualdade, ver §0.1). Cada
  enum não-augmentation sem constantes próprias (e sem augmentations reais com constantes) relata.
- **Posição:** o token do nome do enum (`node.name`), comprimento do nome.
- **Mensagem:** 3.6.2: "The enum must have at least one constant." (correção "Try declaring a
  constant."), sem argumentos. **Oráculo 3.13.4** (arquivos de `sintaxe-nova.json`): "The enum must have
  at least one enum constant." / "Try declaring an enum constant." (7 casos no corpus).
- **Supressões e ordem:** nenhuma específica; sai mesmo com erro de sintaxe no arquivo (todos os 55
  casos têm `expected_token`/`missing_const_final_var_or_type` na linha do `augment`). Sai junto de
  `duplicate_definition` no mesmo nome.
- **No DartForge:** `crates/analise/src/enums.rs` `sem_constantes`, chamado em
  `crates/paridade/src/analise.rs:329`. Classificação dos 62 FN:
  - **55** (corpus 3.6.2): arquivos com `augment enum E ...` sem o experimento (o 2º `E`, vazio, relata;
    ou o 1º vazio quando só o 2º tem constantes, ex. `EnumWithoutConstants__hasConstants_inAu_eadf4494.dart:1:6`).
    Causa: o código antigo juntava constantes por **nome** do enum. A correção já no working tree
    (`git diff crates/analise/src/enums.rs`: só `decl.augment` contribui constantes ao alvo; todo enum
    não-augment com 0 constantes próprias relata) está **de acordo com o fonte**: sem o experimento o
    parser (`declarations.rs:269`) põe `augment = false` e a 2ª declaração não entra em
    `com_constantes`. Fica correta também com o experimento ligado (augmentation real com constantes
    completa o alvo). Ressalva: `com_constantes` é por nome, então com o experimento um `augment enum E`
    para um `E` **duplicado** completaria os dois — irrelevante no corpus.
  - **3** (sintaxe nova, também `augment enum` duplicado: `AugmentationModifierExtra__enum_constru_a5e1b2c2.dart`,
    `ConstructorBody__enum_primaryConstructo_b4bc584e/c6f5a1c0.dart`): a correção acima os acha, mas a
    **mensagem** tem de ser a do 3.13 numa biblioteca de `libs_com_sintaxe_nova`
    (`analise.rs:297-327`; `Diagnostic.message` é público, dá para sobrescrever depois de
    `sem_constantes` quando `sintaxe_nova`).
  - **4** (sintaxe nova, `linguagem/primary_constructors/syntax/empty_body_error_test.dart:8,13,18,26`):
    `enum E1;`, `enum E2(final int x);`, `enum const E3;`, `enum const E4(final int x);` — corpo vazio
    `;` (construtores primários, 3.13). Posição: o nome (`E1`, col 6; com `const`, col 12). Precisa o
    parser aceitar o corpo `;` de enum e `sem_constantes` vê-lo com 0 constantes, e a mensagem 3.13.

##### `conflicting_static_and_instance` (perda 55: FN 55, FP 0, msg 0, pos 0)
- **Emissão (três caminhos):**
  1. Classe/mixin/tipo de extensão, membro local: `MemberDuplicateDefinitionVerifier._checkClassStatic`
     (`src/error/duplicate_definition_verifier.dart:411-461`), fase `_checkUnitStatic` (`:773`).
  2. Enum (local **e** herdado): `_checkEnumStatic` (`:647-685`), mesma fase.
  3. Classe/mixin/tipo de extensão, herdado: `ErrorVerifier._checkForConflictingClassMembers`
     (`src/generated/error_verifier.dart:2364`; chamado de `visitClassDeclaration:510`,
     `visitMixinDeclaration:1254`, `visitExtensionTypeDeclaration:824`; **não** de enum nem de alias de classe).
- **Condição exata:**
  - (1) para cada `FieldDeclaration` estático (cada variável) e `MethodDeclaration` estático (método,
    getter ou setter) do fragmento: `instanceGetters.containsKey(name) || instanceSetters.containsKey(name)`
    (os mapas do contexto do elemento, já com **todos** os membros de instância de todos os fragmentos /
    homônimos) e `declarationElement is InterfaceElement` → relata `[className, name, className]`.
  - (2) `for accessor in fragment.accessors` (inclui os getters sintéticos das **constantes** e dos campos
    estáticos, e o getter `values`) com `accessor.source == unidade atual` e `accessor.isStatic`:
    `instance = _getInterfaceMember(declaration, baseName)` (`:827`: `getMember2(element, Name(lib, n))`
    ou, se nulo, `getMember2(element, Name(lib, 'n='))` — a **interface inteira**: declarados + herdados de
    `Enum`/`Object`/mixins/interfaces); relata se `instance != null && baseName != 'values'`. Depois
    `for method in fragment.methods` estáticos: mesmo teste, **sem** a exceção de `values`. Argumentos
    `[declarationName, baseName, declarationName]`.
  - (3) `getter = getInherited2(enclosingClass, Name(lib, name))`, `setter = getInherited2(…, 'name=')`
    (`getInheritedMap2` = combinação mais específica dos candidatos dos supertipos diretos, sem o que a
    classe declara; para tipo de extensão sobre `interface.redeclared`, `inheritance_manager3.dart:215-230`).
    Métodos estáticos (`fragment.methods`, só os da unidade): `getter ?? setter` herdado → relata
    `[enclosingClass.displayName, name, inherited.enclosingElement3.displayName]` e `continue`.
    Acessores (`fragment.accessors`, inclui os sintéticos de campos): `inherited = getInherited2(name) ??
    getInherited2('name=')`; `accessor.isStatic && inherited != null` → relata e marca o nome em
    `conflictingDeclaredNames`. Mixin sem `on` tem restrição implícita `Object`
    (`_getInterfaceMixin` `inheritance_manager3.dart:930-952`), então `Object.runtimeType`/`toString`
    são herdados.
- **Posição:** (1) token do nome do campo/método; (2)(3) `atElement` → `nameOffset`/`nameLength` do
  elemento: para getter sintético de constante ou campo, o nome da constante/variável; para métodos e
  acessores explícitos, o nome. Campo estático não-final gera getter e setter → dois relatos idênticos
  (deduplicados).
- **Mensagem:** "Class '{0}' can't define static member '{1}' and have instance member '{2}.{1}' with the
  same name." — strings (sem alias). Em enum, `{0}` e `{2}` são **ambos** o nome do enum, mesmo quando o
  membro de instância vem de `Enum`/`Object`/mixin (`'E.runtimeType'`, `'E.index'`). Em classe/mixin/
  tipo de extensão com herdado, `{2}` é o **dono** do membro escolhido pelo `getInherited2` ('A.foo',
  'Object.runtimeType'); com membro local (caminho 1) `{2}` é a própria classe. Os dois caminhos podem
  relatar no mesmo token com mensagens diferentes (local e herdado).
- **Supressões e ordem:** nenhuma por erro de sintaxe. Caminho (1) não roda para enum (enum usa só o 2).
- **No DartForge:** hoje `analise/src/duplicatas.rs:735-765` (caminho 1, enum excluído, correto) e
  `analise/src/heranca.rs:15` `estatico_contra_super` (caminho 3 parcial: só classe/mixin, cadeia linear,
  ≤1 mixin, ≤1 interface, ≤1 `on`, e pula o nome que a classe também declara de instância).
  Classificação dos 55 FN: **44 enums** (24 `ConflictingStaticAndInstanceEnum__*`, 11
  `enum/enhanced_enums_error_test.dart`, 6 `executable_body/ExecutableBody__enum_static*`,
  2 `const_variable_augmentation/*enum*`, 1 `AugmentationOfDifferentDeclarationKind__dab386cb`),
  **9 tipos de extensão** (8 `ConflictingStaticAndInstanceExtensionTy_*`, 1
  `ConflictingInheritedMethodAndSetter__ex_f50c5959`), **2 mixins** (`Object.runtimeType`/`toString` via
  restrição implícita). Dos 44 enums, **9** estão em arquivos com `augment` (6 executable_body + 2
  const_variable_augmentation, nos quais a recuperação de `augment static int get foo => 0;` cria um campo
  de instância `static` do tipo `augment` e um **getter de instância** `foo`; e 1 enum duplicado `A`
  cujo `getMember2` usa a interface do 1º `A` — §0.1).

  **Código novo não validado** (working tree):
  - `crates/types/src/sobrescritas.rs:1508` `estaticos_de_enum` (ligado em `paridade/src/analise.rs:518`):
    para cada enum não-augment da biblioteca, junta constantes, campos estáticos e métodos/acessores
    estáticos; para cada um (exceto acessor `values`), procura `nome` e depois `nome_=` em
    `cx.na_interface(este, k, 0)` (declarado ou herdado de `Enum` via `superclasse()`
    `sobrescritas.rs:937-947`, mixins e interfaces, com `combineSignatures`) e relata com o nome do enum nos
    dois lugares. Fiel a `_checkEnumStatic`. Riscos: (a) os filtros "linha `augment `" e "nome repetido"
    (`:1525-1534`) deixam os 9 FN de augment/duplicado sem relato (§0.1); (b) `na_interface` devolve
    `None` quando `supertype_of`/`tipo_visto` falha (`?` em `:192,213,234`) — só gera FN, não FP;
    (c) nome privado: filtro `_` de outra biblioteca correto (`Name` com URI);
    (d) `na_interface` com candidatos de espécies diferentes devolve `None` (`:246-248`), igual ao
    `getMember2` (o `GetterMethodConflict` não entra no `map`), ok; (e) nenhuma checagem de
    `accessor.source == unidade` — irrelevante sem partes que dividam o enum.
  - `crates/types/src/sobrescritas.rs:1337` `membros_em_conflito`, agora emite `CONFLICTING_STATIC_AND_INSTANCE`
    para classe/mixin/tipo de extensão (método estático: `getter.or(setter)` herdado, `:1404-1415`;
    acessor estático: `herdado(nome).or(herdado(nome_=))`, `:1451-1458`) e, para mixin sem `on`, cai em
    `Object` quando `herdado` não acha (`:1375-1384`). Fiel ao caminho (3). Riscos:
    1. **Sobreposição com `analise::heranca`**: os dois relatam o caminho 3. Quando o dono escolhido
       difere (ex.: `class C extends A implements I` com `A.foo(int)` e `I.foo(num)`: o
       `combineSignatures` com `doTopMerge: false` escolhe o 1º *valid override*, `I.foo`; `heranca.rs`
       diz `A`), sai um FP com mensagem diferente no mesmo token. Recomendo remover o
       `CONFLICTING_STATIC_AND_INSTANCE` de `heranca.rs` depois de conferir no placar que nada se perde
       (atenção: `heranca.rs` não tem o filtro de `augment`, então hoje acerta casos em arquivos com
       `augment` que `membros_em_conflito` pula — tirar os filtros do §0.1 primeiro).
    2. O fallback para `Object` também alimenta `CONFLICTING_METHOD_AND_FIELD`/`FIELD_AND_METHOD`/
       `INHERITED_METHOD_AND_SETTER` em mixins sem `on` (ex.: `mixin M { int hashCode() => 0; }` →
       `conflicting_method_and_field` 'Object.hashCode'). É o que o analyzer faz, mas é FP novo possível
       se `herdado` já incluísse `Object` por outro caminho com outro dono.
    3. Tipo de extensão sem `implements`: `getInherited2` usa `redeclared` das interfaces declaradas
       (`_getInterfaceExtensionType`, `inheritance_manager3.dart:784-797`), **sem** `Object`. Conferir que
       `herdado` não devolve membros de `Object` para extension type sem `implements` (senão
       `static String toString()` num extension type vira FP).
    4. O filtro "linha `augment `"/nome repetido (`:1358-1364`) mantém os FN do §0.1.

##### `concrete_class_with_abstract_member` (perda 26: FN 26)
- **Emissão:** `_ClassVerifier._reportConcreteClassWithAbstractMember`
  (`src/error/inheritance_override.dart:809-845`), chamado do laço de `verify()` `:288-300`.
  (`src/fasta/error_converter.dart:63-68` também converte o código do CFE, sem argumentos — não ocorre
  no corpus.)
- **Condição exata:** classe (`ClassElement`) não abstrata ou enum; para cada `name` de
  `interface.map` acessível (`name.isAccessibleFor(libraryUri)`) sem `interface.implemented[name]`:
  ```
  for member in members (AST do fragmento, em ordem):
    MethodDeclaration: memberName = lexeme (+ '=' se setter); se == name → relata
    FieldDeclaration: para cada variável: se name == v → relata; se !isFinal e name == 'v=' → relata
  ```
  **Sem filtrar `static`** nem abstrato: o 1º membro AST cujo nome bate é o relatado, mesmo estático ou
  concreto (casos `executable_body/*static*`, em que a recuperação do `augment` criou um getter de
  instância abstrato `foo`, e o relato cai no `static int get foo` anterior; e o 2º `B` homônimo de
  `UnusedElement__…83bf2fbf` relata no seu `_m` concreto). Relatou → `continue` (o nome não entra em
  `non_abstract_class_inherits_abstract_member`). Enum → código `enum_with_abstract_member`.
  A classe só é "não abstrata" se `!isAbstract`; **`sealed` com o recurso desligado é descartado pelo
  AstBuilder** (`src/fasta/ast_builder.dart:261-268`: "Pretend that 'sealed' didn't occur"), então a classe
  fica concreta.
- **Posição:** `atNode(member)`: o `MethodDeclaration`/`FieldDeclaration` inteiro, do 1º token depois do
  comentário/metadados (comentário de documentação incluído no início — `inicio_com_documentacao` do
  DartForge) até o `;`/fim do corpo. Modificador que o parser descarta não entra (`const factory();` →
  começa em `factory`, `constructor_body/…e4682f2c:3:9`).
- **Mensagem:** "'{0}' must have a method body because '{1}' isn't abstract." — `{0}` = lexeme do nome
  (sem `=`; nome sintético vazio dá `''`, `factory/redirection3_cyclic_test.dart:21:26`), `{1}` =
  `classElement.name`.
- **Supressões:** só roda se `verify()` não parou antes (§0.2).
- **No DartForge:** `crates/types/src/sobrescritas.rs:1076-1108` (`membros_abstratos`). Divergências:
  1. **22 FN** em arquivos com `augment` (filtros `:1029-1042` e nome repetido em
     `clausulas.rs:1114`): `augmentation_return_type_mismatch`(2), `const_variable_augmentation`(3),
     `constructor_body`(2), `deprecated_factory_method`(2), `duplicate_definition`(4),
     `executable_body`(8), `unused_element`(1).
  2. O `find` em `:1079-1095` exige `!af.static_`/`!vl.static_` — o analyzer não filtra (necessário para
     os `executable_body/*static*`). Retirar o filtro de `static`.
  3. `sealed_class/sealed_class_syntax_disabled_error_test.dart:17,21` (2 FN, `@dart = 2.19`): o parser
     (`frontend/src/parser/declarations.rs:964-982`) põe `modifiers.sealed = true` mesmo com
     `exigir_versao("sealed-class", 3, 0)` falhando; `membros_abstratos` (`:1021`) e
     `superclasse_concreta` (`:1058`) tratam como abstrata. Sem o recurso, `sealed` deve sumir dos
     modificadores (idem `base`/`interface`/`final`/`mixin` para `class-modifiers`, que o AstBuilder
     também descarta em seguida, `ast_builder.dart:271+`).
  4. `factory/redirection3_cyclic_test.dart:21:26` (nome sintético vazio após recuperação de
     `factory C.foo() = C.bar();`) e `abstract/factory_constructor_test.dart:27:3` (`method();` numa
     classe `A2` que também tem `A2.make() {}`): o filtro `recuperado` (`:1029-1034`, "método cujo nome
     vem depois de `.`") descarta a classe inteira por causa do construtor `A2.make` lido como método? —
     conferir; e o método de nome vazio precisa existir no modelo com nome `""`.

##### `conflicting_generic_interfaces` (perda 26: FN 26; nada emitido hoje)
- **Emissão:** `ErrorVerifier._checkForConflictingGenerics` (`src/generated/error_verifier.dart:2652-2678`),
  chamado de `_checkClassInheritance` (`:1893`; classes com alguma cláusula, aliases de classe, enums com
  `implements`/`with`), `_checkMixinInheritance` (`:6108`; mixin com `on`/`implements`) e
  `visitExtensionTypeDeclaration` (`:825`, sempre).
- **Condição exata:** fragmento não-augmentation; `classHierarchy.errors(element)`
  (`src/dart/element/class_hierarchy.dart:41-103`): um `InterfacesMerger` recebe, em ordem,
  `append(supertype)`, `append(cada on)` (mixin), `append(cada interface)`, `append(cada mixin)`; `append(T)`
  adiciona `T` e **todas as interfaces implementadas de `T.element`** (recursivo, `implementedInterfaces`)
  substituídas pelos argumentos de `T`. Por elemento de classe: 1º tipo guarda `_singleType`; tipo igual
  (`==`) ignora; tipo diferente → `_currentResult = normalize(_singleType)` e
  `_merge(_currentResult, normalize(type))` = `topMerge` (`NNBD_TOP_MERGE`; para `Object`, `Object?` com
  `Object` dá `Object`, `:198-213`); se `topMerge` lança → erro `Incompatible(first=_currentResult,
  second=type)` e o coletor para (`:171-196`). Só o 1º erro por elemento. Em `_checkClassInheritance` só roda
  se `!_checkForExtendsDisallowedClass && !_checkForImplementsClauseErrorCodes &&
  !_checkForAllMixinErrorCodes && !_checkForNoGenerativeConstructorsInSuperclass` (`:1880-1883`); em
  mixin, se `!_checkForOnClauseErrorCodes && !_checkForImplementsClauseErrorCodes` (`:6098-6099`).
  Os tipos dos mixins sem argumentos são os **inferidos** (inferência de mixin do link: só a partir das
  restrições `on`; sem restrição o parâmetro fica no limite → `M0<dynamic>`).
- **Posição:** `node.name` (nome da classe/enum/mixin/tipo de extensão/alias).
- **Mensagem:** "The {0} '{1}' can't implement both '{2}' and '{3}' because the type arguments are
  different." — `{0}` = `_enclosingClass.kind.displayName` ("class" para classe e alias, "enum", "mixin",
  "extension type"), `{1}` = nome, `{2}` = `first.getDisplayString()` (o **acumulado normalizado**),
  `{3}` = `second.getDisplayString()` (o tipo **cru** que falhou); strings, sem alias. Ex.: `'I<int>'` e
  `'I<dynamic>'` (implements antes de mixin), `'A<Object>'`/`'A<Object?>'`, `'I<dynamic, List<dynamic>>'`.
- **Supressões:** acima; vários elementos em conflito → vários relatos no mesmo nome (ex.
  `mixin_declaration_inference_invalid_06_test.dart:28:7` relata `I` e `J`; a ordem segue a ordem de
  inserção dos elementos no merger).
- **No DartForge:** não existe. Lugar natural: `crates/types` (precisa de tipos instanciados e `topMerge`;
  `crates/types/src/hierarchy.rs` tem `supertype_of`). Classificação dos 26 FN: 16 diretos
  (10 `conflicting_generic_interfaces/*`, `generic/conflicting_generic_interfaces_simple_test`, 2
  `private_name_duplicate_interface_error_test`, `regress22976_test` com `'A<S>'`/`'A<T>'`, enum ×2,
  extension type, mixin `on`), **9 dependem da inferência de mixin** (`mixin_declaration_inference_invalid_03…11`:
  `with M0` sem argumentos → `M0<dynamic>`).

##### `inconsistent_inheritance_getter_and_method` (perda 26: FN 26; nada emitido)
- **Emissão:** `_ClassVerifier._reportInconsistentInheritance` (`src/error/inheritance_override.dart:847-863`),
  do laço `for conflict in interface.conflicts` em `verify()` `:215-217`.
- **Condição exata:** `interface = getInterface(declaration)` (`inheritance_manager3.dart:234`). Conflitos
  `GetterMethodConflict` nascem em:
  - `_findMostSpecificFromNamedCandidates` (`:526-566`): para cada nome com **>1 candidato** dos supertipos
    diretos, se há um candidato `GETTER` e um `METHOD` (o 1º de cada, na ordem dos candidatos,
    `_checkForGetterMethodConflict` `:498-519`) → conflito, **mesmo que a classe declare o nome** (o teste
    vem antes de `map.containsKey`). Candidatos de classe (`_getInterfaceClass` `:568-748`): superclasse
    (substituída), cada mixin **substitui** o candidato quando o próprio mixin declara o nome, depois cada
    interface (`:676-682`). Para mixin (`_getInterfaceMixin` `:930-989`): `superConflicts` (restrições `on`)
    + `interfaceConflicts` (restrições + interfaces) — o mesmo conflito pode sair duas vezes (dedup).
  - Mixins aplicados (`:632-644`): se o mixin declara o nome e o candidato corrente (da superclasse/mixin
    anterior) tem outra espécie → `GetterMethodConflict(getter: o que é getter, method: o outro)`; esses
    vêm **depois** dos conflitos principais (`:727-731`).
- **Posição:** `classNameToken` (nome da classe/alias/enum/mixin), comprimento do nome.
- **Mensagem:** "'{0}' is inherited as a getter (from '{1}') and also a method (from '{2}')." —
  `{0}` = `name.name`, `{1}` = `getter.enclosingElement3.name`, `{2}` = `method.enclosingElement3.name`
  (classe declarante; campo conta como getter).
- **Supressões:** só se `verify()` passou de `_checkDirectSuperTypes`/Enum/recursão. Conflito → o nome não
  entra no `map` (sem `non_abstract…`, sem `invalid_implementation_override` para ele).
- **No DartForge:** inexistente. `sobrescritas.rs:241-259` (`na_interface_ex`) detecta "espécies
  diferentes" e só devolve `None`. Implementar o cálculo da interface como `_getInterfaceClass`/
  `_getInterfaceMixin` com a lista `conflicts` e relatar no nome da classe. FN: 26, todos em
  `inconsistent_inheritance_getter_and_method/*`; **6** deles são a 2ª cópia no nome da declaração
  homônima `augment class C` (§0.1: `…136ab991:11:15`, `…647dda10:13:24`, `…c26fdd00:11:15`,
  `…c2a9fc2d:15:15`, `…cf063203:11:15`), 2 são mixins aplicados (`…4c20b7f5` relata S/M1 e M2/M1).

##### `inconsistent_inheritance` (perda 18: FN 18; nada emitido)
- **Emissão:** `_reportInconsistentInheritance` (`src/error/inheritance_override.dart:864-875`), mesmo laço.
- **Condição exata:** `CandidatesConflict` de `combineSignatures` (`inheritance_manager3.dart:79-126`):
  nome **não declarado** pela classe (`map.containsKey` antes), >1 candidato, nenhum candidato cujo tipo
  seja subtipo do tipo de **todos** os outros (`isSubtypeOf(validOverrideType, candidate.type)`; tipo
  com `covariant` não muda nada aqui). Os conflitos do merge superclasse+mixin (`:651-662`) são
  **descartados** (o retorno não é usado). Tipos de extensão não passam por aqui (não estão no
  `verifyUnit`).
- **Posição:** `classNameToken`.
- **Mensagem:** "Superinterfaces don't have a valid override for '{0}': {1}." — `{1}` =
  `candidates.map((c) => '${c.enclosingElement3.name}.${name} (${c.type.getDisplayString()})').join(', ')`,
  na ordem dos candidatos (superclasse, mixins, interfaces na ordem escrita; em mixin: `on` em ordem);
  o tipo é o do membro **substituído** pelo supertipo (`ExecutableMember.from2`), ex.
  `A.m (void Function(int)), B.m (void Function(String))`; `class C extends B implements A` dá `B.m …, A.m …`.
- **Supressões:** como acima; mixin `on A, B` gera o conflito em `superConflicts` e em
  `interfaceConflicts` (mesmo texto → dedup).
- **No DartForge:** inexistente; mesmo lugar do anterior. 18 FN, todos em `inconsistent_inheritance/*`
  (classe, mixin `on`/`implements`, enum).

##### `override_on_non_overriding_member` (perda 21: FN 21; nada emitido)
- **Emissão:** `OverrideVerifier` (`src/error/override_verifier.dart:15-107`), em `_computeWarnings`
  (`src/dart/analysis/library_analyzer.dart:483`). Códigos únicos `WarningCode.OVERRIDE_ON_NON_OVERRIDING_FIELD/
  GETTER/METHOD/SETTER`, todos com nome `override_on_non_overriding_member`.
- **Condição exata:** `_currentClass` é setado só em `visitClassDeclaration`, `visitEnumDeclaration`,
  `visitMixinDeclaration` (`:32-95`); em extensão e tipo de extensão fica `null`. `_isOverride(m)` =
  `_currentClass != null && getOverridden2(currentClass.augmented.declaration, Name(lib, m.name)) != null`
  (`interface.overridden` = candidatos dos supertipos diretos, `inheritance_manager3.dart:409-412`;
  não olha `static`). Campo (`visitFieldDeclaration` `:47-63`): para cada variável com `@override`
  (`fieldElement.hasOverride`), relata se nem o getter nem o setter (se houver) é override. Método/acessor
  (`:66-88`): `element.hasOverride && !_isOverride(element)` → METHOD / GETTER / SETTER conforme o elemento.
  Logo: **toda** `@override` em extensão (e em tipo de extensão) relata; `static` com `@override` relata
  se nenhum supertipo tem o nome.
- **Posição:** `field.name` / `node.name` (token do nome; nome sintético vazio de recuperação também,
  comprimento 0: `OverrideOnNonOverridingMethod__class_fi_a44d5436.dart:5:15`). Construtor primário
  (3.13, `OverrideOnNonOverridingField__class_pri_fb057801.dart:1:29`): o nome do parâmetro declarante.
- **Mensagem:** FIELD "The field doesn't override an inherited getter or setter."; GETTER "The getter
  doesn't override an inherited getter."; METHOD "The method doesn't override an inherited method.";
  SETTER "The setter doesn't override an inherited setter.". Sem argumentos.
- **Supressões:** `hasOverride` exige a anotação resolvida ao `override` de `dart:core`. Função de topo não é
  visitada (`OverrideOnNonOverridingGetter__topLevel` não relata).
- **No DartForge:** inexistente. Pode ir em `crates/types` (usa os candidatos dos supertipos diretos,
  `na_interface` por supertipo direto) após o resto. 21 FN em `override_on_non_overriding_{field,getter,
  method,setter}/*` (classe, enum, mixin, extensão, estáticos, construtor primário 3.13).

##### `supertype_expands_to_type_parameter` (perda 21: FN 21; nada emitido)
- **Emissão:** `NamedTypeResolver._verifyTypeAliasForContext` (`src/dart/resolver/named_type_resolver.dart:420-476`),
  na resolução do tipo (`ResolutionVisitor`), códigos únicos `EXTENDS_/IMPLEMENTS_/MIXIN_ON_/MIXIN_OF_TYPE_ALIAS_EXPANDS_TO_TYPE_PARAMETER`.
- **Condição exata:** o `NamedType` resolve para um `TypeAliasElement` com `aliasedType is TypeParameterType`
  (inclui `X?` e aliases que se expandem a outro alias que dá `X`), e o pai é `ExtendsClause` /
  `ImplementsClause` / `MixinOnClause` / `WithClause` (`:451-474`). Retorna `InvalidType` e marca
  `hasErrorReported` → a cláusula não gera `*_non_class` etc. (`resolution_visitor.dart:1660`).
- **Posição:** `_getErrorRange` (`:653-666`): do prefixo de import (se houver) até o fim de `name2`, **sem**
  os argumentos de tipo (`T<A>` → só `T`).
- **Mensagem (sem argumentos):** extends "A type alias that expands to a type parameter can't be used as a
  superclass."; implements "… can't be implemented."; on "… can't be used as a superclass constraint.";
  with "… can't be mixed in.".
- **No DartForge:** `crates/analise/src/clausulas.rs:115-122` (`alvo`) e `:310-316` (`resolver`) já
  reconhecem o caso (só o alias direto com nome de parâmetro) e devolvem `Desconhecido`/`Ignorar` com o
  comentário "outro verificador" — mas ninguém emite. Emitir ali (em `verificar`, ao lado dos
  `*_non_class`) com o span do nome. 21 FN: 9 em `*_type_alias_expands_to_type_parameter/*`, 12 em
  `nonfunction_type_aliases/{generic_,}usage_type_variable_error_test.dart`.

##### `conflicting_inherited_method_and_setter` (perda 11: FN 11)
- **Emissão:** `ErrorVerifier._checkForConflictingClassMembers` (`src/generated/error_verifier.dart:2478-2529`).
- **Condição exata:** `inherited = getInheritedMap2(enclosingClass)`; para cada entrada `MethodElement`
  cujo nome não está em `conflictingDeclaredNames` (preenchido só por acessores estáticos com herdado e por
  `conflicting_field_and_method`; em tipo de extensão nunca por este último), se `inherited[name=]` é
  `PropertyAccessorElement` → relata. Em tipo de extensão `getInheritedMap2` usa `interface.redeclared`
  (todos os candidatos das interfaces, **antes** da preclusão pelos membros declarados,
  `inheritance_manager3.dart:806,867`), então relata mesmo que a classe declare `set foo`.
- **Posição:** `atElement(enclosingClass)` → nome da classe/mixin/tipo de extensão.
- **Mensagem:** "The {0} '{1}' can't inherit both a method and a setter named '{2}'." — `{0}` =
  `kind.displayName` ("class", "mixin", "extension type"), `{1}` nome, `{2}` nome do método. Com duas
  mensagens de contexto (não comparadas).
- **No DartForge:** `crates/types/src/sobrescritas.rs:1473-1496` (`membros_em_conflito`). Os 11 FN são
  **todos tipos de extensão** (`conflicting_inherited_method_and_setter/ConflictingInheritedMethodAndSetter__ex_*`).
  Causa provável (confirmar com teste): `chaves_da_hierarquia` (`:971-986`) usa
  `outline.hierarchy.get(c).supertypes`, e/ou `na_interface_ex` aborta com `?` em `visto(...)` (`:234`)
  quando o supertipo é um tipo de extensão/classe implementada por tipo de extensão — então `foo` nunca é
  examinado. Para tipo de extensão, as chaves devem vir das interfaces declaradas (como `redeclared`).

##### `getter_not_subtype_setter_types` (perda 11: FN 11)
- **Emissão:** `GetterSetterTypesVerifier` (`src/error/getter_setter_types_verifier.dart`):
  `checkInterface` (`:38-86`, de `InheritanceOverrideVerifier.verify` `:279` para classe/alias/enum/mixin,
  e de `checkExtensionType` `:33` em `error_verifier.dart:842-845`); `checkStaticAccessors` (`:88-94`,
  classe `error_verifier.dart:522-525`, enum `:719-722`, tipo de extensão via `checkExtensionType`);
  `checkExtension` (`:25-31`, extensão `error_verifier.dart:776-779`: **todos** os getters, estáticos e de
  instância).
- **Condição exata:** `checkInterface`: para cada nome acessível de `interface.map` cujo membro é `GETTER`,
  setter `interface.map['name=']` com exatamente 1 parâmetro; `!isSubtypeOf(getter.returnType,
  setter.parameters[0].type)` → relata. `_checkLocalGetter`: getter com `correspondingSetter` (mesmo
  recipiente, mesmo `isStatic`), tipos idem. Getters estáticos incluem os **sintéticos**: constantes de
  enum (tipo = o enum) e `values` (tipo `List<E>`).
- **Posição:** `checkInterface`: `errorElement` = getter se declarado nesta classe (exceto o getter de
  representação de tipo de extensão → o setter), senão o setter se declarado aqui, senão a própria classe
  (nome). `_checkLocalGetter`: o getter; getter sintético de constante → nome da constante; getter `values`
  → **o nome do enum** (`_GetterNotSubtypeSetterTypes__enum_stat_013ef1dc.dart:1:6`).
- **Mensagem:** "The return type of getter '{0}' is '{1}' which isn't a subtype of the type '{2}' of its
  setter '{3}'." — `{0}`/`{3}`: `displayName`, qualificado `Dono.nome` quando o dono não é a classe
  (comparação por `==` de elemento, §0.1); `{1}`/`{2}` DartType (exibição com alias preferido).
- **No DartForge:** `crates/types/src/sobrescritas.rs:674-847`. FN por causa:
  - enum estático sintético (constante `foo` com `static set foo(int)`, `values`, `enhanced_enums_error_test:217:3`):
    as constantes e `values` não são funções no modelo → não entram nos `grupos` (`:719-755`). 3 FN.
  - enum de instância contra `Enum.index` (`…enum_inst_dd376939:3:7`, posição no setter, `'Enum.index'`):
    conferir se enums entram em `classes`/`hierarchy` com `Enum`. 1 FN.
  - extensão com campo estático + setter estático (`…extension_ee42a5ad:2:20`): o acessor implícito do
    campo de extensão provavelmente não tem `f.extension`. 1 FN.
  - tipo de extensão: representação + `set it` (`…extension_20069aeb:2:12`) e interface com `implements`
    (`…extension_bc351d4a:6:11`, `'A.foo'`): a parte de interface (`:777`) só percorre `classes`, que não
    inclui tipos de extensão (`checkExtensionType`). 2 FN.
  - tipo do campo inferido por sobrescrita (`DifferentInheritedGetterAndSetterTypes__285b5fce.dart:7:9`,
    `final foo = 0` sobre `num get foo` → tipo `num`): `inferencia_confiavel` (`:109-123`) recusa
    `FunctionRef::None`. 1 FN.
  - `augment` (2 `augmentation_variable_different_getter_setter_types`, 1 `executable_body`): §0.1. 3 FN.
  - Observação: `depende_da_linha` (`paridade/src/analise.rs:831`) descarta este código em linhas com erro
    de recuperação do parser — no analyzer o código sai normalmente; conferir se o filtro ainda é necessário.

##### `mixin_application_no_concrete_super_invoked_member` (perda 11: FN 11; nada emitido)
- **Emissão:** `ErrorVerifier._checkForMixinSuperInvokedMembers` (`src/generated/error_verifier.dart:4428-4489`),
  chamado de `_checkForAllMixinErrorCodes` (`:1974-2017`) para cada mixin `MixinElement` da cláusula `with`
  quando `_checkForMixinSuperclassConstraints` não relatou (`else if`, `:2000-2005`).
- **Condição exata:** `superInvokedNames` do mixin (coletados no link,
  `src/summary2/library_builder.dart:281-298` com `MixinSuperInvokedNamesCollector`
  `src/dart/ast/mixin_super_invoked_names.dart:10-70`: só **corpos de métodos/acessores** do mixin;
  `super.m()` → `m`; `super.x` → `x` e/ou `x=` conforme contexto de leitura/escrita; `super[i]` → `[]`/`[]=`;
  `super op e` → `op`; `-super` → `unary-`; `~super` → `~`; conjunto em ordem de inserção). Para cada nome
  em ordem: `superMember = getMember2(enclosingClass, Name(mixinLib, name), forMixinIndex: i, concrete: true,
  forSuper: true)` = `superImplemented[i]` (implementados concretos da superclasse + mixins **anteriores**;
  `i` conta só mixins que são `InterfaceType`). Nulo → relata e **retorna** (um relato por mixin).
- **Posição:** `atNode(mixinName)` — o `NamedType` inteiro (com argumentos de tipo).
- **Mensagem:** MEMBER "The class doesn't have a concrete implementation of the super-invoked member
  '{0}'."; SETTER (nome termina em `=`) "… super-invoked setter '{0}'." com `{0}` sem o `=`.
- **Supressões:** `_checkForAllMixinErrorCodes` devolve `true` → `_checkClassInheritance` não roda
  `conflicting_generic_interfaces`, `class_used_as_mixin` etc. Mixin com restrição não satisfeita relata
  `mixin_application_not_implemented_interface` e não chega aqui.
- **No DartForge:** inexistente. `crates/analise/src/clausulas.rs:741-750` marca `incerta` todo mixin cujo
  texto contém `super` — é exatamente o caso deste código. Precisa: coletor de nomes (sintático, possível em
  `analise`) e o "implementado concreto até o mixin i" (`implementado` em `sobrescritas.rs:909`, com corte
  por índice de mixin). 11 FN (8 no diretório do código, `mixin/illegal_super_use_test`, 2
  `mixin_declaration_invalid_override*`).

##### `mixin_application_not_implemented_interface` (perda 11: FN 11)
- **Emissão:** `_checkForMixinSuperclassConstraints` (`src/generated/error_verifier.dart:4393-4425`).
- **Condição exata:** para cada restrição `constraint` de `mixinType.superclassConstraints` (substituída
  pelos argumentos **inferidos** do mixin): `superType = _enclosingClass.supertype` sem `?`;
  satisfeita se `isSubtypeOf(superType, constraint)` ou algum `mixins[j]` (j < índice do mixin) é subtipo.
  A 1ª não satisfeita relata e retorna `true`.
- **Posição:** `mixinName.name2` (só o identificador, sem prefixo nem argumentos).
- **Mensagem:** "'{0}' can't be mixed onto '{1}' because '{1}' doesn't implement '{2}'." — DartType:
  `{0}` = `mixinName.type` (com args inferidos: `'M<dynamic>'`, `'M0<int>'`), `{1}` = supertipo da classe
  (`'Object'`, `'A<double>'`, `'B<X, Y>'`), `{2}` = restrição substituída (`'A<int>'`, `'I<int>'`).
- **No DartForge:** `crates/analise/src/clausulas.rs:923-1015` (`restricoes_satisfeitas`/`nao_implementa`),
  nominal. FN: **9** genéricos (restrição com argumentos ou mixin genérico/inferido → `None` em `:946-958`:
  `…65fb1b04`, `…66b205b0`, `…71767084`, `…99c7eb8b`, `…d6fae84e`, `mixin_declaration_inference_invalid_00/01/02`,
  `regress32353_2_test`); **2** mixins cujo corpo contém `super` (`incerta` em `clausulas.rs:741-750`:
  `…23fc610b`, `MixinApplicationNoConcreteSuperInvokedM_f0c49557` — este também com `augment class X with M2`
  homônimo). Precisa subtipagem com argumentos e inferência de mixin (`crates/types`).

##### `no_combined_super_signature` (perda 11: FN 11; nada emitido)
- **Emissão:** `_ClassVerifier._reportNoCombinedSuperSignature` (`src/error/inheritance_override.dart:953-971`),
  chamado para cada `MethodDeclaration` em `verify()` `:259-263`. O erro é gravado na inferência de topo:
  `InstanceMemberInferrer._inferExecutable` (`src/task/strong_mode.dart:403-457`).
- **Condição exata:** método de instância não sintético (só `MethodElementImpl`: métodos e operadores, não
  getters/setters); `overridden = getOverridden2(classe, name)` não nulo e todos da mesma espécie
  (`_allSameElementKind`); algum tipo omitido (retorno implícito ou parâmetro sem tipo); `combineSignatures`
  dos candidatos falha → `typeInferenceError(overrideNoCombinedSuperSignature, [explicação])`. Explicação:
  se houve exatamente 1 conflito `CandidatesConflict`, `'Classe.nome (tipo)'` por candidato com `, `; senão
  `'<unknown>'`. Os tipos omitidos viram `dynamic`.
- **Posição:** `node.name` (nome do método/operador).
- **Mensagem:** "Can't infer missing types in '{0}' from overridden methods: {1}." — `{0}` =
  `classElement.name`, `{1}` = explicação (tipos via `getDisplayString()`, ex.
  `A.foo (void Function(int)), B.foo (void Function(double))`, `IOptx.foo (int Function({int x}))`).
- **Supressões:** relatado → o método pula `_checkDeclaredMember` (sem `invalid_override` para ele, `:261-263`).
- **No DartForge:** inexistente; a inferência de sobrescrita de `types` segue o primeiro supertipo
  (`sobrescritas.rs:105-123` declara a limitação). 11 FN (3 no diretório do código,
  7 `class/override_inference_error_test`, 1 `inference/inconsistent_inheritance_test`).

##### `non_abstract_class_inherits_abstract_member` (perda 11: FN 11)
- **Emissão:** `_reportInheritedAbstractMembers` (`src/error/inheritance_override.dart:881-951`), de
  `verify()` `:284-349`.
- **Condição exata:** classe não abstrata ou enum; nome acessível de `interface.map` sem
  `interface.implemented[name]`; não pego por `_reportConcreteClassWithAbstractMember`; não
  `_isNotImplementedInConcreteSuperClass` (`:793-803`: superclasse **declarada** concreta cuja interface tem
  o nome); em enum, nem `values`/`values=`. `implemented` inclui os encaminhadores de `noSuchMethod` quando o
  `noSuchMethod` implementado não é o de `Object` (`inheritance_manager3.dart:705-723`) — `noSuchMethod`
  **abstrato** declarado não conta (o implementado continua o de `Object`).
- **Posição:** `classNameToken`.
- **Mensagem:** ONE..FIVE_PLUS, descrições `'[getter |setter ]Dono.nome'` ordenadas (`descriptions.sort()`),
  FIVE_PLUS com `{4}` = total − 4.
- **No DartForge:** `crates/types/src/sobrescritas.rs:1110-1176`. FN: **10** em arquivos com declaração
  homônima `augment class/enum` (§0.1: o 1º **e** o 2º relatam — `…9a2979d9:5:7,9:15`,
  `…9aac33d1:5:6,11:14`, `InconsistentInheritance__class_augmentW_28c73dd6:9:7,13:15`,
  `InconsistentInheritance__enum_returnTyp_221d07c1:9:6,13:14`, `constructor_body/…e4682f2c:6:15`,
  `unused_element/…83bf2fbf:4:7`); **1** `NonAbstractClassInheritsAbstractMember__ae339b2b.dart:4:7`
  (`class C implements I { noSuchMethod(v); }`, `I.m(p)`): pela leitura `implementado(noSuchMethod)` cai no de
  `Object` (o de `C` é abstrato) e não deveria pular; causa não identificada — reproduzir com teste.

##### `recursive_interface_inheritance` (perda 10: FN 4, msg 6)
- **Emissão:** `_checkForRecursiveInterfaceInheritance` (`src/error/inheritance_override.dart:582-658`),
  `verify()` `:207`. Códigos únicos `RECURSIVE_INTERFACE_INHERITANCE` e `_EXTENDS/_IMPLEMENTS/_ON/_WITH`
  (`_getRecursiveErrorCode` `:767-789`).
- **Condição exata:** DFS a partir da classe por `supertype`, `mixins`, restrições `on`, `interfaces`;
  `path` sem repetir (`path.indexOf(element) > 0` corta). Ao reencontrar `classElement == element`
  (**igualdade por localização**, §0.1): `size > 1` → genérico com caminho; `size == 1` → código direto de
  `_getRecursiveErrorCode(element)`, que olha o `supertype`/`on`/`mixins` **de `element`** (o reencontrado)
  apontando para `classElement`; nenhum → `_IMPLEMENTS`.
- **Posição:** `atElement(classElement)` → nome da classe que está sendo verificada.
- **Mensagem:** genérico "'{0}' can't be a superinterface of itself: {1}." com `{1}` =
  `path[0..].displayName` + o elemento final, separados por `, ` (3.6.2: `A, C, B, A` — começa e termina na
  própria classe); diretos "'{0}' can't extend itself." / "implement itself." / "use itself as a superclass
  constraint." / "use itself as a mixin.".
- **No DartForge:** `crates/analise/src/clausulas.rs:1340-1370`. Divergências:
  - **4 FN** — `augment class A extends/implements/with A {}` e `augment mixin A on A {}` homônimos: a
    referência `A` resolve para o 1º `A`, que é "igual" ao 2º → base, e `_getRecursiveErrorCode(1º A)` não
    acha o 2º em supertype/on/mixins do 1º → **sempre `_IMPLEMENTS`** ("'A' can't implement itself.") no
    nome do 2º. Implementar: numa declaração homônima, supertipo direto que resolve para outra declaração
    de mesmo nome → `RECURSIVE_INTERFACE_INHERITANCE_IMPLEMENTS` no nome dela (e `verify()` para).
  - **6 msg** — `primary_constructors/cycle_error_test.dart` (só sintaxe nova, oráculo 3.13.4): a lista do
    3.13 não repete o elemento inicial e é **a mesma para todos os membros do ciclo**
    (`'A' … itself: B, C, A.` para A, B e C; `SuperB, SuperC, SuperA` para os três Super*), na ordem
    "cada um é estendido pelo seguinte" começando pela 2ª classe declarada do ciclo (inferido do corpus;
    3.13 não disponível localmente). Nas bibliotecas de `libs_com_sintaxe_nova`, gerar esse formato.

##### `invalid_override` (perda 9: FN 9)
- **Emissão:** `CorrectOverrideHelper.verify` (`src/error/correct_override.dart:46-67`) chamado de
  `_checkDeclaredMember` (`src/error/inheritance_override.dart:357-404`, código `INVALID_OVERRIDE` ou
  `INVALID_OVERRIDE_SETTER` `:389-395`); também `CovariantParametersVerifier` (`correct_override.dart:112-140`).
- **Condição exata:** membro de instância declarado (campo → getter e setter) contra
  `getMember(superType, name, forMixinIndex)` de cada supertipo direto (superclasse, `on`, cada mixin, cada
  interface — `directSuperInterfaces` `:219-241`), mesma espécie; `!isSubtypeOf(thisType', superType)` com
  parâmetros `covariant` (declarados **ou herdados**) trocados por `Object?`. Membros de mixins aplicados são
  conferidos contra superclasse + mixins anteriores (`:233-239`).
- **Posição:** o nome do membro (campo: nome da variável; nos mixins: o `NamedType` do mixin).
- **Mensagem:** "'{1}.{0}' ('{2}') isn't a valid override of '{3}.{0}' ('{4}')." / "The setter …";
  tipos de função com `getDisplayString` (genéricos `<T>`).
- **No DartForge:** `crates/types/src/sobrescritas.rs:313-466`. 9 FN:
  - tipos omitidos com mais de um supertipo declarante (`InvalidOverride__method_normalParamType_27ff3ba9`,
    `…optionalParamTy_1088b290`: `m(String n)` sem retorno → `dynamic` pela combinação; `inferencia_confiavel`
    recusa, `:117-121`): 2;
  - campo com tipo inferido de sobrescrita (`DifferentInheritedGetterAndSetterTypes__6dc87d36.dart:9:7`,
    `var foo = 0` → setter `int`): 1;
  - operador genérico `==<T>(a)` (`operator/invalid_operators_test.dart:490:12`, tipo exibido
    `'dynamic Function<T>(dynamic)'`): `inferencia_confiavel` exige `type_params` vazios; 1;
  - variância declarada (`variance/variance_in_subtyping_error_test`, `variance_inout_subtyping_error_test`,
    experimento `variance`, `class Contravariant<in T>`): subtipagem sem variância de declaração; 4;
  - `augment` (`InvalidOverride__class_augment_method_c_cc5d5d9d`): 1.

##### `conflicting_field_and_method` (perda 7: FN 7)
- **Emissão:** classe/mixin: `_checkForConflictingClassMembers` (`error_verifier.dart:2439-2475`,
  acessor não estático com herdado `MethodElement`); **enum**: `_checkEnum` (`duplicate_definition_verifier.dart:602-622`,
  `fragment.accessors` de instância da unidade contra `_getInheritedMember` `:813-825` = `getInherited2(n) ??
  getInherited2(n=)`).
- **Posição:** `atElement(accessor)` → nome do campo/getter/setter.
- **Mensagem:** "Class '{0}' can't define field '{1}' and have method '{2}.{1}' with the same name." —
  `{0}` displayName da classe/enum, `{2}` dono do método herdado.
- **No DartForge:** `sobrescritas.rs:1459-1470`, só `K::Class | K::Mixin | K::ExtensionType` (`:1350`).
  FN: **5 enums** (`ConflictingFieldAndMethod__enum_inMixin_{field,getter,setter}`,
  `ConflictingMethodAndField__enum_inMixin_field`, `enhanced_enums_error_test:143:11`) — falta a parte de
  `_checkEnum` (pode entrar em `estaticos_de_enum` ou numa função irmã); **2** homônimos `augment class B`
  /`augment mixin B` (§0.1).

##### `enum_with_abstract_member` (perda 7: FN 7)
- **Emissão/condição/posição:** idênticas a `concrete_class_with_abstract_member` com `classElement is
  EnumElement` (`inheritance_override.dart:815-817`). Mensagem "'{0}' must have a method body because '{1}'
  is an enum.".
- **No DartForge:** `sobrescritas.rs:1106`. Os 7 FN estão em arquivos com `augment`
  (`const_variable_augmentation`, `executable_body/*enum*`) — filtro §0.1 e filtro de `static` (os
  relatos caem em membros **estáticos**, ex. `ExecutableBody__enum_staticGetter_noBod_d0836f15.dart:3:3`).

##### `conflicting_method_and_field` (perda 6: FN 6)
- **Emissão:** classe/mixin `_checkForConflictingClassMembers` (`error_verifier.dart:2373-2437`: método de
  instância com getter/setter herdado `PropertyAccessorElement`; não em tipo de extensão); **enum**
  `_checkEnum` (`duplicate_definition_verifier.dart:624-644`: `fragment.methods` de instância contra
  `_getInheritedMember`).
- **Posição:** nome do método. **Mensagem:** "Class '{0}' can't define method '{1}' and have field '{2}.{1}'
  with the same name.".
- **No DartForge:** `sobrescritas.rs:1419-1425` (sem enums). FN: **4 enums** (`…enum_inMixin_getter/setter`,
  `enhanced_enums_error_test:278:7,286:7`), **2** homônimos `augment class B` (§0.1).

##### `conflicting_constructor_and_static_member` (perda 5: FN 5)
- **Emissão:** `_checkConflictingConstructorAndStatic` (`duplicate_definition_verifier.dart:463-496`), do fim
  de `_checkClassMembers` (`:402-408`).
- **Condição exata:** para cada construtor do elemento (inclui o **construtor primário** do tipo de
  extensão), `staticGetters[name] ?? staticSetters[name]`: acessor sintético (de campo) → `_FIELD`; getter
  → `_GETTER`; setter → `_SETTER`; método → `_METHOD`.
- **Posição:** `atElement(constructor)` → nome do construtor (no primário `A.foo(...)`, o `foo`).
- **Mensagem:** "'{0}' can't be used to name both a constructor and a static field/getter/setter/method in
  this class.".
- **No DartForge:** `crates/analise/src/duplicatas.rs:711-726` usa só `nomeados` (construtores membros,
  `:656-672`). O primário do tipo de extensão é inserido em `ctx.construtores` (`:617`) mas **não** em
  `nomeados` → 4 FN (`ConflictingConstructorAndStatic{Field,Method}__e_*`). 1 FN em sintaxe nova
  (`primary_constructors/header/static_member_conflict_error_test.dart:59:20`, construtor primário de classe
  3.13).

##### `invalid_implementation_override` (perda 5: FN 5)
- **Emissão:** `verify()` `inheritance_override.dart:314-345` (`CorrectOverrideHelper` com `thisMember =
  implemented[name]`, `superMember = interface.map[name]`, mesma espécie, `errorNode: classNameToken`).
- **Posição:** nome da classe. **Mensagem:** "'{1}.{0}' ('{2}') isn't a valid concrete implementation of
  '{3}.{0}' ('{4}')." / "The setter …".
- **No DartForge:** `sobrescritas.rs:1126-1157`. FN: 4 com `augment` (3 `augmentation_type_parameter_bound`,
  1 `executable_body`), e `enum/enhanced_enums_error_test.dart:627:6` (`enum NSMImplementsNeverIndex
  implements NeverIndexGetter` com `noSuchMethod`: `'Enum.index' ('int Function()')` contra
  `'NeverIndexGetter.index' ('Never Function()')`). Causa: `membros_abstratos` pula a classe **inteira**
  quando há `noSuchMethod` não-`Object` (`:1044-1050`); no analyzer os encaminhadores só preenchem nomes
  **sem** implementação, e `index` tem a de `Enum` → a comparação continua. Trocar o `continue` por "nome
  sem implementação conta como implementado (encaminhador)".

##### `concrete_class_has_enum_superinterface` (perda 2: FN 2)
- **Emissão:** direto: `_checkDirectSuperTypeNode` (`inheritance_override.dart:459-481`, no `NamedType`);
  transitivo: `verify()` `:196-205` (`declaration is ClassElement && !isAbstract && implementsDartCoreEnum`,
  `allSupertypes.any(isDartCoreEnum)` `:166-167`), no `classNameToken`, e `verify()` para.
- **No DartForge:** `crates/analise/src/clausulas.rs:1145-1153` (`verificador_de_heranca_prossegue`) calcula o
  caso transitivo só para tirar a classe da lista, sem relatar; relatar ali no nome da classe (inclui alias de
  classe `class B = Object with M implements A`). 2 FN: `NonAbstractClassHasEnumSuperinterface___0aaf3cfa:3:7`,
  `…fc47f638:2:7`.

##### `conflicting_type_variable_and_member` (perda 2: FN 2)
- **Emissão:** `_checkForConflictingExtensionTypeTypeVariableErrorCodes` (`error_verifier.dart:2593-2623`:
  `getNamedConstructor(name)` — inclui o primário nomeado — ou método/getter/setter) e
  `_checkForConflictingEnumTypeVariableErrorCodes` (`:2567-2591`: `getMethod/getGetter/getSetter(name)`,
  incluindo o getter sintético **`values`**).
- **Posição:** `atElement(typeParameter)` → nome do parâmetro de tipo.
- **No DartForge:** `crates/analise/src/membros.rs:855-880`. FN: `extension type A<T>.T(int it)` (nome do
  construtor primário não considerado) e `enum/enhanced_enums_error_test.dart:232:34` (`enum E<values>`,
  falta o `values` implícito).

##### `class_used_as_mixin` (perda 1: FN 1)
- **Emissão:** `_checkForClassUsedAsMixin` (`error_verifier.dart:2336-2356`), em `_checkClassInheritance`.
  Classe (não `mixin class`) de biblioteca com `class_modifiers`, `atNode(withMixin)`.
- **No DartForge:** `clausulas.rs:731-736` marca `incerta` quando `m == id && duplicada`. FN
  `RecursiveInterfaceInheritanceWith__clas_e91be616.dart:2:22` (`augment class A with A`): `A` resolve para o
  1º `A` (classe) → relata, junto do `recursive_interface_inheritance` no 2º `A` (§0.1).

##### `mixin_super_class_constraint_non_interface` (perda 1: FN 1)
- **Emissão:** `ResolutionVisitor` (`src/dart/resolver/resolution_visitor.dart:1688-1715`), cláusula `on`
  com tipo que não é classe/mixin; posição do prefixo/`name2` (`:1708-1714`).
- **No DartForge:** `mixin M on void {}`: o parser do analyzer recupera `void` como `NamedType` (relata
  `expected_type_name` e este código, ambos em `void`, 1:12 len 4). `clausulas.rs:1240-1270` só trata
  `TypeKind::Named`; tratar `TypeKind::Void` em `on` como não-classe com o span do `void`.

##### `mixin_application_concrete_super_invoked_member_type` (perda 2: FN 2; nada emitido)
- **Emissão:** `_checkForMixinSuperInvokedMembers` (`error_verifier.dart:4465-4485`): para nome
  super-invocado com `superMember` concreto achado, `mixinMember = getMember(mixinType, name, forSuper: true)`
  (o membro da interface das restrições do mixin); se `!isCorrectOverrideOf(superMember → mixinMember)` relata
  e retorna.
- **Posição:** o `NamedType` do mixin. **Mensagem:** "The super-invoked member '{0}' has the type '{1}', and
  the concrete member in the class has the type '{2}'." — `{1}` = tipo do membro do mixin (ex.
  `'void Function([int?])'`), `{2}` = tipo do concreto (`'void Function(int?)'`), DartType.
- **No DartForge:** inexistente; mesmo trabalho do `no_concrete_super_invoked_member`.

##### `deprecated_subtype_of_function` (perda 1: FN 1; nada emitido)
- **Emissão:** `ErrorVerifier._checkForBadFunctionUse` (`error_verifier.dart:2226-2269`), de classe
  (`:512`) e alias de classe (`:549`); não de mixin.
- **Condição:** recurso `class_modifiers` **desligado** (biblioteca < 3.0); `extends Function` →
  `DEPRECATED_EXTENDS_FUNCTION`; primeiro `implements Function` (`break`) → `DEPRECATED_IMPLEMENTS_FUNCTION`;
  cada `with Function` → `DEPRECATED_MIXIN_FUNCTION`. Tipo `isDartCoreFunction`.
- **Posição:** o `NamedType`. **Mensagem:** "Extending 'Function' is deprecated." / "Implementing 'Function'
  has no effect." / "Mixing in 'Function' is deprecated." (warnings).
- **No DartForge:** inexistente; cabe em `clausulas.rs` (versão da biblioteca disponível em `features`).
  FN: `call/method_implicit_invoke_local_legacy_test.dart:16:21` (`// @dart=2.19`, `class C2 implements Function`).

##### `enum_mixin_with_instance_variable` (perda 4: FN 4; nada emitido)
- **Emissão:** `_ClassVerifier._checkMixinOfEnum` (`inheritance_override.dart:742-763`), de
  `_checkDirectSuperTypes` (`:524-526`) para cada tipo do `with` de um enum.
- **Condição:** tipo `InterfaceType` cujo elemento não é enum nem tipo de extensão, com algum campo
  `!isStatic && !isSynthetic` (inclui `final`) → relata e a verificação de herança para (§0.2).
- **Posição:** o `NamedType` inteiro. **Mensagem:** "Mixins applied to enums can't have instance variables.".
- **No DartForge:** `clausulas.rs:1136-1144` já detecta (`tem_campo_de_instancia`) só para tirar o enum da
  lista; emitir ali. 4 FN em `enum_mixin_with_instance_variable/*`.

##### `duplicate_definition` (perda 17: FN 2, FP 13, pos 2)
- **Emissão:** `_checkDuplicateIdentifier` (`duplicate_definition_verifier.dart:501-570`), por escopo
  (unidade, membros `_checkClassMembers` `:333-409`, parâmetros, variáveis locais; padrões: `:100-104`, sobre
  `declaration.elements`, que a resolução já deduplica — `resolution_visitor.dart:1144`, duplicata de padrão
  relatada como `duplicate_variable_pattern` `:1983`).
- **No DartForge:** `crates/analise/src/duplicatas.rs`.
  - **FP 12** em parâmetros nomeados privados (`duplicate_private_named_parameter/*` ×4,
    `private_named_parameters/{declaring_parameter,initializing_formal}_collision_error_test.dart` ×8; todos
    sintaxe nova, oráculo 3.13.4): relatamos "The name 'foo' is already defined." pela colisão do **nome
    público** (`_foo` ↔ `foo`) com o recurso `private-named-parameters` **desligado**
    (`experiment_not_enabled`). O parser (`frontend/src/parser/types.rs:799-838`
    `nome_publico_do_nomeado`) devolve o nome público mesmo sem o recurso (só os nomes inválidos são tratados
    à parte, `:823-833`); sem o recurso devolver `None`. Os 2 `posição:` (`declaring_parameter_collision_error_test.dart:69:44`,
    `:104:54`, `'_foo'`) são a mesma raiz (relatamos a colisão no outro parâmetro).
  - **FP 1** `DuplicateVariablePattern__variableDeclaration.dart:2:11` (`var [a, a] = …`): o analyzer só dá
    `duplicate_variable_pattern`; não relatar `duplicate_definition` para variáveis de padrão repetidas.
  - **FN 2** `class_modifiers/base/base_class_syntax_error_test.dart:78:1` e
    `sealed_class/sealed_class_syntax_error_test.dart:81:1`: `base typedef …`/`sealed typedef …` sem recurso →
    variável de topo `base`/`sealed` (recuperação), duplicando a de `base base class …` (linha 39); conferir a
    recuperação do nosso parser para `<palavra> typedef`.

##### `duplicate_constructor` (perda 2: FN 2)
- **Emissão:** `_checkClassMembers` (`duplicate_definition_verifier.dart:348-374`); para tipo de extensão,
  `_checkExtensionType` (`:734-747`) pré-registra `element.constructors.first.name` (o primário; `A.new`
  vira `''`) em `constructorNames`.
- **Posição:** `atConstructorDeclaration` (do nome da classe ao fim do nome do construtor: `A.new` len 5,
  `A` len 1).
- **No DartForge:** `duplicatas.rs:617` insere o nome do primário **sem** normalizar `new` → `''` (o membro
  normaliza, `:664-666`). 2 FN: `DuplicateConstructorDefault__extensionT_253a9e6e:2:3`, `…32cc51e4:2:3`.

##### `type_alias_cannot_reference_itself` (perda 2: FN 2)
- **Emissão:** `_checkForTypeAliasCannotReferenceItself` (`error_verifier.dart:5354-5365`), com
  `hasSelfReference` do link (`src/summary2/type_alias.dart:10-140`, `link.dart:468`): visita limites dos
  parâmetros de tipo, parâmetros formais (`DefaultFormalParameter`, `FunctionTypedFormalParameter`,
  `SimpleFormalParameter`), retorno, argumentos de tipo, tipos de função (parâmetros de tipo, parâmetros,
  retorno), campos de registro; segue classes/mixins (só limites), outros typedefs.
- **Posição:** nome do typedef.
- **No DartForge:** `crates/analise/src/membros.rs:393-500` (`AchaAutoReferencia`). FN:
  `nonfunction_type_aliases/cyclic_bound_error_test.dart:51:9` e `cyclic_bound_unused_error_test.dart:52:9`
  — `typedef T10<X extends void Function({T10<Never> x})> = List<X>;` (parâmetro **nomeado** de tipo de
  função no limite). `parametros` (`:473-488`) deveria cobri-lo; conferir se o parser guarda `ty` do
  parâmetro nomeado em tipo de função (o caso posicional opcional `[T8<Never>]` funciona).

##### `invalid_use_of_covariant` (perda 7: FN 7; nada emitido)
- **Emissão:** `ErrorVerifier._checkUseOfCovariantInParameters` (`error_verifier.dart:6143-6178`), de
  `visitFormalParameterList` (`:935-940`) — **toda** lista de parâmetros, inclusive de tipo de função
  (`GenericFunctionType`), de parâmetro função (`void p(covariant int)`), de expressão de função e de função
  local.
- **Condição exata:** sai sem relatar se: dentro de classe/mixin/enum e o pai é `MethodDeclaration`
  (estático já tem `extraneous_modifier` do parser); dentro de extensão (`invalid_use_of_covariant_in_extension`
  do parser); pai `FunctionExpression` de `FunctionDeclaration` de topo (`extraneous_modifier`). Senão, cada
  parâmetro (`notDefault`) com `covariantKeyword` relata.
- **Posição:** o token `covariant` (len 9).
- **Mensagem:** "The 'covariant' keyword can only be used for parameters in instance methods or before
  non-final instance fields.".
- **No DartForge:** inexistente; `ast::Parameter.covariant` é `bool` (`frontend/src/ast.rs:576`) — precisa do
  span do token (ou derivá-lo: início do parâmetro depois dos metadados). 7 FN em `invalid_use_of_covariant/*`
  (expressão de função, tipo de função em parâmetro/tipo/alias/limite, parâmetro-função, função local).
### Parte B_construtores1 — construtores, redirecionamentos e inicialização de campos

Grupo: 22 códigos, perda somada 292. Citações do fonte 3.6.2: `analyzer/lib/src/...:linha`
(em `E:\references\dart-sdk-3.6.2\pkg`). Abreviações: **EV** = `analyzer/lib/src/generated/error_verifier.dart`;
**CFV** = `analyzer/lib/src/error/constructor_fields_verifier.dart`; **ER** = `analyzer/lib/src/generated/element_resolver.dart`.

Rótulos das amostras (script de classificação): **NOVA** = arquivo em `sintaxe-nova.json` (oráculo 3.13.4);
**AUG** = arquivo com `augment class/enum/extension type` **sem** o experimento — no 3.6.2 o `augment` vira
identificador: no topo, `augment` é uma variável sem `;` (`expected_token` + `missing_const_final_var_or_type`
ou `undefined_class 'augment'`) e o resto é **outra declaração homônima** (`duplicate_definition`); no corpo,
`augment A(...)` é construtor com tipo de retorno `augment` (`constructor_with_return_type`) e `augment factory`
é `type_before_factory`. **PRIM** = construtor primário `class A(...)`; **THIS** = parte `this : ...`.

#### Achados transversais (valem para vários códigos)

1. **Nenhum destes códigos tem emissão registrada** fora de `final_not_initialized*`,
   `invalid_factory_name_not_a_class` e `duplicate_field_formal_parameter`. O rascunho
   `crates/analise/src/construtores.rs` não está em `analise/src/lib.rs` (não há `pub mod construtores;`) nem é
   chamado em `paridade/src/analise.rs`; além disso **não compila**: usa `c::PRIMARY_CONSTRUCTOR_CANNOT_REDIRECT`
   (código ausente da tabela `diagnostics/src/codigos_*.rs`) e `dartforge_frontend::features::Feature::SuperParameters`
   (não existe em `frontend/src/features.rs`; só há `PrimaryConstructors`, `Augmentations`, `Macros`…). Como recebe
   `&Program`, deve ser chamado na fase em que o programa existe (junto de `clausulas::verificador_de_heranca_prossegue`,
   `paridade/src/analise.rs:~506`).
2. **Classes homônimas: o escopo do analyzer é "a primeira vence"** (`analyzer/lib/src/dart/element/scope.dart:793-797`,
   `_getters[id] ??= element`), mas cada declaração repetida continua com **o seu próprio elemento** (campos e
   construtores só dela). O DartForge faz o contrário no escopo: `elements/src/outline.rs:131-134`
   (`entry.getter = Some(Element::Class(class_id))`, sobrescreve → **a última vence**). Toda a família AUG depende
   disso: dentro da segunda `A`, o nome `A` (na `factory A.x`, no alvo `= A`, em `implements A`) designa a **primeira** `A`.
3. **O rascunho pula exatamente esses casos**: (a) pula a unidade inteira se alguma linha começa com `augment `
   (`construtores.rs`, em `verificar`), (b) pula classe cujo nome se repete na biblioteca (`contagem != 1`).
   Sem o experimento, `decl.augment`/`membro.augment` já vêm `false` do parser
   (`frontend/src/parser/declarations.rs:269-280`, `parse_augment_opt`) e o membro `augment C(...)` já é lido como
   construtor (`declarations.rs:2500-2525`). Trocar (a) por "pular só se a biblioteca tem `Augmentations`/`Macros`"
   e (b) por "cada declaração usa os próprios campos; nomes escritos resolvem para a primeira declaração" recupera
   os FN AUG.
4. `analise/src/inicializacao.rs` indexa campos e construtores **pelo nome da classe** (`(bool, SymbolId)`), o que
   **funde classes homônimas** (FP de `final_not_initialized_constructor` em `augment A(int x);`) e **descarta
   campos de nome repetido** (`contagem[nome] == 1`), enquanto o analyzer os conta **uma vez** (o `Map<FieldElement,…>`
   do CFV usa a igualdade de `ElementImpl`, por localização — dois `v` colapsam).
5. Arquivos NOVA (3.13.4): o parser já elabora o primário (`declarations.rs:1186-1370`, `elaborar_construtor_primario`):
   parâmetros declarantes viram campos + `this.p` (`p.this_ = true`, `:1306`), a parte `this : inits` vira o
   inicializador do k2, e o k2 tem `class_name` = nome no cabeçalho. Não há marca no `Parameter` de que o `this.p`
   veio de um declarante (necessária em `duplicate_field_formal_parameter`).

---

##### `recursive_constructor_redirect` (perda 26: FN 26, FP 0, msg 0, pos 0)
- **Emissão:** ErrorVerifier, `visitConstructorDeclaration` (EV:589-614) chama
  `_checkForRecursiveConstructorRedirect` (EV:5047-5066) e `_checkForRecursiveFactoryRedirect` (EV:5073-5091);
  os dois usam `_hasRedirectingFactoryConstructorCycle` (EV:6327-6338). O `redirectedConstructor` do elemento vem
  do `ElementResolver.visitConstructorDeclaration` (ER:121-139: `= C.x` → `redirectedNode.staticElement`;
  `: this.x()` → `initializer.staticElement`) e, para outras bibliotecas, de
  `summary2/constructor_initializer_resolver.dart:68-72`. O placar junta os dois `uniqueName`
  (`RECURSIVE_FACTORY_REDIRECT` tem `name: 'RECURSIVE_CONSTRUCTOR_REDIRECT'`).
- **Condição:**
  ```
  ciclo(c0): vistos={}; c=c0
    while c != null: if c in vistos: return c identical c0
                     vistos.add(c); c = c.redirectedConstructor?.declaration
    return false
  gerador (sem factory): o PRIMEIRO RedirectingConstructorInvocation da lista; se ciclo(elem) → erro nele; return
  factory: se redirectedConstructor (o nó `= …`) != null e ciclo(elem) → erro; e então NÃO roda
           _checkForAllRedirectConstructorErrorCodes (EV:603-604)
  ```
  Construtor que entra num ciclo do qual não faz parte não é relatado (`RecursiveFactoryRedirect__outsideCycle`:
  só B e C). O alvo `= C<C<T>>` é o construtor sem nome de C (o próprio). O construtor redirecionado pode ser de
  outra classe (cadeias A→C→B→A em `__loop`/`__generic`/`__named`).
- **Posição:** gerador: o nó `RedirectingConstructorInvocation` inteiro (`this` … `)`, ex. `this.b()`); factory: o
  `ConstructorName` do alvo (tipo com argumentos + `.nome`, sem `;` — `C.foo` 5; em `= C.bar();` também 5, o `()`
  é erro de sintaxe).
- **Mensagem:** fixa "Constructors can't redirect to themselves either directly or indirectly." (correção "Try
  changing one of the constructors in the loop to not redirect."); sem argumentos.
- **Supressões:** nenhuma além de o nó existir. Não é filtrado por erro de sintaxe no analyzer (redirection3_cyclic_test
  tem `expected_token` e sai assim mesmo; no DartForge o filtro de `paridade` só cobre `depende_de_declaracoes`/
  `depende_da_linha`, que não incluem este código).
- **No DartForge:** não emitido (só no rascunho). Amostras: 24 FN comuns (12 gerador/factory simples
  `RecursiveConstructorRedirect__*`, `RecursiveFactoryRedirect__*`, `cyclic_constructor_test`, `redirect_cycle_test`,
  `redirect_indirect_cycle_test`, `redirection3_cyclic_test`, `regress23038_test`) + 2 NOVA (`const new named() :
  this.named()`, `const new () : this()` — forma `new` do 3.13; o rascunho só pega se o parser já ligar o `new`
  ao construtor). **Rascunho:** `em_ciclo` é fiel ao `_hasRedirectingFactoryConstructorCycle`; posições certas
  (`Initializer::Redirect.span` e `RedirectTarget.span` — conferir que este não inclui `()`/`;`); usa só o primeiro
  `Redirect` (certo). Erros: (1) pula homônimas e unidades com `augment` (achado 3); (2) `alvo_da_fabrica` resolve o
  nome pelo escopo do DartForge (última vence, achado 2); (3) `classe_escrita` devolve `None` em binding ambíguo e
  para nomes que não são classe — certo (sem elemento não há ciclo). Falta registrar o módulo.

##### `redirect_to_invalid_return_type` (perda 26: FN 26)
- **Emissão:** `_checkForAllRedirectConstructorErrorCodes` (EV:2025-2075, erro em EV:2064), chamado de
  `visitConstructorDeclaration` só se `_checkForRecursiveFactoryRedirect` devolveu `false` (EV:603-604).
- **Condição:** `redirectedConstructor` (nó `= T.n`) != null; `redirectedElement = staticElement`; se null →
  `REDIRECT_TO_MISSING_CONSTRUCTOR` (outro código) e fim. Senão `redirectedType = redirectedElement.type`
  (FunctionType, com a substituição dos argumentos de tipo do alvo), `constructorType = declaredElement.type`;
  `if (!isAssignableTo(redirectedType.returnType, constructorType.returnType))` → este erro e **return** (não sai
  também `_function_type`). Vale para `external factory … = Bar` (unsorted/external_test:63) e para enum e tipo de
  extensão (`= EBox.foo`). O tipo do alvo sem argumentos é inferido por `_inferRedirectedConstructor`
  (`analyzer/lib/src/dart/resolver/named_type_resolver.dart:172-200`: mesma classe → `thisType`; genérica → inferência
  contra `enclosingClass.thisType`; falha → limites, ex. `LinkFactory<dynamic>`).
- **Posição:** o `ConstructorName` (`A` 1; `EBox.foo` 8; `LinkFactory<T>.create` 21; `lib.Foo2` 8).
- **Mensagem:** "The return type '{0}' of the redirected constructor isn't a subtype of '{1}'." — ambos DartType
  (display com argumentos: `X<B>`/`X<T>`, `LinkFactory<T>`/`Link<T>`, `AFactory` quando a classe não é genérica).
- **Supressões:** ciclo de factory (RECURSIVE_FACTORY_REDIRECT) suprime; alvo não resolvido suprime.
- **No DartForge:** não emitido em lugar nenhum. O lugar natural é `types/src/inferencia/funcoes.rs`
  (`alvo_de_factory_redirecionadora`, que já resolve o alvo) ou uma passada de outline em `types`. Amostras:
  **14 AUG** (13 em `DefaultValueInRedirectingFactoryConstru_*` + `InvalidFactoryNameNotAClass__valid_inAu`):
  `augment factory A.foo(...) = A;` dentro da **segunda** `A`/`E` — o `= A` resolve para a primeira `A`, cujo tipo
  não é subtipo da segunda (mensagem "'A' … isn't a subtype of 'A'", "'EBox' … of 'E'", "'B' … of 'A'"); exige o
  achado 2. **12 comuns**: `RedirectToInvalidReturnType__*`, `TypeArgumentNotMatchingBounds__redirect` (`X<B>`
  vs `X<T>`), `default_factory_test`, `deferred/inheritance_constraints_*` (`lib.Foo2` prefixado), `factory*_test`,
  `external_test`.

##### `final_not_initialized_constructor` (perda 24: FN 21, FP 3; acerto 19/40)
- **Emissão:** CFV. `ErrorVerifier.visitClassDeclaration` (EV:505-507, só se `nativeClause == null`),
  `visitEnumDeclaration` (EV:711-712) e `visitExtensionTypeDeclaration` (EV:825-826) chamam
  `addConstructors(errorReporter, augmented, members)` (CFV:22-36); o relato sai no fim da biblioteca,
  `LibraryAnalyzer` → `constructorFieldsVerifier.report()` (`analyzer/lib/src/dart/analysis/library_analyzer.dart:313`)
  → `_Constructor.report()` (CFV:119-174).
- **Condição:**
  ```
  campos iniciais (CFV:74-98): para field in augmented.fields: pula isSynthetic; em enum pula 'index';
     estado = hasInitializer ? initInDeclaration : notInit      (inclui estáticos!)
  por ConstructorDeclaration em members (só os escritos; o primário de tipo de extensão NÃO é membro):
     pula factory, `= C` (redirectedConstructor) e external (CFV:51-55)
     parâmetros this.x (notDefault): field = FieldFormalParameterElement.field (augmented.getField(nome),
        `summary2/library_builder.dart:685-700`); notInit → initInFieldFormal
     inicializadores: RedirectingConstructorInvocation → hasRedirecting=true;
        `x = e` com staticElement FieldElement: notInit → initInInitializer
  report: se hasRedirecting → nada; senão faltam = campos notInit && !late && !abstract && !external && !static
     finais → nomes ordenados (sort de String): 1 → _1, 2 → _2, ≥3 → _3_PLUS(n0, n1, n-2)
  ```
  A chave do mapa é `FieldElement` com igualdade por localização: dois campos de mesmo nome contam **uma** vez.
- **Posição:** `node.returnType` — só o nome da classe, mesmo em `A.named()` (length 1 / 2 / 5). NOVA (3.13):
  primário → o nome no cabeçalho (`class A(` 1:7, `enum A(` 1:6); parte `this` → também o nome da classe.
- **Mensagem:** "All final variables must be initialized, but '{0}' isn't." / "'{0}' and '{1}' aren't." /
  "'{0}', '{1}', and {2} others aren't." Nome do campo cru; tipo de extensão sem primário tem campo `<empty>`
  (ConstructorBody__extensionType_primaryC_6c72dd61, VariableNotInitialized__extensionType_i_d6cf91d5:6:20).
- **Supressões:** `NOT_INITIALIZED_NON_NULLABLE_INSTANCE_FIELD_CONSTRUCTOR` sai do mesmo `report` para os não finais.
- **No DartForge:** `analise/src/inicializacao.rs:143-210` (`finais_nao_inicializados`). Causas dos 21 FN:
  **13 NOVA** (11 PRIM + 2 THIS): `inicializacao.rs:157` pula o primário (`if primario == Some(m) { continue; }`,
  com o comentário de que o corpus é 3.6.2 — está desatualizado: esses arquivos agora têm oráculo 3.13 e esperam o
  código no nome da classe; o teste `construtor_primario_nao_gera_codigo_semantico_no_corpus_36` afirma o contrário
  do oráculo atual); **4 tipo de extensão** (`ExtensionTypeConstructorWithSuperInvoca_7c3c540a`,
  `VariableNotInitialized__extensionType_i_a8487832` e `_d6cf91d5` ×2, `ConstructorBody__extensionType_primaryC`):
  `DeclKind::ExtensionType` cai em `_ => continue` (`:153`); o campo de representação (`it`, ou `<empty>` sem
  primário) é final e não inicializado para os construtores secundários; **2 campos repetidos**
  (`VariableNotInitialized__class_instanceF_856ce0bc`, `constructor9_test`): `contagem[nome] == 1` os descarta;
  **1 recuperação do parser** (`ExperimentalMemberUse__incorrectlyNeste_0e44e3bf`: `{required this.x, {this.y =
  false}}` — o fasta mantém o construtor com `this.x`; o nosso perde o construtor, daí também 2 FP de
  `final_not_initialized`). **3 FP NOVA AUG** (`ConstructorBody__class_primaryConstruct_*`: `augment A(int x);` na
  segunda `A`): a chave por nome funde as duas `A` (achado 4). O rascunho não trata este código.

##### `invalid_factory_name_not_a_class` (perda 23: FN 23; acerto 21/44)
- **Emissão:** `SimpleIdentifierResolver` (`analyzer/lib/src/dart/resolver/simple_identifier_resolver.dart:189-197`),
  ao resolver o `returnType` do construtor (`ResolverVisitor.visitConstructorDeclaration`,
  `generated/resolver.dart:2321`).
- **Condição:** `_isFactoryConstructorReturnType(node)` (`:344-351`: o identificador é o `returnType` de um
  `ConstructorDeclaration` com `factory`) e `!identical(element, enclosingClass.augmented.declaration)`, onde
  `element` é o que o **escopo** dá ao nome (primeira declaração da biblioteca com esse nome; pode ser não-classe ou
  null).
- **Posição:** o identificador do nome da classe antes do ponto (`A` em `factory A.foo`).
- **Mensagem:** fixa, sem argumentos.
- **No DartForge:** `analise/src/membros.rs:976-986` compara só `k.class_name.sym != nome_da_declaração`.
  **20 FN AUG** (`augment factory A.foo(...)` / `const factory A() = B;` dentro da segunda `A`/`E`): o nome é igual
  ao da declaração, mas resolve para a **primeira** `A` — regra a acrescentar: relatar também quando a declaração
  corrente não é a primeira da biblioteca com esse nome (ou o nome resolve, com primeira-vence, para outro elemento).
  **3 NOVA**: `augment factory E.named()`/`E(int it)` em arquivos 3.13 com primário, e `factory new() => …`
  (unnamed_new_error_test:35, sintaxe 3.13).

##### `undefined_constructor_in_initializer` (perda 18: FN 18)
- **Emissão:** dois caminhos, mesmo `name`:
  - explícito: `ElementResolver.visitSuperConstructorInvocation` (ER:337-375; erros em ER:357 e ER:363), na
    resolução (ResolverVisitor);
  - implícito: `ErrorVerifier._checkForUndefinedConstructorInInitializerImplicit` (EV:5444-5492, erro EV:5489).
- **Condição explícita:** `enclosingClass is InterfaceElement` e `supertype != null` (a superclasse do `extends`,
  sem aplicação de mixin: em `extends Super with M` é `Super`); `element = superType.lookUpConstructor(nome,
  biblioteca)`; se `element == null || !element.isAccessibleIn(biblioteca)` (construtor `_privado` de outra
  biblioteca) → com nome: `UNDEFINED_CONSTRUCTOR_IN_INITIALIZER` [superType, nome]; sem nome: `_DEFAULT`
  [superType]. Sem `extends`, a superclasse é `Object` ("The class 'Object' doesn't have a constructor named 'test'").
- **Condição implícita:** construtor não-factory, não-external, sem `super(...)` nem `this(...)` na lista;
  `superType != null`; se todos os construtores da superclasse são factory → nada (já há
  `NO_GENERATIVE_CONSTRUCTORS_IN_SUPERCLASS`); `superElement.unnamedConstructor == null` → `_DEFAULT`
  [superElement.name] (String, sem argumentos de tipo); se é factory → `NON_GENERATIVE_CONSTRUCTOR`; depois vêm
  `NO_DEFAULT_SUPER_CONSTRUCTOR_EXPLICIT` / `IMPLICIT_SUPER_INITIALIZER_MISSING_ARGUMENTS`.
- **Posição:** explícito: o nó `SuperConstructorInvocation` inteiro (`super.named()` 13, `super()` 7,
  `super._private(42)` 18). Implícito: `constructor.returnType` só (`D2` 2 também em `D2.named()`, `B` 1).
  NOVA (3.13): primário implícito → `B.foo` (5, nome + `.foo`), `new foo` (7), `this` (4); parte `this : super.x()`
  → o `super…()`.
- **Mensagem:** "The class '{0}' doesn't have a constructor named '{1}'." ({0} = DartType com argumentos de tipo,
  ex. `NoUnnamed<int>`; {1} = nome escrito, inclusive `new` e `_foo`) / "The class '{0}' doesn't have an unnamed
  constructor." ({0} DartType no explícito, `superElement.name` no implícito).
- **No DartForge:** não emitido. O rascunho só trata o caso implícito quando há construtor sem nome e retorna em
  silêncio quando não há (`let Some(sup_ctor) = sup_ctor else { return };`) — aí deveria sair `_DEFAULT`; o
  explícito não existe. Amostras: **10 comuns** (UndefinedConstructorInInitializer__expl_*, `_339b0438`,
  SuperFormalParameterWithoutAssociatedPo_3c060505/b10e2def, not_enough_positional_arguments_error_test:15,
  private/super_constructor_test e regress20394_test — privados de outra biblioteca —, super/call3_test:46/50);
  **8 NOVA** (4 PRIM+THIS `this : super.named()`, 2 primário implícito, `new foo()`, `super.new()`).

##### `initializing_formal_for_non_existent_field` (perda 17: FN 17)
- **Emissão:** `ErrorVerifier.visitFieldFormalParameter` → `_checkForValidField` (EV:895-896, EV:5702-5760).
- **Condição:** `parameter.parent.parent` (ou o avô via `DefaultFormalParameter`) é `ConstructorDeclaration`
  (inclui factory; não inclui parâmetros-função aninhados nem métodos); `field = element.field`
  (= `augmented.getField(nome)`: o primeiro de `fields`, **incluindo os sintéticos** de getter/setter e o
  `values` de enum, `summary2/element_builder.dart:346-351`); `field == null || field.isSynthetic` → este código;
  `field.isStatic` → `INITIALIZER_FOR_STATIC_FIELD`; senão `!isSubtypeOf(tipoDoParâmetro, tipoDoCampo)` →
  `FIELD_INITIALIZING_FORMAL_NOT_ASSIGNABLE`.
- **Posição:** o `FieldFormalParameter` (não o `DefaultFormalParameter`): de metadados/`required`/`covariant`/
  tipo/`this` até o nome (ou `?`/lista do parâmetro-função) — `this.x` 6, `required this.` 14, `this.` 5 com nome
  sintético.
- **Mensagem:** "'{0}' isn't a field in the enclosing class." — `parameter.name.lexeme` ('' para nome sintético;
  `_unknown` para nomeado privado).
- **No DartForge:** não emitido. Amostras: 11 comuns (classe, enum `const E(this.x)` com `int get x`, tipo de
  extensão `E.named(this.x)`, subclasse cujo campo é herdado — `660be275`: campo da superclasse **não** conta,
  `[this.x]` opcional), 1 AUG (`augment C(this.x)` na segunda `C`, sem campos), 4 NOVA PRIM (`class C(this.x)`,
  posição `this.x` 6) + 1 NOVA (`{this._unknown}`). **Rascunho:** regra certa (só campos declarados da própria
  declaração; enum: constantes como estáticos; tipo de extensão: o campo de representação), posição
  `span_sem_padrao` certa se `Parameter.span` começar nos metadados. Erros: (1) achado 3 (AUG); (2) os 2 casos
  `{this.}`/`{required this.}` (`ExperimentalMemberUse__namedParameterMissingName`,
  `MissingRequiredParam__constructor_field_ce0ca2ef`) dependem do parser: `frontend/src/parser/types.rs:713-718`
  faz `self.expect_identifier()?` depois de `this.` e falha a lista inteira; o fasta insere identificador sintético
  (`missing_identifier`) — precisa criar `name_from("", vazio)` como já faz para `,`/`)` em `:722-728`; o teste
  `fonte.get(span) != Some(texto)` do rascunho aceita o nome vazio. (3) Campo de nome repetido é pulado
  (`repetidos`); o analyzer usa o primeiro.

##### `primary_constructor_cannot_redirect` (perda 17: FN 17) — **só oráculo 3.13.4**
- **Emissão/condição (pelo corpus, 3.13 não disponível):** na lista de inicialização do construtor primário
  (parte `this : …`) cada `this(...)`/`this.n(...)` é erro; todas as ocorrências saem (`MultipleRedirecting…`:
  9:10 e 9:22). Todos os 17 são NOVA+PRIM+THIS.
- **Posição:** o token `this` do redirecionamento (length 4).
- **Mensagem:** "A primary constructor can't be a redirecting constructor." correção "Try removing the redirect.".
- **No DartForge:** o código **não existe** na tabela `diagnostics` (precisa ser acrescentado). O rascunho tem a
  regra certa (`e_primario` + cada `Initializer::Redirect`, `span.start..+4`) — a parte `this` é fundida no k2
  pelo parser, então os inicializadores estão no primário.

##### `default_value_in_redirecting_factory_constructor` (perda 16: FN 16)
- **Emissão:** `_checkForRedirectingConstructorErrorCodes` (EV:5094-5110), chamado por `visitConstructorDeclaration`
  (EV:600) antes dos demais.
- **Condição:** `redirectedConstructor != null` (nó `= …`); para cada parâmetro `DefaultFormalParameter` com
  `defaultValue != null`.
- **Posição:** `parameter.name` (o token do nome, ex. `x`, length 1).
- **Mensagem:** fixa "Default values aren't allowed in factory constructors that redirect to another constructor.".
- **No DartForge:** não emitido. 9 comuns + **7 AUG** (o padrão está no `augment factory … = A` da segunda
  declaração; ex. `_22f3fa27:8:30`). Rascunho correto (`k.redirect.is_some()` + `p.default_value` + `p.name`); só
  perde os AUG pelo achado 3.

##### `mixin_class_declares_non_trivial_generative_constructor` (perda 16: FN 16) — **só oráculo 3.13.4**
- **3.6.2:** o mesmo teste é `MIXIN_CLASS_DECLARES_CONSTRUCTOR` (`_checkForMixinClassErrorCodes`, EV:4320-4342:
  construtor não sintético, não factory, `!isTrivial` — `analyzer/lib/src/dart/ast/ast.dart:4307-4312`: sem `= …`,
  sem parâmetros, sem inicializadores, corpo vazio `;`, sem `external` — no `returnType`, arg `element.name`). No
  3.13 o código foi renomeado: "The mixin class '{0}' can't declare a non-trivial generative constructor."
- **Posição no 3.13 (corpus):** construtor `new(...)` → o `new` (3; `external new()` também no `new`); primário
  `mixin class A(int x)` → nome da classe (`A` 1, `M1` 2), com nome → `A.named` (7); parte `this {}` /
  `this : assert(true)` → o token **depois** do `this` (`{` ou `:`, length 1). Primário sem parâmetros e sem corpo
  (`mixin class A() {}`, `this;`) é trivial.
- **No DartForge:** `analise/src/clausulas.rs:828-844` (`classe_mixin`) pula de propósito primário e `new(...)`
  (`nova_sintaxe`). Todos os 16 são NOVA. Para cobrir: na biblioteca de sintaxe nova, emitir o código novo (a
  acrescentar à tabela) com as posições acima, inclusive para o k2 elaborado (não trivial se tem parâmetros, lista,
  corpo de bloco).

##### `final_not_initialized` (perda 13: FN 6, FP 7)
- **Emissão:** `_checkForFinalNotInitialized` (EV:3576-3615), chamado em `visitTopLevelVariableDeclaration`
  (EV:1558) e `visitVariableDeclarationStatement` (EV:1638, que retorna logo para `final` local); campos só via
  `_checkForFinalNotInitializedInClass` (EV:3625-3645, chamado de classe EV:511, enum EV:714, tipo de extensão
  EV:774, mixin/extensão EV:1255).
- **Condição:** lista não sintética, fora de classe nativa; `final` (não `const`) de variável local é da análise de
  fluxo; por variável sem inicializador: `FieldElement` abstract/external → nada; top-level `external` → nada;
  `late` → nada; senão erro. **Em classe/enum/tipo de extensão: se existe algum construtor gerador não sintético,
  a classe inteira é pulada — inclusive os campos `static final`** (EV:3629-3636). O primário do tipo de extensão
  conta como gerador não sintético.
- **Posição:** o nome da variável; **Mensagem:** "The final variable '{0}' must be initialized." (lexema; '' para
  nome sintético).
- **No DartForge:** `analise/src/inicializacao.rs:11-23` e `:127-150`. FP: **3** `static final` em classe/tipo de
  extensão com construtor (`inicializacao.rs:132`, `if v.static_ || !membros.1` — o `static_` não pode furar a
  regra); **2** `incorrectlyNeste` (parser perde o construtor); **2** tipo de extensão (`membros.1` é `false` para
  `ExtensionType`, `:131`; o primário deveria contar). FN: **4** `abstract final int foo;` de **topo**
  (`abstract` é `extraneous_modifier` e não vale fora de classe; `final_sem_inicializador` testa `v.abstract_`
  sem olhar o contexto — 3 deles AUG por acaso); **2** recuperação (`final final class X {}` e
  `final abstract class X {}` — o fasta cria variável `final` de nome sintético ''/tipo `abstract`, length 0 no
  token seguinte).

##### `field_initialized_by_multiple_initializers` (perda 12: FN 12)
- **Emissão:** CFV `updateWithInitializers` (CFV:197-236, erro CFV:226-231), durante `addConstructors`.
- **Condição:** `x = e` cujo `fieldName.staticElement` é `FieldElement` (resolvido em
  `generated/resolver.dart:2355`, `augmented.getField`) e estado `initInInitializer` (segunda atribuição na lista).
  Cada repetição a partir da segunda sai.
- **Posição:** `fieldName` (o identificador, sem `this.`). **Mensagem:** "The field '{0}' can't be initialized
  twice in the same constructor." — `element.displayName`.
- **No DartForge:** não emitido. 6 comuns + 1 (`duplicate_initializers_test:10`) + **5 NOVA THIS**
  (`this : v = 0, v = 0`) — o rascunho cobre todos (posição `name.span`, arg nome) se registrado, pois a parte
  `this` vira inicializadores do k2.

##### `field_initializer_not_assignable` (perda 10: FN 10)
- **Emissão:** `ResolverVisitor.visitConstructorFieldInitializer` (`generated/resolver.dart:2345-2366`) →
  `checkForFieldInitializerNotAssignable` (`generated/error_detection_helpers.dart:168-205`).
- **Condição:** campo resolvido (`augmented.getField`); `!isAssignableTo(staticType(expr), fieldType)` → em
  construtor `const` `CONST_FIELD_INITIALIZER_NOT_ASSIGNABLE`, senão `FIELD_INITIALIZER_NOT_ASSIGNABLE` (mesmo
  `name`); assinalável e campo não-void → `checkForUseOfVoidResult`. A expressão é inferida com o tipo do campo como
  contexto.
- **Posição:** a expressão. **Mensagem:** "The initializer type '{0}' can't be assigned to the field type '{1}'."
  (+ " in a const constructor." na variante const), DartTypes com alias (`T<dynamic>`/`T<Null>`), `int?`;
  context messages do *why not promoted*.
- **No DartForge:** `types/src/inferencia/funcoes.rs:150-159` (`inicializador`) chama
  `verificar_atribuivel_expr(..., INVALID_ASSIGNMENT.template)` → sai `invalid_assignment` no lugar (6 FP de
  `invalid_assignment` no placar nas mesmas posições). Trocar o template pelo código certo (const conforme
  `ctor.const_`). Restante: `generic_usage_futureor_error_test:16` (`T()` com alias; dependente de tipos de alias).

##### `field_initializing_formal_not_assignable` (perda 9: FN 9)
- **Emissão/condição:** `_checkForValidField` (EV:5733-5739): campo existe, não sintético, não estático, e
  `!isSubtypeOf(tipoDeclaradoDoParâmetro, tipoDoCampo)` (subtipo, não atribuível: `dynamic this.x` com campo `int`
  é erro). Sem tipo escrito o parâmetro herda o do campo (nunca erra).
- **Posição:** o `FieldFormalParameter` (tipo + `this.x`: `String this.x` 13, `dynamic this.x` 14).
- **Mensagem:** "The parameter type '{0}' is incompatible with the field type '{1}'." (DartTypes).
- **No DartForge:** não emitido (precisa de tipos: `types`, na resolução de parâmetros, `resolve.rs:1036`).
  8 comuns + 1 AUG (`AugmentationFormalParameterType__class__3d054f55`: `augment A(String this.p1);` é um
  segundo construtor sem nome **da mesma classe** — `duplicate_constructor` — e o `_checkForValidField` roda nele
  assim mesmo; o rascunho/implementação não pode pular unidades com `augment`).

##### `redirect_to_invalid_function_type` (perda 9: FN 9)
- **Emissão:** EV:2068-2075 (ramo `else if` de `_checkForAllRedirectConstructorErrorCodes`).
- **Condição:** tipo de retorno compatível, mas `!isSubtypeOf(redirectedType, constructorType)` (tipos de função
  completos: parâmetros contravariantes, opcionais/nomeados).
- **Posição:** o `ConstructorName`. **Mensagem:** "The redirected constructor '{0}' has incompatible parameters with
  '{1}'." — FunctionTypes no display do analyzer: `A Function(int, [int])`, `A Function({int x})`,
  `A Function(InvalidType)` (parâmetro `this.x` sem campo tem tipo inválido), `C Function({dynamic })` (nomeado
  com nome sintético: tipo + espaço + nome vazio).
- **No DartForge:** não emitido. 7 comuns + 2 AUG (o `factory A.foo([int x]) = A` da primeira declaração aponta
  para o primário recuperado de um tipo de extensão sem representação). Mesmo lugar do `_return_type`.

##### `duplicate_field_formal_parameter` (perda 8: FP 8; acerto 28/28)
- **Emissão:** `DuplicateDefinitionVerifier._checkDuplicateIdentifier` (`analyzer/lib/src/error/duplicate_definition_verifier.dart:240-246`):
  `DUPLICATE_FIELD_FORMAL_PARAMETER` só quando o anterior **e** o atual são `FieldFormalParameterElement`; senão
  `DUPLICATE_DEFINITION`. Nome sintético ou curinga `_` (com o recurso) não entram (`:213`).
- **No DartForge:** `analise/src/duplicatas.rs:66-82` (`formal_campo`). Os 8 FP são todos **NOVA PRIM**: no 3.13
  um parâmetro **declarante** (`var int _`, `final int _`, `required final String _foo`) não é initializing formal,
  e o oráculo dá `duplicate_definition` (inclusive declarante × `this._foo` escrito: `C8`, `C12`). O parser
  converte o declarante em `this.p` (`declarations.rs:1306`) sem deixar marca; é preciso um campo no `Parameter`
  (ex. `declarante: bool`) e `formal_campo = p.this_ && !p.declarante`.

##### `field_initialized_in_parameter_and_initializer` (perda 8: FN 8)
- **Emissão/condição:** CFV:220-225 — `x = e` com estado `initInFieldFormal` (o campo veio de `this.x` do mesmo
  construtor).
- **Posição:** `fieldName`. **Mensagem:** fixa "Fields can't be initialized in both the parameter list and the
  initializers.".
- **No DartForge:** não emitido. 3 comuns (classe, enum, tipo de extensão `A.named(this.it) : it = 0`) +
  `duplicate_initializers_test:18` + **4 NOVA THIS** — 3 com primário `class A(this.v)` + `this : v = 0` (rascunho
  cobre) e **1 tipo de extensão** `InvalidReferenceToThis__extensionType_p_2bda2a43` (`this : it = this.hashCode`):
  no 3.13 a parte `this` do tipo de extensão é o primário cujo parâmetro de representação conta como `this.it`; o
  rascunho pula (`DeclKind::ExtensionType` dá `primario = None` e a parte tem `parte_primaria` → `continue`).

##### `initializer_for_static_field` (perda 8: FN 8)
- **Emissão:** dois pontos: `_checkForValidField` (EV:5728-5732 / 5749-5753) para `this.x`, e
  `_checkForInvalidField` (EV:3933-3938) para `x = e`.
- **Condição:** o `FieldElement` resolvido (não sintético) é `isStatic` (inclui constantes de enum).
- **Posição:** `this.x` → o `FieldFormalParameter` (6); `x = e` → o `ConstructorFieldInitializer` inteiro
  (`x = 0`, 5). NOVA (3.13) primário `class A(this.x)` com `static int? x` → length 4 no `this`.
- **Mensagem:** "'{0}' is a static field in the enclosing class. Fields initialized in a constructor can't be
  static." (nome escrito).
- **Supressões:** o CFV também roda (o mapa inclui estáticos): `static final x = 0` + `this.x` dá ainda
  `FINAL_INITIALIZED_IN_DECLARATION_AND_CONSTRUCTOR`.
- **No DartForge:** não emitido. 6 comuns + 2 NOVA PRIM. Rascunho certo nos 6 (inclusive tipo de extensão
  `E.named(this.x) : this.it = 0`); nos NOVA, ajustar a posição (4) do primário.

##### `invalid_super_formal_parameter_location` (perda 8: FN 8)
- **Emissão:** `ErrorVerifier.visitSuperFormalParameter` (EV:1457-1478).
- **Condição:** se `_enclosingClass is ExtensionTypeElement` → `EXTENSION_TYPE_CONSTRUCTOR_WITH_SUPER_FORMAL_PARAMETER`
  (até em método) e fim; senão, se o pai da lista de parâmetros não é `ConstructorDeclaration` com
  `isNonRedirectingGenerative` (`analyzer/lib/src/dart/ast/extensions.dart:122-136`: sem `external`, sem `factory`,
  sem `this(...)`) → este erro. Vale para método, função de topo, função local, factory, construtor `external`,
  construtor redirecionador e parâmetro-função aninhado.
- **Posição:** o token `super` (5). **Mensagem:** fixa "Super parameters can only be used in non-redirecting
  generative constructors.".
- **No DartForge:** não emitido. Os 8 são comuns. O rascunho apenas pula esses construtores (certo para os
  `without_associated_*`) mas não emite este código. Lugar natural: `analise/src/membros.rs:205-215`
  (`formais_fora`, que já percorre parâmetros de funções não-construtor, recursivo nos aninhados) + os
  construtores factory/external/redirecionadores.

##### `no_default_super_constructor` (perda 8: FN 8)
- **Emissão:** `_checkForNoDefaultSuperConstructorImplicit` (EV:4634-4677), chamado em `visitClassDeclaration`
  (EV:494-502) **só** se a classe tem `extends`, `with` ou `implements` e `_checkClassInheritance` (EV:1871-1905)
  devolve `true` (nenhum de: extends de classe proibida, erro de implements, erro de mixin,
  `NO_GENERATIVE_CONSTRUCTORS_IN_SUPERCLASS`). Variante `_EXPLICIT`: EV:5520-5531, em biblioteca sem
  `super-parameters` (< 2.17), no lugar de `IMPLICIT_SUPER_INITIALIZER_MISSING_ARGUMENTS`.
- **Condição (implícita):** `augmented.constructors[0].isSynthetic` (a classe não declara construtor);
  `superType = element.supertype`; `unnamed = superElement.unnamedConstructor`; se `unnamed` é factory →
  `NON_GENERATIVE_IMPLICIT_CONSTRUCTOR`; se `isDefaultConstructor` (`analyzer/lib/src/dart/element/element.dart:1592-1605`:
  sem nome e nenhum parâmetro obrigatório, posicional ou `required` nomeado) → ok; senão (sem construtor sem nome,
  ou com obrigatório) e superclasse não é não-subtipável → erro. Não roda para `class C = S with M;`.
- **Posição:** `atElement(element)` → o nome da classe (`B`, 1). **Mensagem:** "The superclass '{0}' doesn't
  have a zero argument constructor." — {0} = `superType` (DartType com argumentos); {1} (só na correção) =
  `element.displayName`.
- **No DartForge:** não emitido e **não coberto pelo rascunho** (ele trata construtores escritos). Amostras: 8
  comuns (`NoDefaultSuperConstructor__super_requir_*` com `A({required int? a})`, constructor10_test ×3 —
  `A(this.x)` —, call3_test, not_enough_positional_arguments_error_test). Implementar em `analise` junto de
  `clausulas` (precisa saber que `_checkClassInheritance` passou) usando a superclasse escrita (não a aplicação de
  mixin do DartForge).

##### `initializer_for_non_existent_field` (perda 7: FN 7)
- **Emissão/condição:** `visitConstructorFieldInitializer` (EV:618-623) → `_checkForInvalidField` (EV:3925-3948):
  `staticElement` não é `FieldElement` (null) ou é sintético (getter/setter só) → erro.
- **Posição:** o `ConstructorFieldInitializer` inteiro. Na recuperação do fasta
  (`_fe_analyzer_shared/lib/src/parser/parser_impl.dart:4051-4140`, `parseInitializer`) `this.foo`, `this` e
  `this?.x()` sem `=` viram `<sintético ''> = expressão` com o sintético no offset do `this`: o nó cobre a expressão
  (`this.foo` 8, `this` 4, `this?.namedConstructor()` 24) e o argumento é ''.
- **Mensagem:** "'{0}' isn't a field in the enclosing class." (`fieldName.name`).
- **No DartForge:** não emitido. 5 normais (`x = …` com getter, sem campo, `setter_test`) + 2 de recuperação; o
  nosso parser já faz a mesma recuperação (`declarations.rs:3063-3076`, `inicializador_sem_atribuicao`: nome ''
  vazio, span desde o `this`), e o teste de texto do rascunho aceita '' — o rascunho cobre os 7.

##### `field_initialized_in_initializer_and_declaration` (perda 5: FN 5)
- **Emissão/condição:** CFV:212-219 — `x = e` com estado `initInDeclaration` e campo `final` ou `const`.
- **Posição:** `fieldName` (sem `this.`: `attempt_reinitialization_test:15:43`). **Mensagem:** fixa "Fields can't be
  initialized in the constructor if they are final and were already initialized at their declaration.".
- **No DartForge:** não emitido; os 5 são comuns; rascunho certo.

##### `final_initialized_in_declaration_and_constructor` (perda 4: FN 4)
- **Emissão/condição:** CFV `updateWithParameters` (CFV:238-268, erro CFV:254-262) — `this.x` (só se o construtor
  não é augmentation) com estado `initInDeclaration` e campo `final`/`const`.
- **Posição:** `parameter.name` (o token do nome). **Mensagem:** "'{0}' is final and was given a value when it was
  declared, so it can't be set to a new value." — `displayName`.
- **No DartForge:** não emitido; 4 comuns; rascunho certo.

---

#### Revisão do rascunho `crates/analise/src/construtores.rs` (regra por regra)

| Regra | Acerta | Errado / faltando |
|---|---|---|
| `recursive_constructor_redirect` (+factory) | ciclo = `_hasRedirectingFactoryConstructorCycle`; primeiro `this(...)`; posições | pula AUG/homônimas; resolve alvo com escopo última-vence |
| `default_value_in_redirecting_factory_constructor` | condição e posição (`p.name`) | pula AUG |
| `initializing_formal_for_non_existent_field` / `initializer_for_static_field` (`this.x`) | campos só declarados (sintético = inexistente), enum, representação; vale em factory | `{this.}` depende do parser (`types.rs:717`); campo repetido pulado (analyzer usa o primeiro); posição 3.13 do estático no primário (4) |
| `initializer_for_non_existent_field` / `_static_field` (`x = e`) | posição do nó inteiro, recuperação `''` | — |
| estados do CFV | mapa com estáticos, `index` de enum, ordem parâmetros→lista, guarda factory/`=`/external, posições e argumentos dos 4 códigos | tipo de extensão: a parte `this` (primário) é pulada; campos repetidos devem colapsar, não sumir |
| `primary_constructor_cannot_redirect` | regra e posição | código ausente da tabela (não compila) |
| `implicit_super_initializer_missing_arguments` | contagem de posicionais/nomeados obrigatórios menos os `super.x` | arg deve ser `superType` com argumentos de tipo (usa só o nome); sem construtor sem nome deve sair `undefined_constructor_in_initializer` (`_DEFAULT`) em vez de silêncio; factory → `non_generative_constructor`; `Feature::SuperParameters` não existe (usar versão ≥ 2.17 → `_EXPLICIT` abaixo) |
| `super_formal_parameter_without_associated_*` | índice entre os `super.` (como `indexIn`), nomeado pelo nome externo | `superConstructor` (`summary2/super_constructor_resolver.dart:28-62`) é **qualquer** construtor com o nome, inclusive factory e privado de outra biblioteca — o rascunho pula factory e privados; quando não há construtor (`None`) **todos** os `super.x` saem (o rascunho retorna) — caso `SuperFormalParameterWithoutAssociatedPo_3c060505` |
| não cobertos | — | `invalid_super_formal_parameter_location`, `no_default_super_constructor`, `undefined_constructor_in_initializer` explícito, `redirect_to_invalid_*`, `field_*_not_assignable`, `mixin_class_declares_non_trivial…` |

Resumo das amostras do grupo (linhas FN/FP do placar): 86 em arquivos NOVA (oráculo 3.13), 52 em arquivos AUG
(`augment` sem o experimento: declaração homônima ou membro `augment C(...)`), 154 casos comuns que só faltam
registrar/implementar.
### Parte C_construtores2 — construtores (redirecionamento, super, primários, enum)

Fonte: `E:\references\dart-sdk-3.6.2\pkg` (citado como `analyzer/lib/...:linha`,
`_fe_analyzer_shared/lib/...:linha`). Amostras: `placar-base-r3.txt`. "SN" = arquivo listado em
`corpus/diagnosticos/sintaxe-nova.json` (oráculo **3.13.4**, não 3.6.2).

#### Achados transversais (valem para vários códigos)

1. **Nenhum dos códigos de redirecionamento é emitido hoje** (`redirect_*`, `non_generative_*`,
   `enum_constant_invokes_factory_constructor`, `undefined_enum_constructor`,
   `super_formal_parameter_type_is_not_subtype_of_associated`, `tearoff_of_generative_constructor_of_abstract_class`):
   o grep por `c::NOME` só acha a tabela `diagnostics/src/codigos_g.rs`. Os lugares naturais já existem:
   - `types/src/inferencia/funcoes.rs:195` `alvo_de_factory_redirecionadora` (já resolve o alvo de
     `factory … = X.y` sintaticamente, só para `invalid_reference_to_generative_enum_constructor`);
   - `types/src/inferencia/funcoes.rs:160-175` (`Initializer::Super`/`Initializer::Redirect` →
     `chamar_construtor_de`, `:265`), que já acha o construtor alvo;
   - `types/src/inferencia/funcoes.rs:776-812` (`inferir_metadados_da_unidade`: argumentos das constantes de enum);
   - `types/src/inferencia/expr.rs:1652` `tearoff_de_construtor`;
   - o rascunho não registrado `analise/src/construtores.rs` (estado do `ConstructorFieldsVerifier`,
     superclasse, `alvo_da_fabrica`).
2. **Modelo de elementos sem construtor sintético para classe abstrata e enum**:
   `elements/src/outline.rs:754-773` só cria o sintético quando
   `!abstract_ && kind == ClassKind::Class`. O analyzer cria para toda classe sem construtor
   (`analyzer/lib/src/summary2/library_builder.dart:183`) e para enum sem construtor
   (`:253`, com `isConst = true`). Consequência: `redirect_to_abstract_class_constructor`,
   `tearoff_of_generative_constructor_of_abstract_class` e `undefined_enum_constructor` precisam
   tratar "classe abstrata/enum sem construtor declarado" como "tem o sem nome gerador" (o do
   enum é `const`, o da classe não é: `ConstructorElementImpl.isConst`, `dart/element/element.dart:1414`).
3. **Elementos como argumento** (`non_generative_constructor`): `ErrorReporter._convertElements`
   (`analyzer/lib/error/listener.dart:338-347`) troca um `Element` por `getDisplayString()`; para
   construtor é `writeConstructorElement` (`analyzer/lib/src/dart/element/display_string_builder.dart:78-93`):
   `<tipo de retorno> <displayName>(<parâmetros>)`, p.ex. `'A A.named()'`, `'A A()'`,
   `'A<int> A.named(int x, {required String y})'`. O 3.13 (SN) tira o tipo de retorno (`'A.named()'`).
   O DartForge não tem formatador de elemento; precisa de um (tipo de retorno = tipo `this` da classe
   com os parâmetros de tipo dela; parâmetros como `_writeFormalParameters`, `:405-462`, grupos `[...]`/`{...}`).
4. **Construtor primário (SN)**: `analise/src/membros.rs:971-973` pula o construtor primário
   (`primario == Some(m)` → `continue`) antes de `cx.inicializadores(...)`; por isso
   `multiple_super_initializers`, `super_in_enum_constructor` e `super_invocation_not_last` não
   saem para a parte `this : …` (as listas e os spans dela sobrevivem na elaboração,
   `frontend/src/parser/declarations.rs:1311-1323`). E o tipo de extensão não passa por
   `elaborar_construtor_primario` (só `Elaborando::Classe`/`Enum`, `declarations.rs:1044`, `:1535`):
   nada das partes `this` de `extension type` é conferido.
5. **Arquivos com `augment` sem o experimento**: no 3.6.2 `augment B(...)` vira construtor com tipo de
   retorno (`constructor_with_return_type` + `duplicate_constructor`) e o resto das regras roda
   normalmente nele (amostra `802d1e86`). O rascunho `construtores.rs` pula unidades com
   linha começando em `augment ` — essas amostras ficariam de fora.

---

##### `redirect_generative_to_missing_constructor` (perda 7: FN 7, FP 0, msg 0, pos 0)
- **Emissão:** `ErrorVerifier.visitConstructorDeclaration` (`analyzer/lib/src/generated/error_verifier.dart:589`)
  → `_checkForConflictingInitializerErrorCodes` (`:2684`), ramo `RedirectingConstructorInvocation`
  (`:2705-2716`). O `staticElement` do `this(...)` vem de `ElementResolver.visitRedirectingConstructorInvocation`
  (`analyzer/lib/src/generated/element_resolver.dart:299-328`): `augmented.unnamedConstructor` ou
  `getNamedConstructor(nome)` da classe envolvente (sem `lookUpConstructor`, sem acessibilidade).
- **Condição:** `_enclosingClass != null`; para cada `this(...)`/`this.n(...)` na lista de um construtor
  **sem `factory`**, se o elemento é `null`. Vale para classe, enum (`bb599ffa`), mixin? (mixin não tem
  construtor) e tipo de extensão.
- **Posição:** o nó `RedirectingConstructorInvocation` inteiro (`this.noSuchConstructor()` — do `this`
  ao `)`).
- **Mensagem:** `The constructor '{0}' couldn't be found in '{1}'.` — `{0}` = `enclosingClass.displayName`
  (sem argumentos de tipo) + `.nome` se houver; `{1}` = `displayName` (String, sem `<T>`).
- **Supressões/ordem:** quando falta o alvo não sai `redirect_to_non_const_constructor` (exige elemento)
  nem `redirect_generative_to_non_generative_constructor`. Contagem de redirecionamentos continua
  (`multiple_redirecting_constructor_invocations`, `field_initializer_redirecting_constructor` etc.).
- **No DartForge:** não emitido. Lugar: `types/src/inferencia/funcoes.rs:168` (`Initializer::Redirect`,
  já busca `inf.program.class(c).constructors.get(chave)`) ou `analise/src/membros.rs::inicializadores`
  (tem o `Container` e o span). Atenção ao sintético: classe sem construtor declarado tem o sem nome
  (mas um construtor que redireciona é declarado, então só importa para enum/abstrata — ver achado 2).
  Amostras: 3 de 3.6.2 (`375eb019` classe, `bb599ffa` enum, `044be837` `const A.b() : this.a()`);
  4 SN: `272a43f4` (classe com primário, `this.missing()`), `unnamed_new_error_test.dart:115`
  (`this.new()`, texto `'NoUnnamed.new'`), e 2 com `augment class C` (`0c516f6b`, `9946c654`): no 3.13 o
  `augment` de topo vira variável + **segunda** `class C` duplicada, sem primário, e o `augment C.named(...)`
  vira construtor com tipo de retorno cujo `this(x)` não acha `C` (mensagem `'C'`/`'C'`).

##### `redirect_to_non_class` (perda 7: FN 7)
- **Emissão:** resolução de tipos, não o ErrorVerifier: `ResolutionVisitor._resolveRedirectedConstructor`
  (`analyzer/lib/src/dart/resolver/resolution_visitor.dart:1638-1648`) marca o `NamedType` do alvo;
  `NamedTypeResolver._resolveToElement` (`named_type_resolver.dart:304-312`) ou o ramo "elemento que não
  é tipo" (`:245`, `:286`) chamam `_ErrorHelper.reportNullOrNonTypeElement` (`:518`), que após os casos
  `boolean`/catch/`as`/`is` relata `REDIRECT_TO_NON_CLASS` (`:573-581`) se
  `_isRedirectingConstructor(node)` (`:669-678`: pai `ConstructorName` cujo pai é a `ConstructorDeclaration`
  e é o `redirectedConstructor`).
- **Condição:** o nome do tipo do alvo (`= X`, `= X<T>`, `= X.y` com `X` não resolvido, `= p.X`) não
  resolve (`null`, salvo `shouldIgnoreUndefinedNamedType`, `dart/element/element.dart:1241`) ou resolve
  a algo que não é classe/alias/dynamic/Never/parâmetro de tipo (p.ex. campo `int A` da própria classe,
  `notAType`: o escopo é o da classe, membros incluídos). `X.y` com `X` não resolvido: o prefixo não é
  `PrefixElement` → `_resolveToElement(node, null)` (`:95-97`).
- **Posição:** `_getErrorRange` (`:653-666`): do `importPrefix` (se houver) ao fim de `name2`; os
  argumentos de tipo ficam de fora (`Foo<T>` → `Foo`, 3).
- **Mensagem:** `The name '{0}' isn't a type and can't be used in a redirected constructor.`,
  `{0}` = `name2.lexeme` (só o último segmento).
- **Supressões/ordem:** o tipo vira `InvalidType` → no ErrorVerifier `_checkForAllRedirectConstructorErrorCodes`
  não relata `redirect_to_missing_constructor` (`error_verifier.dart:2038`). Duplicatas legítimas: dois
  construtores iguais (`regress35259` linhas 6 e 11) relatam cada um.
- **No DartForge:** não emitido. Hoje não há FP no lugar (o `undefined_class` do alvo de redirect já é
  calado). Lugar: `alvo_de_factory_redirecionadora` (`funcoes.rs:195`) — quando `resolver_classe_alvo`
  dá `RefNome` não-`Elemento` ou um elemento que não é classe/typedef. Cuidado com `[a, b]` sem prefixo
  (`= C.x`): se `C` não resolve, relatar `C.x` inteiro com `{0} = x`. As 7 amostras são 3.6.2
  (classe, `factory5/6` com `Foo<T>`, `regress35259` ×2, `4924ba57` `const factory`).

##### `field_initializer_outside_constructor` (perda 6: FN 6; acerto 18)
- **Emissão (duas fontes, mesmo nome de código):**
  1. `ErrorVerifier.visitFieldFormalParameter` (`error_verifier.dart:895-898`) →
     `_checkForFieldInitializingFormalRedirectingConstructor` (`:3535-3569`): `CompileTimeErrorCode`, no
     **parâmetro inteiro** (`this.x`, `int this.x()`), mensagem "Initializing formal parameters can only be
     used in constructors." — já fazemos (`analise/src/membros.rs:205-215`, `formais_fora`).
  2. **Parser** (`AstBuilder.checkFieldFormalParameters`, `analyzer/lib/src/fasta/ast_builder.dart:779-789`):
     `ParserErrorCode.FIELD_INITIALIZER_OUTSIDE_CONSTRUCTOR` (`front_end/messages.yaml:2165`,
     "Field formal parameters can only be used in a constructor." / "Try removing 'this.'."), no **token
     `this`** (4). Chamado só por `endClassMethod` (`:1353`; também via `endMixinMethod` `:2608`,
     `endExtensionMethod` `:1687`, `endEnumMethod`/`endExtensionTypeMethod`
     `_fe_analyzer_shared/lib/src/parser/listener.dart:653`, `:1244`) e `endLocalFunctionDeclaration`
     (`:2471`). **Não** por funções de topo, expressões de função nem parâmetros aninhados.
- **Condição (cópia do parser):** para cada parâmetro *direto* da lista que seja `FieldFormalParameterImpl`
  — os opcionais/nomeados vêm embrulhados em `DefaultFormalParameter`, logo só os **posicionais
  obrigatórios** contam (o corpus confirma: `def_971fce3a`, `top_072ff5f5`, `closure`, `inF_b1bccb22`
  têm só a cópia do ErrorVerifier).
- **Posição:** o `this` (`int this.x()` → coluna do `this`).
- **No DartForge:** as 6 FN são exatamente a cópia do parser (métodos, `static`, setter, função local).
  Emitir `codigos::parser::FIELD_INITIALIZER_OUTSIDE_CONSTRUCTOR` (existe, `codigos_g.rs:869`) para
  `p.this_ && p.kind == Required` nas listas de `MemberKind::Method(fid)` e `StmtKind::Function(fid)`
  (não em `DeclKind::Function` de topo nem em literais). Pode ser em `membros.rs` junto de `formais_fora`
  ou no parser. Todas 3.6.2.

##### `invalid_modifier_on_constructor` (perda 6: FN 6)
- **Emissão:** `ErrorVerifier.visitConstructorDeclaration` (`error_verifier.dart:595-596`) →
  `_checkForInvalidModifierOnBody` (`:4044-4053`).
- **Condição:** `body.keyword != null` (`async`, `async*`, `sync*`) em qualquer `ConstructorDeclaration`:
  gerador **e factory** (não redirecionadora; a redirecionadora tem `EmptyFunctionBody`).
- **Posição:** o token `async`/`sync` (5/4; o `*` é outro token).
- **Mensagem:** `The modifier '{0}' can't be applied to the body of a constructor.`, `{0}` =
  `keyword.lexeme` → `'async'` também para `async*`, `'sync'` para `sync*`.
- **Supressões/ordem:** na factory sai junto com `non_sync_factory` (mesmo offset, ver abaixo).
- **No DartForge:** não emitido. `parse_constructor_resto` (`frontend/src/parser/declarations.rs:2919-2920`)
  lê o modificador e o **descarta** (`ast::Constructor` não tem campo). Guardar o `AsyncModifier` e o span
  do token (ou emitir ali como `compile_time_error`, como já se faz com
  `NON_REDIRECTING_GENERATIVE_CONSTRUCTOR_WITH_PRIMARY` `:1227`). A parte `this` (3.13) tem regra própria
  (`PRIMARY_CONSTRUCTOR_BODY_WITH_MODIFIER`, `:2833-2841`). Todas 3.6.2.

##### `initializing_declaring_parameter` (perda 5: FN 5) — **só SN (3.13.4)**
- **Origem:** não existe no 3.6.2. Pelo corpus: `SYNTACTIC_ERROR.INITIALIZING_DECLARING_PARAMETER`
  (`primary_constructors/syntax/initializing_declaring_error_test.dart:5`, `:14`), parâmetro declarante
  (`var`/`final`) do cabeçalho primário que também é `this.x`.
- **Posição:** o token `this` (4). **Mensagem:** "Declaring parameters can't be initializing." /
  correção "Try removing the `this.` prefix or making the parameter non-declaring.".
- **Efeito:** o parâmetro deixa de ser declarante (não há `duplicate_definition` com o campo `var str`).
- **No DartForge:** código ausente da tabela (`diagnostics`); a elaboração já ignora o caso em silêncio
  (`declarations.rs:1276`: `if !(p.var_||p.final_) || p.this_ || p.super_ { continue }`). Emitir lá,
  como `parser`, no `this` (achar o offset do `this` no texto do parâmetro). `const this.x` é outro erro
  (`extraneous_modifier`, já sai).

##### `non_generative_constructor` (perda 5: FN 5)
- **Emissão (duas):**
  1. explícito: `ElementResolver.visitSuperConstructorInvocation` (`element_resolver.dart:362-378`,
     fase de resolução do corpo): `superType.lookUpConstructor(nome, lib)` acha uma `factory` e **nem
     todos** os construtores da superclasse são factory → no nó `SuperConstructorInvocation` inteiro
     (`super.named()`).
  2. implícito: `_checkForUndefinedConstructorInInitializerImplicit` (`error_verifier.dart:5444-5500`):
     construtor gerador, não `external`, sem `super(...)`/`this(...)`; superclasse com algum gerador;
     `superElement.unnamedConstructor` existe e é factory (`:5495-5500`) → no `constructor.returnType`
     (o identificador da classe, `B` em `B.foo()`: comprimento 1).
- **Mensagem:** `The generative constructor '{0}' is expected, but a factory was found.`, `{0}` = elemento
  → `getDisplayString()` = `'A A.named()'` / `'A A()'` (achado 3).
- **Supressões:** se todos os construtores da superclasse são factory sai
  `no_generative_constructors_in_superclass` (`analise/src/clausulas.rs:629-641`) e este não; sem
  construtor sem nome sai `undefined_constructor_in_initializer_default`.
- **No DartForge:** não emitido. Explícito: `chamar_construtor_de` (`funcoes.rs:265`) para
  `Initializer::Super`; implícito: rascunho `construtores.rs` (já calcula `sup_ctor`, `fabrica` e o caso
  "todas factory"). Amostras 3.6.2: `66a12fc0` (explícito, 13 col.), `b104a23b` (implícito, `B.foo();` →
  posição `B`). SN (3): `0f4ecc5d` (`this : super.named()`), `c8a8e648` (`new foo();` → 7 col.),
  `e2af2d88` (`this;` → 4 col.), com mensagem 3.13 sem tipo de retorno.

##### `redirect_to_missing_constructor` (perda 5: FN 5)
- **Emissão:** `visitConstructorDeclaration` (`:603-605`): se `!_checkForRecursiveFactoryRedirect`
  (`:5073`) → `_checkForAllRedirectConstructorErrorCodes` (`:2025-2051`).
- **Condição:** há `redirectedConstructor`, `staticElement == null` (`ElementResolver.visitConstructorName`,
  `element_resolver.dart:142-161`: `type.lookUpConstructor(nome, lib)`, inclui acessibilidade de privado)
  e o tipo do alvo não é `dynamic` nem `InvalidType`.
- **Posição:** o `ConstructorName` inteiro (`A.name`, `A`, `NoUnnamed<T>.new`).
- **Mensagem:** `{0}` = `NamedType.qualifiedName` (`p.A` com prefixo) + `.nome`; `{1}` = `redirectedType`
  como **DartType** (com argumentos de tipo inferidos: `'NoUnnamed<T>'`).
- **No DartForge:** não emitido; `alvo_de_factory_redirecionadora` já encontra classe e nome (hoje sai
  em `None => {}`). 3 amostras 3.6.2 (`named`, `unnamed`, `318d6d7a const factory`), 2 SN
  (`unnamed_new_error_test.dart:121/126`, alvo `.new`).

##### `redirect_to_non_const_constructor` (perda 5: FN 5)
- **Emissão:** `_checkForRedirectToNonConstConstructor` (`error_verifier.dart:5139-5153`), chamado (a) por
  `_checkForRedirectingConstructorErrorCodes` (`:5111`, factory redirecionadora) e (b) por
  `_checkForConflictingInitializerErrorCodes` (`:2728`, `this(...)`).
- **Condição:** alvo resolvido, construtor atual `isConst` e alvo não `isConst` (sintético de classe não
  é const; o de enum é).
- **Posição:** (a) o `ConstructorName` do alvo (`A.a`, 3); (b) `initializer.constructorName ??
  initializer.thisKeyword` → o nome (`a`, 1) ou o `this` (4).
- **Ordem:** em (b) sai **depois** de `redirect_generative_to_non_generative_constructor` no mesmo
  inicializador (`enhanced_enums_error_test.dart:497`: 44/14 e 49/7).
- **No DartForge:** não emitido. Todas 3.6.2 (`4fde75eb` `this()`, `7687a90c` `this.a()`, `77971c9f`
  factory, enum 497, `regress27617`).

##### `super_formal_parameter_type_is_not_subtype_of_associated` (perda 5: FN 5)
- **Emissão:** `ErrorVerifier.visitSuperFormalParameter` (`error_verifier.dart:1457-1503`), após os testes
  de tipo de extensão, local inválido (`constructor.isNonRedirectingGenerative`) e sem associado.
- **Condição:** `!isSubtypeOf(element.type, superParameter.type)`; `superParameter` =
  `SuperFormalParameterElementImpl.superConstructorParameter` (`dart/element/element.dart:9291-9311`): do
  `superConstructor` (com substituição: `A<int>` → `int`), nomeado por nome, posicional pelo índice entre
  os `super.` posicionais. Sem tipo escrito o tipo é o herdado → nunca dispara; só com tipo explícito
  (inclusive `dynamic`).
- **Posição:** `node.name` (o identificador depois de `super.`).
- **Mensagem:** `{0}`, `{1}` DartType com alias (`'String?'`, `'int?'`, `'dynamic'`).
- **No DartForge:** não emitido. `types/src/resolve.rs:1038` já calcula `super_param_type` (substituído,
  `:1780-1840`); falta comparar com o tipo escrito e relatar (na passagem de corpos ou num verificador
  com `table`/`hierarchy`). Nota: `super_param_type_cru` soma os posicionais de `super(...)` ao índice
  (`:1806-1817`) e o 3.6.2 não. Amostras: 4 diretas 3.6.2 e `802d1e86` (3.6.2 com `augment B(...)` como
  construtor com tipo de retorno — achado 5).

##### `enum_constant_invokes_factory_constructor` (perda 4: FN 4)
- **Emissão:** `ResolverVisitor.visitEnumConstantDeclaration` (`analyzer/lib/src/generated/resolver.dart:2483-2503`).
- **Condição:** o `constantInitializer` da constante (`InstanceCreationExpression` sintética) resolveu a
  um construtor `isFactory` (`e2.named()`, ou `v`/`v()` com o sem nome factory).
- **Posição:** `arguments?.constructorSelector?.name` (o `named`) ou o nome da constante.
- **No DartForge:** não emitido. Lugar: `inferir_metadados_da_unidade` (`funcoes.rs:785-812`): quando `f`
  é factory, relatar (o caminho sem argumentos, `:810`, também). Todas 3.6.2.

##### `field_initialized_in_declaration_and_parameter_of_primary_constructor` (perda 4: FN 4) — **só SN**
- **Origem 3.13** (pelo corpus `double_initialization_error_test.dart:10` e os `VariableNotInitialized_*`):
  versão primária do estado `initInDeclaration` do `ConstructorFieldsVerifier` (no 3.6.2,
  `analyzer/lib/src/error/constructor_fields_verifier.dart:236-262`, `final_initialized_in_declaration_and_constructor`).
- **Condição:** `this.x` no **cabeçalho primário** e o campo tem inicializador na declaração — qualquer
  campo, `final` ou não (`248f8e38` `int v = 0`, `df673f1b` `final int v = 0`); substitui
  `final_initialized_in_declaration_and_constructor`. Depois do formal o estado vira "no formal": um
  `this : v = 0` seguinte dá `field_initialized_in_parameter_and_initializer` (`521fd215`).
- **Posição:** o nome do parâmetro (`v`, 1). Correção: "Try removing one of the initializations.".
- **No DartForge:** código ausente da tabela. Lugar: rascunho `construtores.rs` (bloco `Estado`,
  `e_primario` já existe): no ramo `Estado::NaDeclaracao` dos formais, se `e_primario` relatar este e
  passar a `NoFormal`.

##### `super_initializing_declaring_parameter` (perda 4: FN 4) — **só SN**
- Como `initializing_declaring_parameter`, para `var super.x`/`final super.x` no cabeçalho primário.
  Posição: o token `super` (5). Mensagem "Declaring parameters can't be super parameters." / "Try removing
  the `super.` prefix or making the parameter non-declaring.". Mesmo lugar (`declarations.rs:1276`).

##### `undefined_enum_constructor` (perda 4: FN 4)
- **Emissão:** `visitEnumConstantDeclaration` (`resolver.dart:2504-2520`): construtor não resolvido e o
  tipo é enum → `UNDEFINED_ENUM_CONSTRUCTOR_NAMED` no nome do seletor (arg `nome`), senão
  `UNDEFINED_ENUM_CONSTRUCTOR_UNNAMED` no nome da constante. `sharedName` `undefined_enum_constructor`
  (`messages.yaml:16872`, `:16963`).
- **Condição:** o sem nome só falta quando o enum declara construtores e nenhum sem nome (enum sem
  construtor tem o sintético `const`).
- **No DartForge:** não emitido. Em `funcoes.rs:791-812` o `else` (`f == None`) e o caminho sem
  argumentos só calam; atenção ao achado 2 (enum sem construtor declarado não tem sintético no modelo:
  não relatar). 3 amostras 3.6.2; 1 SN (`enum E2.named(...)` com `e(1)`, primário nomeado).

##### `field_initialized_in_declaration_and_initializer_of_primary_constructor` (perda 3: FN 3) — **só SN**
- Variante 3.13 de `field_initialized_in_initializer_and_declaration` para a lista da parte `this`:
  campo com inicializador na declaração e `x = e` na lista do primário, **qualquer campo** (`c942df16`
  `int v = 0`). Posição: o nome do campo no inicializador. Mensagem "Fields can't be initialized in both
  the primary constructor and at their declaration.". Mesmo lugar do anterior (`Estado::NaDeclaracao` dos
  inicializadores com `e_primario`).

##### `non_sync_factory` (perda 3: FN 3)
- **Emissão:** parser, `parseFactoryMethod` (`_fe_analyzer_shared/lib/src/parser/parser_impl.dart:5139-5145`):
  `asyncToken = token.next` após os parâmetros; se `!inPlainSync` →
  `reportRecoverableError(asyncToken, messageFactoryNotSync)`; o analyzer converte por `analyzerCode`
  (`analyzer/lib/src/fasta/error_converter.dart:393-398`) em `CompileTimeErrorCode.NON_SYNC_FACTORY`.
- **Posição:** o token `async`/`sync` (5/4). Sai antes do teste do `=`, logo também em factory
  redirecionadora com modificador.
- **Ordem:** junto com `invalid_modifier_on_constructor` no mesmo offset (oráculo traz os dois).
- **No DartForge:** não emitido (o modificador é descartado em `parse_constructor_resto`,
  `declarations.rs:2919`; e no ramo `=` nem é lido). Emitir no parser como `compile_time_error`. Todas 3.6.2.

##### `redirect_to_abstract_class_constructor` (perda 3: FN 3)
- **Emissão:** `_checkForRedirectingConstructorErrorCodes` (`error_verifier.dart:5116-5131`).
- **Condição:** alvo resolvido, `enclosingElement3` é `ClassElement` abstrata e o alvo **não** é factory
  (inclui o sintético: `= B` com `abstract class B extends A {}`, `interface_test` `= InterfaceTest`).
- **Posição:** o `ConstructorName` do alvo. **Mensagem:** `{0}` = `_enclosingClass.displayName` +
  `.${declaration.name}` (o construtor que redireciona, `'A.named'`), `{1}` = `redirectedClass.name`.
- **No DartForge:** não emitido; depende do achado 2 (o modelo não tem o sintético de classe abstrata).

##### `redirect_to_type_alias_expands_to_type_parameter` (perda 3: FN 3)
- **Emissão:** `NamedTypeResolver._verifyTypeAliasForContext` (`named_type_resolver.dart:420-450`): alias
  cujo `aliasedType` é `TypeParameterType`, pai `ConstructorName` que é o `redirectedConstructor` →
  relata e devolve `InvalidType` (sem `redirect_to_missing_constructor` depois).
- **Posição:** `_getErrorRange` (prefixo..nome do alias, sem argumentos de tipo): `B` em `B<A>.named`.
- **No DartForge:** não emitido; `classe_de_elemento` (`funcoes.rs:253-262`) já lê o alvo do typedef:
  `Type::TypeParameter` → relatar no primeiro segmento. Todas 3.6.2.

##### `redirect_generative_to_non_generative_constructor` (perda 2: FN 2)
- **Emissão:** `_checkForConflictingInitializerErrorCodes` (`error_verifier.dart:2717-2723`): `this(...)`
  resolvido a uma `factory` (construtor sem `factory`). **Posição:** o inicializador inteiro.
  Sem argumentos. Seguido de `redirect_to_non_const_constructor` quando o atual é `const`.
- **No DartForge:** não emitido; mesmo lugar de `redirect_generative_to_missing_constructor`. 3.6.2.

##### `tearoff_of_generative_constructor_of_abstract_class` (perda 2: FN 2)
- **Emissão:** `ConstructorReferenceResolver.resolve` (`analyzer/lib/src/dart/resolver/constructor_reference_resolver.dart:31-41`):
  elemento resolvido, não factory, classe `ClassElement` abstrata. **Posição:** o `ConstructorReference`
  inteiro (`A.new`, 5; com argumentos de tipo, inclui-os).
- **No DartForge:** não emitido; `tearoff_de_construtor` (`types/src/inferencia/expr.rs:1652`), depois de
  achar `f` — e o caso `construtor == None` de classe abstrata sem construtor declarado (achado 2) deve
  contar como sintético gerador. 1 amostra 3.6.2; 1 SN (atalho de ponto `.new`).

##### `factory_constructor_new_name` (perda 1: FN 1) — **só SN**
- Não existe no 3.6.2. 3.13 (`constructor/unnamed_new_error_test.dart:35`): `factory new()` → este código
  e `invalid_factory_name_not_a_class`, ambos no `new` (3). Mensagem "Factory constructors can't be named
  'new'." / "Try removing the 'new' keyword or changing it to a different name.".
- **No DartForge:** o arquivo é lido com `PrimaryConstructors`, e o ramo de factory
  (`declarations.rs:2486-2495`) só aceita `factory new(` **sem** o recurso → cai em outro caminho (FP
  `expected_token`, `missing_const_final_var_or_type`, `experiment_not_enabled` em 35). Aceitar
  `factory new (` sempre e emitir os dois códigos (o comentário em `:2491` já prevê).

##### `multiple_primary_constructor_body_declarations` (perda 2: FN 2; acerto 2) — **só SN**
- Emitido em `declarations.rs:1207-1210`. FN: (1) sem cabeçalho (`C3`, linha 31) o `let Some(cab) = cab
  else { … return None }` (`:1200-1206`) sai antes; o 3.13 relata **os dois** códigos na 2ª parte
  (`without_declaration` + `multiple`) — mover o laço do `multiple` para antes do retorno; (2) `extension type ET1(int x) { this; this : … }`
  (linha 61): tipo de extensão não é elaborado (achado 4). Posição: o `this` (4).

##### `multiple_super_initializers` (perda 1: FN 1; acerto 1) — **só SN nas FN**
- `_checkForConflictingInitializerErrorCodes` (`error_verifier.dart:2740-2746`): o 2º `super(...)` (nó
  inteiro). FN `7eab4ea8`: `this : super(), super();` na parte primária — achado 4 (`membros.rs:971`).

##### `non_generative_implicit_constructor` (perda 1: FN 1)
- **Emissão:** `visitClassDeclaration` (`error_verifier.dart:498-501`): só se `_checkClassInheritance`
  devolve `true` (nenhum erro de cláusula, p.ex. `no_generative_constructors_in_superclass`) →
  `_checkForNoDefaultSuperConstructorImplicit` (`:4634-4660`): o primeiro construtor da classe é sintético
  (nenhum declarado), `superElement.unnamedConstructor` existe e é factory (`:4652`).
- **Posição:** `atElement(element)` = nome da classe (`B`). **Mensagem:** `{0}` = nome da superclasse,
  `{1}` = nome da classe; `{2}` (só na correção) = o elemento (`'A A()'`).
- **No DartForge:** não emitido. Lugar: `analise/src/clausulas.rs:625-641`, depois do teste de
  `no_generative_constructors_in_superclass`, com `!fechada` (equivale ao `moreChecks`). Com `with` o
  supertipo é a aplicação (só geradores repassados) → não dispara. 3.6.2.

##### `primary_constructor_body_with_expression_body` (perda 1: FN 1; acerto 1) — **só SN**
- Emitido em `declarations.rs:1328-1338` para classe/enum. FN `cb08b938`: `extension type A(int x) { this => 0; }`
  — achado 4 (tipo de extensão não elabora as partes `this`). Posição: o `=>`.

##### `super_in_enum_constructor` (perda 1: FN 1; acerto 4) — **só SN na FN**
- `error_verifier.dart:2735-2739`: `super(...)` em construtor de enum, no token `super`. FN
  `enum E() { v; this : super(); }` — achado 4.

##### `super_invocation_not_last` (perda 2: FN 2; acerto 18) — **só SN nas FN**
- 3.6.2: `error_verifier.dart:2777-2794` (classe não enum, sem redirecionamento, exatamente um `super`, que
  não é o último inicializador; no token `super`; `{0}` = `supertype.element.displayName` + `.nome`); e
  também pelo parser/`error_converter.dart:421-426`. FN: partes primárias (`this : super(), x = 0;`) —
  achado 4. **Mensagem difere no 3.13**: "The superconstructor call must be last in an initializer list."
  **sem** `: '{0}'` (o 3.6.2 tem o argumento, `messages.yaml:15851`). Ligar o primário sem tratar isso
  troca FN por `msg≠`: em biblioteca com sintaxe nova (`paridade/src/analise.rs:297-327`,
  `libs_com_sintaxe_nova`) usar a variante sem argumento.

---

#### Resumo da classificação das FN (92)
- 3.6.2, regra ausente (57): redirect_* (26), field_initializer_outside_constructor (6, cópia do
  parser), invalid_modifier/non_sync_factory (9), super_formal tipo (5), enum factory/undefined (7),
  non_generative(_implicit) (3), tearoff (1).
- SN / 3.13.4 (35): primários (initializing/super_initializing declaring 9, field_initialized_*_primary 7,
  multiple/super/not_last/expression_body/super_in_enum 7, redirect no primário 1, non_generative
  primário 3, enum primário 1), `augment class` de topo 2, `.new` 3, atalho de ponto 1,
  factory_constructor_new_name 1.
### Parte D_ffi — códigos do `FfiVerifier` (dart analyze 3.6.2)

Grupo: 33 códigos, perda total **153** (todos FN: hoje o DartForge não emite nenhum `FfiCode`; os
códigos já existem na tabela, `crates/diagnostics/src/codigos_g.rs:2828-2878`, `c::ffi::*`, mas
ninguém os usa). Todos os casos estão no grupo `analyzer` do corpus (oráculo 3.6.2); nenhum arquivo
FFI está em `sintaxe-nova.json`.

#### Visão geral (vale para todos os códigos)

- **Fonte única:** `analyzer/lib/src/generated/ffi_verifier.dart` (2643 linhas), classe
  `FfiVerifier extends RecursiveAstVisitor<void>` (`:23`). Mensagens em
  `analyzer/lib/src/dart/error/ffi_code.g.dart` (gerado do `messages.yaml`). Nenhum outro arquivo do
  analyzer emite `FfiCode` (grep).
- **Fase:** `LibraryAnalyzer._computeVerifyErrors` (`analyzer/lib/src/dart/analysis/library_analyzer.dart:420-457`):
  depois do `ConstantVerifier`, do `InheritanceOverrideVerifier` e do `ErrorVerifier`,
  `unit.accept(FfiVerifier(_typeSystem, errorReporter, strictCasts: ...))` (`:455`). Roda em toda
  unidade, importe ela `dart:ffi` ou não; não há `hasError`/supressão por erros anteriores — os
  códigos FFI saem junto com `undefined_class`, `not_enough_positional_arguments`,
  `invalid_annotation` etc. (vários casos do corpus têm os dois).
- **Como o analyzer sabe que algo é de `dart:ffi`:** sempre por **nome do elemento + biblioteca**:
  `InterfaceElement.isFfiClass => library.name == 'dart.ffi'` (`:2400-2402`),
  `ExtensionElement.isFfiExtension` idem (`:2409-2413`), `isTypedDataClass => library.name ==
  'dart.typed_data'` (`:2404-2406`). `dart.ffi` é o nome da diretiva `library dart.ffi;` do
  `sdk/lib/ffi/ffi.dart:11`. No DartForge o equivalente é `program.library(classe.library).uri ==
  "dart:ffi"` (já usado em `crates/analise/src/clausulas.rs:1454` e
  `crates/analise/src/modificadores.rs:82`); `dart:ffi` já é carregado pela seção `vm` do
  `libraries.json` (`crates/paridade/src/analise.rs:109-124`), do SDK 3.6.2
  (`C:/tools/dartsdk-3.6.2/lib/ffi/*.dart`).
- **Predicados por nome** (todos exigem `isFfiClass`/`isFfiExtension`):
  - classes: `Struct`, `Union` (`isCompound`, `:2482-2492`), `AbiSpecificInteger`,
    `AbiSpecificIntegerMapping`, `Array`, `Packed`, `Pointer`, `NativeFunction`, `NativeType`,
    `Handle`, `Void`, `Opaque`, `VarArgs`, `Native`, `NativeCallable`, `DefaultAsset`; primitivos por
    nome `Int8..Int64`, `Uint8..Uint64`, `IntPtr` (`_primitiveIntegerNativeTypes`, `:70-83`),
    `Float`/`Double`, `Bool`, `Void`, `Handle` (`_primitiveNativeType`, `:911-934`).
    `Allocator`/`Finalizable` só são comparados por **nome**, sem biblioteca (`:157-161`).
  - extensões: `AllocatorAlloc`, `DynamicLibraryExtension`, `NativeFunctionPointer`,
    `StructPointer`/`UnionPointer`/`StructArray`/`UnionArray`, e as de `.address`
    (`ArrayAddress`, `StructAddress`, `UnionAddress`, `BoolAddress`, `DoubleAddress`, `IntAddress`,
    `Int8ListAddress`…`Float64ListAddress`, `:39-68`).
  - "subtipo de compound" tem DOIS sentidos: para `DartType` (`isCompoundSubtype`, `:2495-2504`) é
    só o **supertipo direto** (`element.supertype`) ser `Struct`/`Union`; para `NamedType` em
    cláusulas (`:2605-2611`) é `allSupertypes.any(isCompound)`. Idem para AbiSpecificInteger
    (`:2458-2470` × `:2596-2602`).
- **Anotações:** `Annotation.element` (o `ConstructorElement` ou o getter da const) e
  `ElementAnnotation.computeConstantValue()` (valor da constante: tipo `Native<T>`, campos `isLeaf`,
  `memberAlignment`, `dimension1..5`/`dimensions`/`variableLength`, `mapping`). Anotação cujo valor
  é `null` (avaliação falhou) é ignorada em `_checkFfiNative` (`:449-453`).
- **Formatação:** argumentos `DartType` passam pelo `ErrorReporter` ⇒ `getDisplayString` **com
  alias** (`typedef T = Int8 Function(Int8)` aparece como `'T'`); `InvalidType` aparece como
  `InvalidType` (`'InvalidType Function()'`); `dynamic`; `Function`. Argumentos `String` saem como
  estão (`fieldType.toSource()`, `node.name.lexeme`, `'T'` literal).
- **No DartForge (todos):** não existe verificador FFI. Proposta: novo módulo
  `crates/types/src/ffi.rs` (precisa de `TypeTable`, `OutlineTypes`, `BodyTypes`, hierarquia e
  `subtyping::is_subtype`), chamado por biblioteca em `crates/paridade/src/analise.rs` depois das
  constantes (junto do bloco de `sobrescritas`, ~`:488-520`), devolvendo `Vec<(UnitId, Diagnostic)>`.
  Infra que falta: (1) resolver uma `ast::Annotation` para *(classe ffi, construtor, tipo
  instanciado, argumentos)* — o resolvedor de nomes já existe em
  `types/src/inferencia/funcoes.rs:953-1095` (`validar_anotacao`/`anotacao_sem_validar`), e o
  `construir` já guarda a instanciação inferida em `UnitBodyTypes::instanciacao(args.span.start)`
  (`types/src/resolved.rs:88,139`), de onde sai o `T` de `@Native()`; (2) valores de campos das
  anotações: o avaliador de constantes trata o SDK como **opaco**
  (`types/src/constantes/avaliador.rs:9,1842`: construtor de biblioteca não inferida ⇒ valor
  desconhecido), então `isLeaf`, `memberAlignment`, dimensões e `mapping` não sairiam da avaliação;
  é preciso mapear os argumentos para os campos à mão (os construtores são triviais: `Packed(this.memberAlignment)`,
  `Array(d1,[d2..d5])`→`_ArraySize`, `Array.multi(List)`, `AbiSpecificIntegerMapping(this.mapping)`,
  `Native({assetId, isLeaf=false, symbol})`) e avaliar só os **argumentos** (que são da biblioteca
  do usuário) com o avaliador; para `@c` (const do usuário) seguir o inicializador da variável.
  `ast::Annotation.span` inclui o `@` (`frontend/src/parser/declarations.rs:700-731`), igual ao nó
  `Annotation` do analyzer; `annotation.name` (sem `@`) = span de `name[0]` até o último nome antes
  do construtor.

---

##### `must_be_a_native_function_type` (perda 41: FN 41, FP 0, msg 0, pos 0)
- **Mensagem:** `ffi_code.g.dart:330` — "The type '{0}' given to '{1}' must be a valid 'dart:ffi'
  native function type." (correção "Try changing the type to only use members for 'dart:ffi'.").
- **Emissão (6 pontos):**
  1. `_checkFfiNative` (`:498`) — declaração função/método com `@Native<X>` onde `X` **não é
     `FunctionType`**: `arguments: ['T', 'Native']` (o `'T'` é literal!) no token do nome.
  2. `_checkFfiNativeFunction` (`:682-689`) — `@Native<F>` com `F` função mas
     `!_isValidFfiNativeFunctionType(nativeType)`: `[nativeType, 'Native']` no nome; `nativeType` é
     `F` reconstruído sem o receptor (métodos de instância) (`:676-681`).
  3. `_validateFromFunction` (`:1541-1552`): `T = node.typeArgumentTypes[0]` inválido ⇒ no
     `typeArguments[0]` se escrito, senão no `methodName`; `[T, 'fromFunction']`. Só se 1 ≤ nº args ≤ 2
     (`:1533-1538`).
  4. `_validateLookupFunction` (`:1641-1648`): `S` inválido ⇒ em `typeArguments[0]`,
     `[S, 'lookupFunction']`; só com exatamente 2 argumentos de tipo escritos (`:1631-1636`).
  5. `_validateNativeAddressOf` (`:1700-1705`): anotação `@Native` do referenciado tem `T` função e
     o argumento de tipo da chamada não é `NativeFunction` ⇒ no nó inteiro da invocação,
     `[targetType, 'Native.addressOf']`.
  6. `_validateNativeCallable` (`:1771-1778`): `typeArg = (node.staticType as ParameterizedType).typeArguments[0]`
     inválido ⇒ em `node.constructorName` (inclui `<...>`), `[typeArg, 'NativeCallable']`. Só se
     `argCount == 1 || (isolateLocal && argCount == 2)`.
- **Condição `_isValidFfiNativeFunctionType`** (`:777-797`): `FunctionType` que não seja `Function`,
  sem opcionais/nomeados, retorno `_isValidFfiNativeType(allowVoid, allowHandle)` e cada parâmetro
  (após `flattenVarArgs`, `:2622-2642`) `_isValidFfiNativeType(allowHandle)`.
  `_isValidFfiNativeType` (`:800-865`): primitivos ffi (int/double/bool sempre; Void/Handle
  conforme flags), `NativeFunction<F>` recursivo, `Pointer<X>` (quase tudo), subtipo direto de
  Struct/Union (não vazio salvo `allowEmptyStruct`), `Opaque`/subtipo de Opaque, subtipo de
  `AbiSpecificInteger`, `Array` só com `allowArray`; `FunctionType` recursivo; o resto (tipos Dart
  como `int`, `dynamic`, parâmetro de tipo, `InvalidType`) é inválido. Um `VarArgs` que não está no
  fim não é achatado e falha (`VarArgs` não é tipo nativo válido); um registro com campos nomeados
  também não é achatado.
- **Posição:** (1)(2) token do nome da declaração; (3) nó do argumento de tipo (`Int32 Function(Int32)`
  len 21; `T` len 1) ou `fromFunction`; (4) 1º argumento de tipo (pode ser multilinha:
  `Int64 Function(Int64, VarArgs<(...)>)` len 68); (5) invocação inteira `Native.addressOf(foo)`;
  (6) `NativeCallable.listener` (len 23) ou `NativeCallable<void Function(int)>.listener` (len 43).
- **Casos do corpus (41):** 23 são o caso (1) com `'T'`: `@Native()` sem argumento de tipo
  (inferido `Native<dynamic>`; 16 arquivos `Native__Infer*`, `AddressOf__valid` (`foo2`),
  `AddressOf__invalid_MissingType2`, `AddressOf__invalid_NotAPreciseType2`), `@Native<IntPtr>()` em
  função (`Native__annotation_InvalidFieldType`, `Native__annotation_MissingType`,
  `FfiNative__annotation_FfiNative_noTypeArguments`) e `@a` com `const a = Native();`
  (`Native__annotation_MissingTypeConst`, `:6:14`). Os comentários `// [diag.x]` desses arquivos
  são do SDK novo (que infere a assinatura / diz `'IntPtr'`); o 3.6.2 diz `'T'` sempre.
  4 são (2) com tipo Dart dentro (`IntPtr Function(int)`, `double Function(IntPtr)`); 4 de
  `fromFunction` (`'dynamic'` quando há 2 argumentos de tipo para 1 parâmetro — o analyzer usa
  `dynamic`; `'InvalidType Function()'`; `'Function'` sem argumento de tipo, inferido pelo limite;
  `'T'` parâmetro de tipo de classe); 5 de `lookupFunction` (`'S'`/`'T'` por alias ou parâmetro,
  `VarArgs` com nomeado ou fora do fim); 1 de `Native.addressOf` (`'NativeType'`: o `T` inferido
  pelo limite); 4 de `NativeCallable` (`'Function'` inferido pelo limite; `'int Function(int)'`).
- **Supressões:** nenhuma além das guardas de aridade acima; `return` depois de emitir (não segue
  para `must_be_a_subtype`).
- **No DartForge:** nada. Precisa: tipo da anotação `@Native` (explícito em `m.type_args`, inferido
  via `instanciacao`, ou o tipo da const referida), instanciação de chamadas genéricas
  (`instanciacao(args.span.start)` dá os `typeArgumentTypes`; conferir que com nº errado de
  argumentos de tipo gravamos `dynamic` como o analyzer), tipo estático de
  `NativeCallable...(...)` com `T` inferido pelo limite `Function`, e exibição com alias.

##### `must_be_a_subtype` (perda 27: FN 27, FP 0, msg 0, pos 0)
- **Mensagem:** `ffi_code.g.dart:343` — "The type '{0}' must be a subtype of '{1}' for '{2}'."
- **Emissão (todos em `ffi_verifier.dart`):**
  - `_checkFfiNativeField` `:576-580`: `!_validateCompatibleNativeType(nativeToDart, type, ffiSignature, allowFunctions: true)`
    ⇒ no nome, `[type (Dart), ffiSignature, 'Native']` (ordem Dart→nativo!).
  - `_checkFfiNativeFunction` `:690-698`: `!_validateCompatibleFunctionTypes(nativeToDart, dartType, nativeType, nativeFieldWrappersAsPointer: true, permissiveReturnType: true)`
    ⇒ no nome, `[nativeType, dartType, 'Native']`.
  - `_validateAsFunction` `:1231-1238` (no nó inteiro, `[TPrime, F, 'asFunction']`) — sem casos.
  - `_validateFromFunction` `:1557-1564` (no argumento `f`, `[FT, T, 'fromFunction']`) e `:1589-1595`
    (exceptionalReturn `e` incompatível com o retorno `R`, no `e`, `[eType, R, 'fromFunction']`).
  - `_validateLookupFunction` `:1651-1658`: em `typeArguments[1]`, `[S, F, 'lookupFunction']`.
  - `_validateNativeAddressOf` `:1709-1716` (função: `!isAssignableTo(nativeType, targetFunctionType)`)
    e `:1734-1739` (campo: `nativeType` inferido por `_canonicalFfiTypeForDartType` quando `dynamic`),
    no nó inteiro.
  - `_validateNativeCallable` `:1783-1790` (no 1º argumento, que pode ser um `NamedExpression`!,
    `[funcType, typeArg, 'NativeCallable']`) e `:1815-1821` (isolateLocal com 2 args: no
    `(arguments[1] as NamedExpression).expression`, `[eType, natRetType, name]` onde `name` é
    `'isolateLocal'`).
- **Condição:** `_validateCompatibleFunctionTypes` (`:1250-1313`): ambos `FunctionType` não-`Function`,
  mesmo nº de posicionais (nativo achatado), sem genéricos/opcionais; retorno via
  `_validateCompatibleNativeType` na direção (ou nas duas com `permissiveReturnType`) e parâmetros na
  direção inversa. `_validateCompatibleNativeType` (`:1318-1379`): nativo int (ou cujo supertipo é
  `AbiSpecificInteger`) ⇒ Dart `int`; double ⇒ `double`; bool ⇒ `bool`; Void ⇒ (dartToNative: sempre;
  senão Dart `void`); Dart `void` com nativo não-Void ⇒ falso; Handle ⇒ (dartToNative: sempre;
  nativeToDart: `Object` ≤ Dart); duas interfaces ⇒ NativeFieldWrapperClass1 só com `Pointer<Void>`,
  typed data compatível, senão subtipo na direção; função Dart × `NativeFunction` só com
  `allowFunctions`; o resto falso (por isso `InvalidType` sempre falha).
- **Posição:** nome da declaração; `f`/`e`/`typeArguments[1]`; invocação inteira em `addressOf`; em
  `NativeCallable`, o argumento inteiro (`exceptionalReturn: 4`, len 20, quando ele vem primeiro).
- **Casos (27):** 9 `NativeCallable` (7 com `f` incompatível ou com o nomeado em 1º lugar —
  `isolateLocal(exceptionalReturn: 0)` dá `'int' ... 'Int32 Function(Int32)'`; 1 `'String'` vs `'Int32'`
  para `'isolateLocal'`); 6 `fromFunction` (4 com `InvalidType`: identificador indefinido ou prefixo
  usado como valor; 2 com alias `'T'`); 2 `addressOf`; 6 `@Native` função (incl. `Handle` de retorno
  com Dart `void`); 2 `@Native` campo (`'int'` vs `'Double'`; `'int Function()'` vs
  `'NativeFunction<Double Function()>'`); 2 `lookupFunction` (alias `'T'`/`'F'`; VarArgs achatado
  `'Int64 Function(Int64, VarArgs<(...)>)'` — o tipo é exibido **sem** achatar).
- **Supressões:** só depois de (1) passar `must_be_a_native_function_type` (que retorna antes) e das
  guardas de aridade; `return` após emitir em Native/fromFunction(f)/NativeCallable(f).
- **No DartForge:** nada. Precisa do mesmo núcleo do código anterior mais `is_subtype`/assignable,
  tipos estáticos dos argumentos (com `InvalidType` para o não resolvido — `UnitBodyTypes::tipos_invalidos`)
  e a ordem de origem dos argumentos (`ast::Arguments.args` mistura posicionais e nomeados na ordem
  escrita; `arguments[0]` do analyzer é o primeiro escrito, nomeado ou não). O tipo de um argumento
  nomeado é o da sua expressão; o span tem de cobrir `nome: valor`.

##### `non_positive_array_dimension` (perda 10: FN 10)
- **Emissão:** `_validateSizeOfAnnotation` `:2007-2022`, chamado por `_validateFieldsInCompound`
  (`:1505-1510`, campo `Array` de struct/union) e por `_checkFfiNativeField` (`:581-588`, campo
  `@Native` cujo tipo nativo é `Array`).
- **Condição:** para a 1ª anotação `@Array` (elemento = construtor de `Array` do ffi, `:2077-2082`),
  `arraySizeDimensions` (`:2093-2129`): `dimensions` (multi) ou `dimension1..5` não nulos; com
  `variableLength`, dimensão 0 prefixada e pulada. Cada `dimensions[i] <= 0` ⇒ erro.
- **Posição:** `getArgumentNodes()` (`:1996-2005`): se 1 argumento que é `ListLiteral`, seus elementos;
  senão os argumentos; nó `[i]` (ex. `-12` len 3; `-4` em `@Array.multi([1, 2, 3, -4, 5, 6])`);
  sem argumentos ⇒ a anotação.
- **Mensagem:** `ffi_code.g.dart:414` "Array dimensions must be positive numbers." (sem argumentos).
- **Supressões/ordem:** depende de ter achado ≥1 `@Array` (senão `missing_size_annotation_carray` e
  return). Sai depois de `size_annotation_dimensions` (não exclusivos).
- **No DartForge:** nada. Casos: 9 em campos de struct (`@Array(0)`, `@Array(-1)`, `.multi([...])`) e
  1 em campo `@Native()` de topo (`NativeField__Array_InvalidDimension`). Valores: avaliar os
  argumentos (literal, `-literal`, const) com o avaliador; não depender do valor do `_ArraySize`.

##### `subtype_of_struct_class` (perda 7: FN 7)
- **Códigos:** `SUBTYPE_OF_STRUCT_CLASS_IN_EXTENDS` / `_IN_IMPLEMENTS` / `_IN_WITH` (todos com
  nome `SUBTYPE_OF_STRUCT_CLASS`), `ffi_code.g.dart:463/476/489`: "The class '{0}' can't
  extend|implement|mix in '{1}' because '{1}' is a subtype of 'Struct', 'Union', or
  'AbiSpecificInteger'."
- **Emissão:** `visitClassDeclaration` `:145-152` (extends: superclasse que **não** é classe ffi e
  cujo `allSupertypes` contém Struct/Union ou AbiSpecificInteger) e `checkSupertype` `:156-169`
  (implements/with: mesma condição, exceto nomes `Allocator`/`Finalizable`).
- **Posição:** o `NamedType` (inclui prefixo/argumentos de tipo). Argumentos: `node.name.lexeme`,
  `typename.name2.lexeme` (só o identificador).
- **Supressões:** só `ClassDeclaration` (não mixin, enum, `class A = B with C`). Sai junto com
  `invalid_use_of_type_outside_library` (base class) e `empty_struct`.
- **No DartForge:** nada. Usa `ClassElement.supertype_class/interface_classes/mixin_classes` e a
  hierarquia (todos os supertipos). Casos: 2 extends, 3 implements (1 AbiSpecificInteger), 2 with.

##### `ffi_native_unexpected_number_of_parameters` (perda 6: FN 6)
- **Emissão:** `_checkFfiNativeFunction` `:645-654` (declaração estática/topo).
- **Condição:** `formalParameters.length != ffiSignature.normalParameterTypes.flattenVarArgs().length`.
- **Posição:** token do nome. **Mensagem** `ffi_code.g.dart:195`: "Unexpected number of Native
  annotation parameters. Expected {0} but has {1}." com `{0}` = nº do nativo (achatado), `{1}` = nº de
  parâmetros formais (todos, incl. opcionais).
- **Ordem:** antes do teste de validade da assinatura; `return` (não sai `must_be_a_native_function_type`).
  `isLeaf` + Handle (`leaf_call_*`) é checado antes (`:606-610`).
- **No DartForge:** nada. Casos: 4 simples + 2 com `VarArgs<(Int32, Double)>` (achatar o registro).

##### `address_position` (perda 5: FN 5)
- **Emissão:** `_validateAddressPosition` `:1042-1062`, via `visitPrefixedIdentifier` (`:381-384`) /
  `visitPropertyAccess` (`:400-403`) quando o elemento é o getter `address` de uma extensão
  `*Address` do ffi.
- **Condição:** sobe um nível se o pai é `.cast()` de `Pointer`; o pai tem de ser `ArgumentList` de
  `MethodInvocation` cujo elemento é função/método com `@Native(isLeaf: true)` (`isNativeLeaf`,
  `:2140-2205`, lê o campo `isLeaf` da constante). Senão erro.
- **Posição:** o identificador `address` (len 7). **Mensagem** `ffi_code.g.dart:69` (sem correção).
- **No DartForge:** nada. Casos: 5, todos `myNonLeafNative(x.address.cast())` com `@Native<...>()`
  sem `isLeaf`. Precisa: resolução de `.address` para `Resolved::ExtensionMember` de extensão do
  ffi; contexto "argumento direto de chamada a nativa leaf" (o AST não tem pai: marcar de cima para
  baixo os argumentos — e o receptor de `.cast()` de `Pointer` neles); `isLeaf` lido do argumento
  nomeado da anotação (literal ou const). Risco de FP: os 5 arquivos válidos de
  `ffi_address_of_cast` usam `isLeaf: true`.

##### `missing_size_annotation_carray` (perda 5: FN 5)
- **Emissão:** `_validateSizeOfAnnotation` `:1956-1961` quando não há `@Array` do ffi.
- **Posição:** campo de struct ⇒ o `TypeAnnotation` do campo (`Array<Int8>`, len 11); campo
  `@Native` ⇒ o token do nome (`NativeField__Array_MissingAnnotation`, `field`).
- **Mensagem:** `ffi_code.g.dart:318`.
- **Supressões:** `@Array.variableWithVariableDimension(...)` não existe no 3.6.2 ⇒ `invalid_annotation`
  e o elemento é nulo ⇒ conta como ausente (3 casos `InlineArray__variableWithVariableDimens_*`).
- **No DartForge:** nada; 4 em struct, 1 em campo nativo.

##### `abi_specific_integer_mapping_unsupported` (perda 4: FN 4)
- **Emissão:** `_validateAbiSpecificIntegerMappingAnnotation` `:999-1037`, de `visitClassDeclaration`
  `:140-144` (classe que estende `AbiSpecificInteger` diretamente).
- **Condição:** argumento `SetOrMapLiteral` ⇒ cada `MapLiteralEntry` cujo `value.staticType` é
  interface de nome fora de `Int8..Uint64` (`:1000-1016`, reporta no `element.value`); senão (ex. `@AbiSpecificIntegerMapping(c)`)
  avalia `mapping` da constante e reporta cada valor inválido em `arguments.first` (`:1019-1037`).
- **Mensagem:** `ffi_code.g.dart:58`, `{0}` = nome da classe (`'IntPtr'`, `'UintPtr'` — no 3.6.2
  `IntPtr` é `AbiSpecificInteger`, e `UintPtr` pode ser a própria classe do usuário).
- **Posição:** `IntPtr()` (len 8) / `UintPtr()`; no caso da const, `c` (len 1) duas vezes, com
  mensagens diferentes (não são duplicatas).
- **No DartForge:** nada. Literal: tipos estáticos dos valores do mapa (inferidos em
  `anotacao_sem_validar`). Const: avaliar `c` (biblioteca do usuário ⇒ `Estado::Mapa`, `Valor.tipo`
  de cada valor) — não depende do construtor opaco.

##### `empty_struct` (perda 4: FN 4)
- **Emissão:** `visitClassDeclaration` `:130-136` (classe que estende Struct/Union diretamente e
  `isEmptyStruct`), e `_validateFieldsInCompound` `:1511-1519` (campo cujo tipo é compound vazio:
  no `FieldDeclaration` inteiro, `[clazz.name, clazz.supertype.getDisplayString()]`).
- **Condição `isEmptyStruct`** (`:2380-2398`): nenhum `field` (inclui estáticos e sintéticos) com
  tipo `int`/`double`/`bool`/`Pointer`/compound/`Array`.
- **Posição:** nome da classe. **Mensagem** `ffi_code.g.dart:134` `[nome, 'Struct'|'Union']`.
- **No DartForge:** nada. Os 4 casos estão em `subtype_of_struct_class/*` (`final class S extends Struct {}`).

##### `size_annotation_dimensions` (perda 4: FN 4)
- **Emissão:** `_validateSizeOfAnnotation` `:1975-1984`: nº de dimensões da 1ª `@Array` ≠
  `arrayDimensions` do tipo (nº de `Array<` aninhados, `:2433-2443`).
- **Posição:** a anotação inteira (com `@`). **Mensagem** `ffi_code.g.dart:453`.
- **No DartForge:** nada. 3 em struct (`@Array(8, 8)` em `Array<Array<Array<Uint8>>>` etc.), 1 em
  campo `@Native()` (`@Array(10, 20)` em `Array<IntPtr>`). Basta contar argumentos (ou elementos do
  `multi`) — não precisa dos valores.

##### `ffi_native_unexpected_number_of_parameters_with_receiver` (perda 3: FN 3)
- **Emissão:** `_checkFfiNativeFunction` `:615-627`: `MethodElement` (ou getter) **não estático** —
  inclui método de extensão — com `formalParameters.length + 1 != ffiParameterTypes.length`.
- **Mensagem** `ffi_code.g.dart` (`FFI_NATIVE_UNEXPECTED_NUMBER_OF_PARAMETERS_WITH_RECEIVER`):
  `[formais + 1, nativos]` ("Expected 2 but has 1." — note a ordem inversa da do código sem receptor).
- **Posição:** nome do método. **No DartForge:** nada; 2 em classe, 1 em `extension on int`.

##### `invalid_exception_value` (perda 3: FN 3)
- **Emissão:** `_validateFromFunction` `:1568-1579` e `_validateNativeCallable` (isolateLocal)
  `:1794-1805`: retorno nativo Void/Pointer/Handle/compound e há 2º argumento.
- **Posição:** `arguments[1]` como escrito (`42`; `0` posicional; `exceptionalReturn: 4` inteiro, len 20).
- **Mensagem** `ffi_code.g.dart:238`, `{0}` = `'fromFunction'` ou o nome do construtor (`isolateLocal`).
- **No DartForge:** nada. Sai junto com `extra_positional_arguments_could_be_named` no caso `(f, 0)`.

##### `invalid_field_type_in_struct` (perda 3: FN 3)
- **Emissão:** `_validateFieldsInCompound` `:1459-1464` (tipo anulável) e `:1520-1526` (tipo que não
  é int/double/bool/Pointer/Array/compound).
- **Posição/mensagem:** o `TypeAnnotation`; `{0}` = `fieldType.toSource()` (texto: `'String'`,
  `'Pointer?'`). `ffi_code.g.dart:249`.
- **Supressões:** campos `static` são pulados (`:1437`); só em classe que estende Struct/Union diretamente.
- **No DartForge:** nada; texto do span do tipo.

##### `native_field_invalid_type` (perda 3: FN 3)
- **Emissão:** `_checkFfiNative` `:484-491` (campo/variável com `@Native<FunctionType>`) e
  `_checkFfiNativeField` `:589-595` (tipo nativo compatível mas `Handle` ou `NativeFunction`).
- **Posição:** nome; **mensagem** `ffi_code.g.dart:362`, `{0}` = o tipo nativo (`'IntPtr Function(IntPtr)'`,
  `'NativeFunction<Void Function()>'`, `'Handle'`).
- **No DartForge:** nada. O caso `NativeFunction` precisa do ramo `allowFunctions` da compatibilidade;
  o `Handle` com `Object` passa pela regra nativeToDart (`Object` ≤ Dart).

##### `annotation_on_pointer_field` (perda 2: FN 2)
- **Emissão:** `_validateNoAnnotations` `:1843-1852` para campo de compound cujo tipo é `Pointer`:
  toda anotação cujo elemento é de classe ffi (`ffiClass != null`).
- **Posição:** a anotação inteira (`@Int32()` len 8). Msg `ffi_code.g.dart:86`.
- **No DartForge:** nada.

##### `creation_of_struct_or_union` (perda 2: FN 2)
- **Emissão:** `visitInstanceCreationExpression` `:276-291`: classe do construtor cujo supertipo
  **direto** é Struct/Union e construtor não-factory.
- **Posição:** `node.constructorName` (`A`, len 1; `A.nome` se nomeado; inclui `new`? não — só o
  nome do construtor). Msg `ffi_code.g.dart:122`.
- **No DartForge:** nada; `A()` é `ExprKind::Call` resolvido como `Resolved::Constructor`.

##### `ffi_native_invalid_duplicate_default_asset` (perda 2: FN 2)
- **Emissão:** `visitLibraryDirective` `:293-316`: 2ª+ anotação da biblioteca cujo valor constante
  tem tipo `DefaultAsset` do ffi.
- **Posição:** `annotationAst.name` (sem `@`: `DefaultAsset` len 12, `defaults` len 8 para `@defaults`
  const). Msg `ffi_code.g.dart:160`. Só com diretiva `library`.
- **No DartForge:** nada; `ast::Directive.metadata` da diretiva `library;`; tipo da const pelo outline.

##### `ffi_native_invalid_multiple_annotations` (perda 2: FN 2)
- **Emissão:** `_checkFfiNative` `:455-462`: 2ª anotação com valor do tipo `Native`; `break`.
- **Posição:** `annotationAst.name` (`Native` len 6; `duplicate` len 9). Msg `ffi_code.g.dart:168`.
- **Ordem:** a 1ª anotação já foi processada inteira (os outros erros dela saem).
- **No DartForge:** nada.

##### `field_must_be_external_in_struct` (perda 2: FN 2)
- **Emissão:** `_validateFieldsInCompound` `:1444-1449`: campo não estático sem `external`.
- **Posição:** nome da 1ª variável. Msg `ffi_code.g.dart:219`. **No DartForge:** nada.

##### `generic_struct_subclass` (perda 2: FN 2)
- **Emissão:** `visitClassDeclaration` `:184-191` (`inCompound` e classe com parâmetros de tipo).
- **Posição:** nome da classe; `{0}` = nome. Msg `ffi_code.g.dart:228`. **No DartForge:** nada.

##### `mismatched_annotation_on_struct_field` (perda 2: FN 2)
- **Emissão:** `_validateAnnotations` `:1147-1191`, para campos `int`/`double`/`bool`: entre as
  anotações ffi (ou de construtor de subclasse de `AbiSpecificInteger`), a primeira cujo tipo
  (`_typeForAnnotation`, `:937-953`) bate é a requerida; as outras são extras. Sem requerida: a 1ª
  extra vira `MISMATCHED`, o resto `EXTRA`.
- **Posição:** a anotação inteira (`@Double()` len 9). Msg `ffi_code.g.dart:277`. **No DartForge:** nada.

##### `missing_exception_value` (perda 2: FN 2)
- **Emissão:** `_validateFromFunction` `:1580-1585` (no `methodName`, `'fromFunction'`) e
  `_validateNativeCallable` `:1806-1811` (na **expressão inteira** `NativeCallable<...>.isolateLocal(f)`,
  len 53, `{0}` = `'isolateLocal'`): retorno nativo não Void/Pointer/Handle/compound e só 1 argumento.
- Msg `ffi_code.g.dart:299`. **No DartForge:** nada.

##### `packed_annotation_alignment` (perda 2: FN 2)
- **Emissão:** `_validatePackedAnnotation` `:1873-1886` (só para `extends Struct`): `memberAlignment`
  da 1ª `@Packed` fora de {1,2,4,8,16} (inclui nulo, `@Packed()` sem argumento).
- **Posição:** `arguments[0]` (`3`) ou a anotação inteira (`@Packed()` len 9). Msg `ffi_code.g.dart:444`.
- **No DartForge:** nada; o valor vem do argumento (avaliador), não do campo (`Packed` é opaco).
  `@Packed()` também dá `not_enough_positional_arguments` (já emitimos?).

##### `abi_specific_integer_mapping_extra` (perda 1) / `abi_specific_integer_mapping_missing` (perda 1)
- **Emissão:** `_validateAbiSpecificIntegerMappingAnnotation` `:974-990`. Missing: nenhuma
  `@AbiSpecificIntegerMapping` (construtor ffi) ⇒ no nome da classe, return. Extra: cada uma
  depois da 1ª ⇒ em `annotation.name` (sem `@`, len 25).
- Msgs `ffi_code.g.dart:47` / `:37`. **No DartForge:** nada.

##### `extra_annotation_on_struct_field` (perda 1)
- Ver `mismatched_annotation_on_struct_field` (`:1175-1180`); posição: a anotação extra inteira
  (`@Int16()` após `@Int32()`). Msg `ffi_code.g.dart:143`.

##### `extra_size_annotation_carray` (perda 1)
- `_validateSizeOfAnnotation` `:1964-1972`: cada `@Array` depois da 1ª, na anotação inteira.
  Msg `ffi_code.g.dart:152`.

##### `ffi_native_must_be_external` (perda 1)
- `_checkFfiNative` `:466-471`: declaração com `@Native` sem `external`; no nome (`field`). O caso é
  uma variável de topo `@Native<IntPtr>() int field;` (sai junto com
  `not_initialized_non_nullable_variable`). Msg `ffi_code.g.dart:176`.

##### `missing_annotation_on_struct_field` (perda 1)
- `_validateAnnotations` `:1181-1190`: nenhuma anotação ffi; no `TypeAnnotation`;
  `[errorNode.type (DartType 'int'), superclasse escrita ('Struct'|'Union')]`. Msg `ffi_code.g.dart:289`.

##### `missing_field_type_in_struct` (perda 1)
- `_validateFieldsInCompound` `:1451-1456`: `fields.type == null` (`external var str;`), no nome.
  Msg `ffi_code.g.dart:309`.

##### `native_field_missing_type` (perda 1)
- `_checkFfiNativeField` `:552-561`: `@Native()` (tipo `dynamic`) em campo cujo tipo Dart não é
  Pointer/compound/Array (`_canonicalFfiTypeForDartType`, `:426-432`); no nome. Msg `ffi_code.g.dart:373`.

##### `native_field_not_static` (perda 1)
- `_checkFfiNativeField` `:528-534` (campo de instância; continua a checagem) e `:544-549`; no nome.
  Msg `ffi_code.g.dart:384`.

##### `packed_annotation` (perda 1)
- `_validatePackedAnnotation` `:1863-1871`: cada `@Packed` depois da 1ª, na anotação inteira.
  Msg `ffi_code.g.dart:436`.

---

#### Ordem de implementação proposta (o que dá mais casos com menos infraestrutura)

0. **Infra mínima** (`types/src/ffi.rs`): `e_ffi(ClassId)` por nome + `uri == "dart:ffi"`;
   `supertipo_direto_e(Struct|Union|AbiSpecificInteger)`; resolução de `ast::Annotation` →
   `{classe ffi, construtor, tipo instanciado (de m.type_args, de instanciacao(args.span.start), ou
   o tipo da const), argumentos}` reaproveitando o resolvedor de `inferencia/funcoes.rs:1043-1095`;
   leitura de argumento inteiro/bool constante (literal, `-literal`, const do usuário via avaliador).
1. **`@Native` nas declarações, sem validar tipos nativos — ~40 casos:** `'T'` quando o argumento
   de `Native<…>` não é função em função/método (23), contagem de parâmetros com e sem receptor (9,
   achatando `VarArgs<(…)>`), `ffi_native_must_be_external` (1), `invalid_multiple_annotations` (2),
   `native_field_not_static` (1), `native_field_missing_type` (1), `native_field_invalid_type` com
   tipo função (1). Só exige tipo da anotação e a forma da declaração.
2. **Classes struct/union/AbiSpecificInteger e seus campos — 38 casos, só outline + anotações:**
   `empty_struct` 4, `generic_struct_subclass` 2, `subtype_of_struct_class` 7,
   `field_must_be_external` 2, `missing_field_type` 1, `invalid_field_type` 3, anotações de campo
   (`mismatched` 2, `extra` 1, `missing` 1, `on_pointer` 2), `packed` 1 + `alignment` 2, mapeamento
   ABI (`missing` 1, `extra` 1, `unsupported` 4), `creation_of_struct_or_union` 2,
   `duplicate_default_asset` 2.
3. **`@Array` — 20 casos:** `missing_size` 5, `extra_size` 1, `size_dimensions` 4,
   `non_positive` 10 (3 deles em campos `@Native`, reaproveitando a etapa 1).
4. **Predicados de tipo nativo + compatibilidade (`_isValidFfiNativeType`,
   `_validateCompatible*`)** e então os usos — **~55 casos:** assinatura inválida em `@Native` (4),
   `must_be_a_subtype` de `@Native` (8), `native_field_invalid_type` Handle/NativeFunction (2);
   chamadas: `Pointer.fromFunction` (12), `lookupFunction` (7), `NativeCallable.*` (16),
   `Native.addressOf` (3), e por fim `address_position` (5, precisa do contexto de argumento e do
   `isLeaf`). Aqui a exibição de tipos com alias e o `InvalidType`/`dynamic` dos argumentos de tipo
   errados têm de estar iguais aos do analyzer.

Total coberto: 153 (= perda do grupo). Outros `FfiCode` do corpus que caem de graça com a mesma infra
(fora deste grupo): `argument_must_be_a_constant` 9, `leaf_call_must_not_*` 12, `argument_must_be_native` 3,
`non_constant_type_argument` 3, `non_sized_type_argument` 2, `must_return_void` 1,
`non_native_function_type_argument_to_pointer` 1. Risco de FP: ~230 arquivos FFI no corpus
`analyzer` (`ffi_native` 93, `ffi_async_callback` 26, `ffi_array` 17…), muitos válidos — em especial
`@Native()` em campo `Pointer`/compound (válido) e chamadas leaf com `.address`.
##### `invalid_field_name` (perda 33: FN 33, FP 0, msg 0, pos 0)
- **Emissão:** três códigos com `sharedName: INVALID_FIELD_NAME` (`analyzer/messages.yaml:8567,8638,8644`):
  - tipo de registro escrito: `RecordTypeAnnotationResolver.reportInvalidFieldNames`
    (`analyzer/lib/src/dart/resolver/record_type_annotation_resolver.dart:61-96`), chamado por `resolve`
    (`:98-102`) a partir de `ResolutionVisitor.visitRecordTypeAnnotation`
    (`analyzer/lib/src/dart/resolver/resolution_visitor.dart:1170-1176`) — fase de resolução de tipos
    escritos, para TODO `RecordTypeAnnotation` da unidade (assinaturas, corpos, argumentos de tipo…);
  - literal de registro: `RecordLiteralResolver._reportInvalidFieldNames`
    (`analyzer/lib/src/dart/resolver/record_literal_resolver.dart:97-131`), chamado por `resolve` (`:27-35`)
    na inferência de corpos (`ResolverVisitor`), para todo literal resolvido.
- **Condição exata:**
  ```
  tipo:    positionalCount = node.positionalFields.length
           para cada campo (posicional ou nomeado) com nome `name`:
             se name começa com '_':  PRIVATE, salvo isPositionalWildCard (campo posicional, name=='_'
                                       e wildcard-variables ligado — NÃO no 3.6.2, o recurso é 3.7)
             senão se positionalFieldIndex(name) = i (regex `\$[1-9]\d*`, i = n-1) e
                      i < positionalCount e positionalFields.indexOf(campo) != i:  POSITIONAL
             senão se name ∈ {hashCode, runtimeType, noSuchMethod, toString}:     FROM_OBJECT
  literal: positionalCount = nº de campos que não são NamedExpression
           para cada NamedExpression (só os nomeados):
             '_…' → PRIVATE; `$i` com i < positionalCount → POSITIONAL; nome de Object → FROM_OBJECT
  ```
  (`RecordTypeExtension.positionalFieldIndex`, `analyzer/lib/src/dart/element/extensions.dart:221-231`;
  `isForbiddenNameForRecordField`, `record_literal_resolver.dart:195-204`.) No tipo, o nome do campo
  POSICIONAL também conta (`(int _a, int b)`, `(int $2, int b)` com $2 no índice 0 ≠ 1, `(int hashCode)`).
- **Posição:** o token do nome do campo (tipo: `atToken(nameToken)`; literal: `atNode(field.name.label)`,
  o identificador antes do `:`).
- **Mensagem:** sem argumentos. PRIVATE "Record field names can't be private." / POSITIONAL "Record field
  names can't be a dollar sign followed by an integer when the integer is the index of a positional
  field." / FROM_OBJECT "Record field names can't be the same as a member from 'Object'." (correções em
  `messages.yaml:8570,8641,8647`).
- **Supressões e ordem:** nenhuma; a duplicata (`duplicate_field_name`) é relatada antes no mesmo
  `resolve` e as duas saem juntas (`({int _, int _})` dá 2× PRIVATE + 1× DUPLICATE). Sem dependência de
  tipos: é puramente sintático.
- **No DartForge:** não existe (grep `INVALID_FIELD_NAME` só na tabela). As 33 amostras são todas desta
  regra (tipos e literais, inclusive os 6 `_` posicionais dos testes de `duplicate_field_name`).
  Obstáculo: `ast::TypeKind::Record { positional: Box<[TypeId]>, named: Box<[(Name, TypeId)]> }`
  (`crates/frontend/src/ast.rs:637`) perde o NOME dos campos posicionais; é preciso guardá-lo no AST
  (`positional: Box<[(Option<Name>, TypeId)]>`) ou relê-lo da fonte depois do span do tipo. Lugar natural:
  um verificador novo em `crates/analise` que percorre `ast.types` (todo tipo escrito) e as expressões
  `ExprKind::Record` de `ast.exprs`.

##### `duplicate_field_name` (perda 7: FN 7)
- **Emissão:** `RecordTypeAnnotationResolver.reportDuplicateFieldDefinitions`
  (`record_type_annotation_resolver.dart:40-58`) e `RecordLiteralResolver._reportDuplicateFieldDefinitions`
  (`record_literal_resolver.dart:78-93`), mesmos caminhos de chamada de `invalid_field_name`; o erro vem de
  `DiagnosticFactory.duplicateFieldDefinitionInType/InLiteral`
  (`analyzer/lib/src/diagnostic/diagnostic_factory.dart:98-147`).
- **Condição:** tipo: todos os campos com nome (posicionais E nomeados), na ordem; um nome já visto é
  duplicata (salvo `_` posicional com wildcards, desligado no 3.6.2). Literal: só os nomeados.
- **Posição:** o nome do campo repetido (o segundo, terceiro…); contexto no primeiro.
- **Mensagem:** "The field name '{0}' is already used in this record." `{0}` = lexema
  (`messages.yaml:3956`).
- **No DartForge:** não existe. Mesmo verificador e mesmo obstáculo (nome posicional) de
  `invalid_field_name`. Todas as 7 amostras são desta forma (`(int a, int a)`, `(int a, {int a})`,
  `({int a, int a})`, `(a: 1, a: 2)`, `(int _, int _)`).
##### `missing_default_value_for_parameter` (perda 27: FN 27 — 25 no corpus 3.6.2, 2 de sintaxe nova)
- **Emissão:** `ErrorVerifier._checkUseOfDefaultValuesInParameters`
  (`analyzer/lib/src/generated/error_verifier.dart:6180-6250`), chamado por `visitFormalParameterList`
  (`:938`) — toda lista de parâmetros (funções de topo, métodos, construtores, funções locais E
  expressões de função/closures).
- **Condição exata:**
  ```
  esperado = pai é ConstructorDeclaration: !external && !(factory && redirectedConstructor != null)
           | pai é FunctionExpression: !(FunctionDeclaration external) && corpo não é native
           | pai é MethodDeclaration: !isAbstract && !external && corpo não é native
           | outro (tipo de função, parâmetro-função) → false
  para cada DefaultFormalParameter p (opcional posicional ou nomeado):
    se p é `required` nomeado: (default → DEFAULT_VALUE_ON_REQUIRED_PARAMETER, no nome)
    senão se esperado e !p.element.hasDefaultValue e isPotentiallyNonNullable(p.element.type):
      se p.element.hasRequired (anotação @required): ..._WITH_ANNOTATION
      senão se !(super formal posicional `_` com wildcards — desligado no 3.6.2):
        POSITIONAL (p posicional) / MISSING_DEFAULT_VALUE_FOR_PARAMETER (nomeado), arg = nome
  ```
  `hasDefaultValue` (`analyzer/lib/src/dart/element/element.dart:8226` = `defaultValueCode != null`):
  para `super.x` (`DefaultSuperFormalParameterElementImpl.defaultValueCode`, `element.dart:1733-1748`):
  o default escrito; senão, SE o valor constante do parâmetro associado no construtor da superclasse
  existe e o tipo dele é subtipo do tipo (erasure) do `super.x` (`_superConstructorParameterDefaultValue`,
  `:1764-1785`), o `defaultValueCode` DESSE parâmetro (recursivo; um `required super.a` intermediário
  tem `defaultValueCode == null` mesmo herdando valor — por isso `C({super.a})` sobre `B({required
  super.a})` sobre `A({this.a = 0})` é erro). O tipo de `super.x` sem tipo escrito é o do parâmetro
  associado.
- **Posição:** o nome do parâmetro (`_parameterName`, `:6426`).
- **Mensagem:** "The parameter '{0}' can't have a value of 'null' because of its type, but the implicit
  default value is 'null'." `{0}` = lexema do nome (`messages.yaml`, MISSING_DEFAULT_VALUE_FOR_PARAMETER
  e _POSITIONAL têm o mesmo texto; _WITH_ANNOTATION sem argumento).
- **Supressões:** tipo potencialmente anulável (inclui parâmetro de tipo sem limite não-nulo,
  `dynamic`, `FutureOr<T?>`), tipo inválido (InvalidType não é potencialmente não-nulo).
- **No DartForge:** `crates/types/src/sobrescritas.rs` `valores_padrao` (≈:1254-1330), que percorre só
  `program.functions` (funções e membros declarados) com tipos do outline. Causas dos FN:
  1. **closures** (`var f = ([int a]) {};`, 2 casos): expressões de função não estão em
     `program.functions`; precisa da passada de corpos (tipos dos parâmetros de closure na inferência).
  2. **`super.x`** (≈6 casos): `p.super_` é pulado; falta a regra de herança do default e do tipo acima.
  3. **`augment …` sem o experimento** (≈14 casos): a função descarta declarações cuja linha (ou a da
     classe) começa com `augment `; o analyzer, sem o experimento, recupera `augment void f([int a]) {}`
     /`augment C([int a]) {}` como outra declaração e relata normalmente. Precisa conferir com a sonda a
     forma que o nosso parser dá a essas linhas antes de tirar o filtro.
  4. 2 de sintaxe nova (construtor primário `class C([int x])` e o teste de inferência de override).

##### `private_optional_parameter` (perda 21: FN 21, todos de sintaxe nova → oráculo 3.13.4)
- **Emissão (3.6.2):** `ErrorVerifier._checkForPrivateOptionalParameter`
  (`analyzer/lib/src/generated/error_verifier.dart:5025-5040`), de `visitFieldFormalParameter`/
  `visitSimpleFormalParameter`… para todo parâmetro nomeado cujo nome (não sintético) começa com `_`.
  Posição: o token do nome. Mensagem: "Named parameters can't start with an underscore." (sem args).
- **Amostras:** as 21 são parâmetros DECLARANTES privados de construtor primário
  (`class A({final int _p = 0})`, `class C1({required final String _foo})`), em arquivos com o
  experimento `primary-constructors` DESLIGADO (o oráculo 3.13.4 dá `experiment_not_enabled` no
  cabeçalho e, mesmo assim, `private_optional_parameter` no nome e `unused_field` no campo).
- **No DartForge:** emitido no parser, `crates/frontend/src/parser/types.rs:800-815`
  (`nome_publico_do_nomeado`): só relata quando `!(this_ || declarante && em_construtor_primario &&
  features.tem(PrimaryConstructors))`. Causa provável do FN: nessas bibliotecas o parser liga
  `PrimaryConstructors` para conseguir ler a sintaxe (o 3.13 lê e só reclama do experimento), e a
  condição então trata o declarante como "inicializa campo". A regra deve usar o recurso **habilitado
  pela biblioteca** (não o usado para recuperar) — conferir com a sonda num `class A({final int _p})`.
##### `invalid_use_of_type_outside_library` (perda 20: FN 20)
- **Emissão:** é o `sharedName` de BASE_/FINAL_/INTERFACE_/SEALED_CLASS_…_OUTSIDE_OF_LIBRARY. Os do
  ErrorVerifier: `_checkForFinalSupertypeOutsideOfLibrary`
  (`analyzer/lib/src/generated/error_verifier.dart:3654-3740`), `_checkForBaseClassOrMixinImplementedOutsideOfLibrary`,
  `_checkForInterfaceClassOrMixinSuperclassOutsideOfLibrary`, `_checkForSealedSupertypeOutsideOfLibrary`,
  chamados de `_checkClassInheritance`/`_checkMixinInheritance` (visitClassDeclaration, ClassTypeAlias,
  MixinDeclaration).
- **Condição:** sobre o TIPO resolvido de cada `NamedType` da cláusula (`superclass.type` é
  `InterfaceType` — um `typedef X = FinalClass;` usado em `extends X` dá o `InterfaceType` da classe):
  `element.isFinal && !isSealed && element.library != _currentLibrary && !_mayIgnoreClassModifiers(lib)`
  (extends/with/on); em `implements`, também os supertipos indiretos finais, mas só se o tipo direto vem
  de uma biblioteca sem `class_modifiers`; análogos para base/interface/sealed.
- **Posição:** o `NamedType` da cláusula inteiro (`atNode(superclass)`), ou seja o nome do ALIAS como
  escrito (`FinalClassTypeDef`), com prefixo e argumentos de tipo se houver.
- **Mensagem:** p.ex. "The class '{0}' can't be extended outside of its library because it's a final
  class." `{0}` = `element.name` da CLASSE (não do alias: `'FinalClass'`, `'Function'`).
- **No DartForge:** `crates/analise/src/modificadores.rs` resolve cada tipo de cláusula pelo nome no
  escopo da biblioteca e só aceita `Element::Class`; as 20 amostras são TODAS por alias de tipo:
  `typedef XTypeDef = FinalClass/BaseClass/InterfaceClass/SealedClass` (testes `*_typedef_*`) e
  `typedef F = Function` / `class A extends Function` (`deprecated_extends_function`,
  `nonfunction_type_aliases/usage_function_error_test`). Correção: seguir o alias (`Element::Typedef`
  cujo alvo, sem argumentos ou com, é uma classe) até a classe, mantendo o span do nome escrito e o nome
  da classe na mensagem; conferir `Function` (classe `final` de `dart:core`) e a regra
  `_mayIgnoreClassModifiers` (SDK + biblioteca < 3.0).

##### `invalid_language_version_override` (perda 19: FN 19; 2 acertos)
- **Emissão:** `LanguageVersionOverrideVerifier.verify`
  (`analyzer/lib/src/error/language_version_override_verifier.dart`), chamado por
  `LibraryAnalyzer._computeVerifyErrors`/hints (`analyzer/lib/src/dart/analysis/library_analyzer.dart:496`)
  para toda unidade. São 9 códigos `WarningCode.INVALID_LANGUAGE_VERSION_OVERRIDE_*` com `sharedName`
  INVALID_LANGUAGE_VERSION_OVERRIDE (`messages.yaml:24203-24290`); o _GREATER vem do scanner
  (`analyzer/lib/src/dart/scanner/scanner.dart:166-190`) e é o único que emitimos
  (`crates/elements/src/load.rs:949`).
- **Condição (`_findLanguageVersionOverrideComment`, só nos comentários ANTES do primeiro token, depois
  do `#!`):** percorre cada comentário; pula as `/` iniciais (conta `slashCount`), espaços (tab/espaço),
  `@` opcional, exige 4 letras que em minúsculas são `dart`, espaços, um separador (sequência de
  caracteres que não são dígito/letra/espaço, pode ser vazia), espaços, uma letra opcional (prefixo),
  um dígito, e o caractere seguinte não pode ser letra; sem `@` e sem separador → não é override. Daí,
  na ordem: `slashCount > 2` → TWO_SLASHES; sem `@` → AT_SIGN; `dart` com maiúscula → LOWER_CASE;
  separador ≠ exatamente `=` → EQUALS; letra antes do número → PREFIX; sem `.` depois dos dígitos →
  NUMBER; resto não-espaço depois de `N.N` → TRAILING_CHARACTERS; válido → para de procurar. Cada erro
  retorna false e o laço segue para o próximo comentário (pode haver vários).
  `_verifyMisplaced`: para todo token DEPOIS do primeiro token significativo (primeira diretiva ou
  declaração), todo comentário que casa `^\s*//\s*@dart\s*=\s*\d+\.\d+` dá LOCATION.
- **Posição:** comentário inteiro (offset e lexema completo) nos casos de formato; em LOCATION, de `@dart`
  até o fim do casamento da regex (`// @dart = 3.0` → `@dart = 3.0`).
- **Mensagem:** texto fixo de cada variante (sem argumentos), p.ex. LOCATION "The language version
  override must be specified before any declaration or directive."
- **No DartForge:** não existe (só o _GREATER). As 19 amostras: 4 LOCATION (depois de classe, de
  diretiva, dentro de classe), e os de formato (`// dart = 2.0`, `// @dart 2.0`, `// @dart >= 2.0`,
  `// @Dart = 2.0`, `/// @dart = 2.0`, `// dart @ 2.0`). Implementação puramente léxica: precisa dos
  comentários com offset (o lexer do frontend) — um verificador novo em `crates/analise` (o código já
  está em `verificados.txt`; nada de FP).
##### `wrong_explicit_type_parameter_variance_in_superinterface` (perda 14: msg 14) e `wrong_type_parameter_variance_in_superinterface` (perda 13: msg 13)
- **Emissão:** `ErrorVerifier._checkForWrongTypeParameterVarianceInSuperinterfaces`
  (`analyzer/lib/src/generated/error_verifier.dart:5926-5975`), chamado de visitClassDeclaration,
  visitClassTypeAlias, visitEnumDeclaration, visitMixinDeclaration.
- **Condição:** para cada supertipo direto (`supertype`, depois `interfaces`, depois `mixins`) e cada
  parâmetro de tipo `X` da classe: `superVariance = X.computeVarianceInType(S)`; se
  `!superVariance.greaterThanOrEqual(X.variance)`: com variância explícita (`in`/`out`/`inout`, experimento
  `variance`) → WRONG_EXPLICIT_… `[X.name, X.variance.keyword, superVariance.keyword, S]`; legado →
  WRONG_TYPE_PARAMETER_VARIANCE_IN_SUPERINTERFACE `[X.name, S]`.
- **Posição:** o nome do parâmetro de tipo (`atElement(typeParameter)`). Acertamos todas.
- **Mensagem:** `'{0}' is an '{1}' type parameter and can't be used in an '{2}' position in '{3}'.` /
  `'{0}' can't be used contravariantly or invariantly in '{1}'.` O `{3}`/`{1}` é o `DartType` do
  supertipo, formatado pelo `ErrorReporter` com `getDisplayString(preferTypeAlias: true)`: os
  argumentos que foram escritos com um `typedef` aparecem PELO ALIAS, inclusive aninhados
  (`'Covariant<CovFunction<T>>'`, `'A<F2<X>>'`, `'MultiThree<CovFunction<U>, CovFunction<T>,
  CovFunction<U>>'`), porque o tipo resolvido carrega `alias` (element + argumentos).
- **No DartForge:** `crates/types/src/variancia.rs` (≈:296-330) formata `dados.supertype`/`interfaces`/
  `mixins` do outline com `despejo::formatar`, mas esses tipos vêm de `resolve_classes_and_hierarchy`
  (`crates/types/src/resolve.rs:550`) sem a decoração `Exibicao::Alias` nos argumentos — daí
  `'Covariant<T Function()>'`. Todas as 27 divergências são isto. Correção: na resolução dos tipos das
  cláusulas, decorar os argumentos escritos com alias (como `limites.rs::resolver` já faz com
  `table.decorar(r, Exibicao::Alias{..})`), ou reresolver o tipo escrito da cláusula só para a mensagem.

##### `illegal_async_generator_return_type` (13), `illegal_sync_generator_return_type` (13), `illegal_async_return_type` (8) — todos FN
- **Emissão:** `ReturnTypeVerifier.verifyReturnType` (`analyzer/lib/src/error/return_type_verifier.dart:79-130`),
  chamado de `ErrorVerifier.visitFunctionDeclaration` (`error_verifier.dart:964`, funções de topo e
  funções locais nomeadas) e `visitMethodDeclaration` (`:1180`). Expressões de função não passam aqui.
- **Condição:** só com tipo de retorno ESCRITO. `async*` → elemento esperado `Stream`, `async` →
  `Future`, `sync*` → `Iterable`. Erro se (gerador e o tipo declarado é `void`) ou
  `!isSubtypeOf(Expected<Never>, declarado)` (`_isLegalReturnType`, `:297-322`): `int`, `void`,
  `SubStream<int>` (subclasse do esperado) falham; `dynamic`, `Object`, `FutureOr<…>`, `Stream<num>` passam.
  Marca `hasLegalReturnType = false`, o que SUPRIME `return_of_invalid_type` dos `return` do corpo
  (`_checkReturnExpression`, `:131-140`).
- **Posição:** o nó do tipo de retorno escrito (`atNode(returnType)`: `int`, `SubStream<int>`, `void`).
- **Mensagem:** fixas (`messages.yaml:6851`, `:7064`, `:6887`): "Functions marked 'async*' must have a
  return type that is a supertype of 'Stream<T>' for some type 'T'." / "…'sync*'… 'Iterable<T>'…" /
  "Functions marked 'async' must have a return type which is a supertype of 'Future'."
- **No DartForge:** não existe. Lugar: um verificador sobre `program.functions` com o tipo de retorno do
  outline (`outline.functions[i].signature`), mais as funções locais nomeadas (passada de corpos), e a
  supressão de `return_of_invalid_type` correspondente na inferência
  (`crates/types/src/inferencia/`). Todas as amostras são funções/métodos simples (`int f() async*`,
  `void f() sync*`, `SubFuture<int> m() async`).

##### `type_parameter_supertype_of_its_bound` (perda 13: FN 13)
- **Emissão:** `ErrorVerifier._checkForTypeParameterBoundRecursion`
  (`error_verifier.dart:5382-5416`), de `visitTypeParameterList` (`:1595-1599`) — toda lista de
  parâmetros de tipo (classes, mixins, enums, extensões, typedefs, funções, métodos, funções locais e
  tipos de função genéricos).
- **Condição:** para cada parâmetro `P` com limite: segue `current = elementToNode[bound.extensionTypeErasure.element]`
  enquanto o limite de `current` é um `NamedType` que (após apagar tipos de extensão) é um parâmetro DA
  MESMA LISTA; se der `parameters.length` passos, é ciclo. Ex.: `T extends T`; `T extends U, U extends T`;
  `U extends A<U>` com `extension type A<T>(T it)` (o apagamento de `A<U>` é `U`).
- **Posição:** o nome do parâmetro (`atToken(parameter.name)`), um por parâmetro do ciclo.
- **Mensagem:** "'{0}' can't be a supertype of its upper bound." `{0}` = displayName (correção usa o
  limite `{1}`).
- **No DartForge:** não existe. Sintático, salvo o apagamento de tipo de extensão (precisa resolver o
  nome do limite para saber se é tipo de extensão cuja representação é um parâmetro). Lugar: um
  verificador em `crates/analise` que percorre todas as listas de parâmetros de tipo (`ast.decls`,
  `ast.functions`, `TypeKind::Function` em `ast.types`).

##### `type_parameter_referenced_by_static` (perda 10: FN 10)
- **Emissão:** `ErrorVerifier._checkForTypeParameterReferencedByStatic` (`error_verifier.dart:5419-5435`),
  de `visitNamedType` (`:1270`, tipos escritos) e `visitSimpleIdentifier` (`:1421`, identificador `T`
  numa expressão).
- **Condição:** dentro de método estático (`_enclosingExecutable.inStaticMethod`) ou de inicializador de
  variável estática, um nome que resolve para parâmetro de tipo de uma classe/mixin/enum/extensão.
- **Posição:** o token do nome. **Mensagem:** "Static members can't reference type parameters of the
  class." (sem args).
- **No DartForge:** `crates/types/src/resolve.rs:180-190` (tipos do outline) e
  `crates/types/src/inferencia/tipos.rs:367` (tipos nos corpos). FN: os usos em expressão (`T;`,
  `new T()`, closures `(T a) {}` dentro de corpo estático, `reify_typevar_static_test`), isto é, os
  identificadores e os tipos de parâmetro de closures resolvidos na inferência sem o contexto estático.

##### `wrong_type_parameter_variance_position` (perda 9: FN 9)
- **Emissão:** `ErrorVerifier._checkForWrongTypeParameterVarianceInField/InMethod` →
  `_checkForWrongVariancePosition` (`error_verifier.dart:≈5860-5925`).
- **Condição/posição/mensagem:** como já implementado em `crates/types/src/variancia.rs` (só experimento
  `variance`): "The '{0}' type parameter '{1}' can't be used in an '{2}' position."
- **No DartForge:** os 9 FN são do teste `variance/variance_in_method_error_test.dart` e
  `variance_out_field_error_test.dart`: tipos `Inv<T>`/`Cov<Cov<T>>` onde a variância vem de classe com
  parâmetro `inout` declarado e setters `set a(T value)` — conferir `variancia_em` para classes com
  variância explícita (Inv = `inout` dá `invariant`) e setters (o parâmetro do setter é contravariante).
##### `main_is_not_function` (perda 9: FN 9)
- **Emissão:** `ErrorVerifier._checkForMainFunction1` (`analyzer/lib/src/generated/error_verifier.dart:4125-4141`),
  chamado de visitClassDeclaration (`:518`), visitClassTypeAlias (`:546`), visitEnumDeclaration (`:716`),
  visitFunctionDeclaration (`:965`), visitFunctionTypeAlias (`:1019`), visitGenericTypeAlias (`:1049`),
  visitMixinDeclaration (`:1256`), visitTopLevelVariableDeclaration (`:1564`, cada variável).
  (Extensão e tipo de extensão não chamam.)
- **Condição:** elemento de topo (pai é a unidade) com `displayName == 'main'` que não é
  `FunctionElement` (classe, alias de classe, enum, mixin, typedef de qualquer forma, getter/setter de
  topo — `PropertyAccessorElement` —, variável de topo).
- **Posição:** o token do nome. **Mensagem:** "The declaration named 'main' must be a function." (sem args).
- **No DartForge:** não existe. Sintático: em `crates/analise` (percorrer `unit.declarations`). As 9
  amostras cobrem exatamente as formas acima (`class main`, `class main = A with M`, `enum main`,
  `int get main`, `mixin main`, `typedef main = …`, `typedef void main()`, `var main`).

##### `top_level_cycle` (perda 10: FN 10)
- **Emissão:** `ResolverVisitor._checkTopLevelCycle` (`analyzer/lib/src/generated/resolver.dart:4009-4030`),
  de `visitVariableDeclaration` (`:3914`), para variáveis de topo e campos (estáticos e de instância)
  que não são `const`.
- **Condição:** `element.typeInferenceError.kind == dependencyCycle`: a inferência de tipo de topo
  (`analyzer/lib/src/summary2/top_level_inference.dart:228-240`) encontrou o elemento sendo inferido de
  novo; todos os elementos do ciclo (da pilha `_inferring` a partir do repetido) recebem o mesmo erro.
  Só variáveis SEM tipo escrito com inicializador participam (as que precisam de inferência).
- **Posição:** o nome da variável. **Mensagem:** "The type of '{0}' can't be inferred because it depends
  on itself through the cycle: {1}." `{0}` = nome; `{1}` = nomes do ciclo ORDENADOS
  (`.sorted()`, `:234`) e unidos por ", ".
- **No DartForge:** não existe. A inferência dos inicializadores está em `crates/types`
  (`infer_bodies_das_bibliotecas`, a passada sem corpos que estabiliza os inicializadores); é preciso
  detectar o ciclo ali (pilha de variáveis em inferência) e marcar cada membro. Amostras: `var x = y + 1;
  var y = x + 1;`, campos estáticos, `var x = x;`, `var elems = [elems]`, e o caso cruzado
  `static final a = b.c; static final b = A(); final c = a;` (ciclo "a, c": `b` tem tipo pelo
  construtor, não entra).

##### `default_value_on_required_parameter` (perda 7: FN 4, pos 3)
- **Emissão:** `ErrorVerifier._checkUseOfDefaultValuesInParameters` (`error_verifier.dart:6196-6206`), em
  TODA lista de parâmetros (independe de `defaultValuesAreExpected`: vale para métodos abstratos,
  `external` etc.).
- **Condição:** parâmetro nomeado `required` com valor padrão.
- **Posição:** o NOME do parâmetro (`_parameterName ?? parameter`).
- **Mensagem:** "Required named parameters can't have a default value." (sem args).
- **No DartForge:** só existe na elaboração do construtor primário
  (`crates/frontend/src/parser/declarations.rs:≈1268`, `p.span` inteiro → as 3 posições erradas, devem
  ser `p.name.span`). FN: funções, métodos (inclusive abstratos) e o teste
  `nnbd/syntax/class_member_declarations_error_test.dart` — falta a regra geral (sintática; pode ficar no
  parser ou em `crates/analise`).

##### `invalid_modifier_on_setter` (perda 6: FN 6)
- **Emissão:** parser fasta, `messageSetterNotSync` (`_fe_analyzer_shared/lib/src/parser/parser_impl.dart:3887-3889`
  para setter de topo, `:4969-4971` para setter membro), mapeado para
  `CompileTimeErrorCode.INVALID_MODIFIER_ON_SETTER` (`analyzer/lib/src/fasta/error_converter.dart:286-290`).
- **Condição:** setter (`set`) cujo corpo tem modificador `async`, `async*` ou `sync*`.
- **Posição:** o primeiro token do modificador (`asyncToken`): `async` (5) para `async`/`async*`,
  `sync` (4) para `sync*` (o `*` não entra).
- **Mensagem:** "Setters can't use 'async', 'async*', or 'sync*'." (sem args).
- **No DartForge:** não existe; entra no parser (`crates/frontend/src/parser`), onde se lê o modificador
  de corpo de setter de topo e de membro. Sintático (sempre publicado).
##### `invalid_annotation` (perda 7: FN 7)
- **Emissão:** `AnnotationResolver` (`analyzer/lib/src/dart/resolver/annotation_resolver.dart`), na
  resolução de corpos/declarações (ResolverVisitor.visitAnnotation), em vários ramos: getter de classe
  que não é construtor (`:86-91`), construtor ausente (`:110-116`), variável local (`_localVariable`,
  `:170-184`: `!element.isConst || node.arguments != null`), elemento que não é acessor/alias/variável
  (`:325-330`), acessor não sintético ou variável não-const ou com argumentos
  (`_resolveAnnotationElementGetter`, `:337-352`), alias de tipo sem construtor/getter
  (`_typeAliasGetter`, `:383-413`).
- **Posição:** a anotação inteira (`atNode(node)`: de `@` ao fim dos argumentos).
- **Mensagem:** "Annotation must be either a const variable reference or const constructor invocation."
- **No DartForge:** `crates/types/src/inferencia/funcoes.rs:950-1035` (`validar_anotacao`), que resolve
  nomes só no escopo da BIBLIOTECA (+ estáticos da classe). FN:
  1. anotação em declaração local que nomeia variável LOCAL (`final a = 0; @a var b;` — não-const →
     inválida; `const a = 0; @a(0)` — com argumentos → inválida): falta o escopo local;
  2. `@V` com `typedef V();` (alias de tipo de FUNÇÃO: `_typeAliasGetter` com `aliasedType` não
     interface → INVALID_ANNOTATION);
  3. 4 casos de `@Deprecated.optional()` em parâmetros de TIPO DE FUNÇÃO (`void Function([@… int? p])`):
     no SDK 3.6.2 `Deprecated.optional` não existe → construtor nulo → INVALID_ANNOTATION; as anotações
     de parâmetros de `TypeKind::Function` não são visitadas pela nossa validação.

##### `undefined_annotation` (perda 1: FN 1)
- **Emissão:** `AnnotationResolver` (`annotation_resolver.dart:287-295` e análogos), "Undefined name
  '{0}' used as an annotation." na anotação inteira.
- **Amostra:** `@foo class A { static const foo = null; }` — a anotação de uma DECLARAÇÃO DE CLASSE é
  resolvida no escopo da biblioteca, não no da classe. O nosso `validar_anotacao` consulta
  `getter_estatico(classe…)` primeiro: para a anotação da própria classe, `classe` deve ser `None`.

##### `return_without_value` (perda 6: FN 6)
- **Emissão:** `ReturnTypeVerifier._checkReturnWithoutValue` (`analyzer/lib/src/error/return_type_verifier.dart:277-295`),
  de `verifyReturnStatement` (`:53-74`) — fora de construtor gerador e de gerador; `return;` sem valor.
- **Condição:** `T = enclosingExecutable.returnType` (para CLOSURE é o tipo de retorno INFERIDO dela); se
  síncrono, erro salvo `T ∈ {void, dynamic, InvalidType, Null}`; se `async`, o mesmo sobre
  `futureValueType(T)`.
- **Posição:** o token `return`. **Mensagem:** "The return value is missing after 'return'."
- **No DartForge:** `crates/types/src/inferencia/instrucoes.rs:779-792`: exige `fc.executavel` (só
  executáveis DECLARADOS). Os 6 FN são closures: `future.catchError((e, st) { return; })` (o contexto dá
  retorno `FutureOr<int>`), `return (int y) { if (y<0) { return; } … }`, e os
  `invalid_returns/async_invalid_return_0x_test.dart` (closures/funções `async`). Precisa do tipo de
  retorno inferido da closure (o do contexto, ou o inferido dos `return` com valor) — atenção à ordem:
  a inferência do retorno da closure usa os próprios `return`s.

##### `return_in_generator` (perda 10: FN 10)
- **Emissão:** parser fasta, `messageGeneratorReturnsValue` (analyzerCode RETURN_IN_GENERATOR,
  `analyzer/lib/src/fasta/error_converter.dart:414-418`): em `parseReturnStatement` quando há expressão
  (`_fe_analyzer_shared/lib/src/parser/parser_impl.dart:5729-5742`, posição = token `return`) e em
  `parseExpressionFunctionBody` (`:5508-5519`, posição = token `=>`), com `inGenerator` (corpo `sync*` ou
  `async*`).
- **Mensagem:** "Can't return a value from a generator function that uses the 'async*' or 'sync*'
  modifier." (sem args).
- **No DartForge:** não existe; entra no parser (sintático, sempre publicado). Amostras: `f() async* {
  return 0; }`, `f() sync* => 0;`, `Iterable<void> f() sync* => [];`.

##### `yield_in_non_generator` (perda 4: FN 4 — dois por arquivo)
- **Emissão:** DUAS fontes, ambas com o código `yield_in_non_generator`:
  1. parser: `messageYieldNotGenerator` em `parseYieldStatement`
     (`_fe_analyzer_shared/lib/src/parser/parser_impl.dart:5700-5716`) fora de gerador, no token `yield`
     (analyzerCode YIELD_IN_NON_GENERATOR, `error_converter.dart:506-510`), mensagem "Yield statements
     must be in a generator function (one marked with either 'async*' or 'sync*')." (mesmo para `yield*`);
  2. resolução: `YieldStatementResolver._resolve_notGenerator`
     (`analyzer/lib/src/dart/resolver/yield_statement_resolver.dart:185-198`), no `YieldStatement` inteiro
     (`yield 0;` com o `;`), YIELD_IN_NON_GENERATOR ou, com `*`, YIELD_EACH_IN_NON_GENERATOR
     (`sharedName: YIELD_IN_NON_GENERATOR`, `messages.yaml:18526`: "Yield-each statements must be in a
     generator function…").
- **No DartForge:** não existe nenhuma das duas. Amostras: `f() async { yield 0; }` e `yield* 0;`.

##### `yield_of_invalid_type` (perda 2: FN 2)
- **Emissão:** `YieldStatementResolver._checkForYieldOfInvalidType`
  (`analyzer/lib/src/dart/resolver/yield_statement_resolver.dart:72-140`).
- **No DartForge:** `crates/types/src/inferencia/instrucoes.rs:544-570` (`yield_invalido`) usa o retorno
  DECLARADO; os 2 FN são closures geradoras `() async* { yield 1; }` / `() sync* { yield 1; }` cujo tipo
  de retorno vem do CONTEXTO (`Stream<String> Function() v = …`) — falta usar o retorno inferido/contexto
  da closure.
##### `non_covariant_type_parameter_position_in_representation_type` (perda 2: FN 2)
- **Emissão:** `ErrorVerifier._checkForNonCovariantTypeParameterPositionInRepresentationType`
  (`analyzer/lib/src/generated/error_verifier.dart:4752-4780`), de visitExtensionTypeDeclaration.
- **Condição:** para cada parâmetro de tipo do tipo de extensão, o tipo de representação, visitado por
  `NonCovariantTypeParameterPositionVisitor` com variância inicial covariante, tem uma ocorrência dele em
  posição não covariante (parâmetro de tipo de função, invariante…).
- **Posição:** o nó do parâmetro de tipo inteiro (`atNode(typeParameterNode)`: nome + `extends` limite se
  houver). **Mensagem:** "An extension type parameter can't be used in a non-covariant position of its
  representation type." (sem args).
- **No DartForge:** não existe; `crates/types/src/variancia.rs` já tem o cálculo de variância
  (`variancia_em`) e o outline tem o tipo de representação. Amostras: `extension type A<T>(T Function(T) it)`,
  `extension type A<T>(void Function(T) it)`.

##### `built_in_identifier_as_type` (perda 2: FN 2)
- **Emissão:** parser fasta (`BUILT_IN_IDENTIFIER_AS_TYPE`, `analyzer/lib/src/fasta/error_converter.dart:55-60`;
  `messageBuiltInIdentifierAsType` no `_fe_analyzer_shared` ao ler um tipo cujo nome é identificador
  embutido), e também `NamedTypeResolver` quando o nome prefixado `p.import` é usado como tipo.
- **Posição:** o identificador embutido (`import` em `import<int> x = [];`; em `p.import x`, o nome
  `p.import` inteiro, 6 caracteres a partir de `p`? — a amostra marca col 10 com `^^^^^^`, i.e.
  `import`… conferir: `void f(p.import x)` col 10 é o `i` de `import`? (`void f(` = 7, `p.` 8-9) → é o
  nome sem o prefixo, length 6).
- **Mensagem:** "The built-in identifier '{0}' can't be used as a type." `{0}` = lexema.
- **No DartForge:** não existe; o caso de topo é do parser (`crates/frontend/src/parser/types.rs`), o
  prefixado é da resolução de tipos (`crates/types/src/resolve.rs` / `codigo_de_nome_de_tipo`).

##### `deprecated_optional` (perda 4: FN 4) — sintaxe nova, oráculo 3.13.4
- Não existe no 3.6.2 (`@Deprecated.optional` e `.m()` dot-shorthand são 3.10+). Pelo corpus: aviso no
  ARGUMENTO OMITIDO de uma chamada a função/construtor cujo parâmetro opcional tem
  `@Deprecated.optional()`, posição = nome do construtor/método na chamada (`.m()` → `m`; `.new()` →
  `new`), mensagem "Omitting an argument for the '{0}' parameter is deprecated." `{0}` = nome do
  parâmetro. Requer o SDK 3.13 (a classe `Deprecated.optional`), que não temos: deixar de fora.

##### `obsolete_colon_for_default_value` (perda 4: FN 4)
- **Emissão:** `BestPracticesVerifier.visitDefaultFormalParameter`
  (`analyzer/lib/src/error/best_practices_verifier.dart:288-306`).
- **Condição:** parâmetro nomeado cujo separador do default é `:` (`{int x : 0}`, `{super.a : ''}`):
  versão da linguagem ≥ 3.0 → `CompileTimeErrorCode.OBSOLETE_COLON_FOR_DEFAULT_VALUE`; < 3.0 →
  `HintCode.DEPRECATED_COLON_FOR_DEFAULT_VALUE`.
- **Posição:** o token `:`. **Mensagem:** "Using a colon as the separator before a default value is no
  longer supported." (correção "Try replacing the colon with an equal sign.").
- **No DartForge:** não existe; sintático (precisa saber que o separador foi `:` — guardar no
  `ast::Parameter` ou olhar a fonte entre o nome e o default, como `membros::span_sem_padrao` já faz).
  As amostras de `deprecated_colon_for_default_value/*` no 3.6.2 também dão OBSOLETE (versão 3.6).

##### `type_check_with_null` (perda 1: FN 1)
- **Emissão:** `BestPracticesVerifier._checkAllTypeChecks` (`best_practices_verifier.dart:770-830`):
  `x is Null`/`is! Null` (TYPE_CHECK_IS_NULL/IS_NOT_NULL, sharedName TYPE_CHECK_WITH_NULL).
- **FN:** `y is Never?` — o tipo escrito `Never?` é `Null` (o verificador olha o TIPO resolvido
  `isDartCoreNull`, não o nome). O nosso só reconhece o nome `Null`.

##### `type_test_with_non_type` (perda 1: FN 1)
- **Emissão:** `NamedTypeResolver` → `_ErrorHelper.reportNullOrNonTypeElement`
  (`analyzer/lib/src/dart/resolver/named_type_resolver.dart:≈555-565`) para o tipo de `is`.
- **FN:** `var aa = new A(); if (a is aa)` — o nome resolve para a VARIÁVEL LOCAL (achou, não é tipo) →
  TYPE_TEST_WITH_NON_TYPE "The name '{0}' isn't a type and can't be used in an 'is' expression.". O
  nosso resolve tipos de corpo só no escopo da biblioteca (não acha `aa` → outro código ou nada).

##### `invalid_return_type_for_catch_error` (perda 8: FN 8)
- **Emissão:** é o `sharedName` de `WarningCode.RETURN_OF_INVALID_TYPE_FROM_CATCH_ERROR`
  (`messages.yaml:24902`), `ReturnTypeVerifier._checkReturnExpression` → `reportTypeError`
  (`analyzer/lib/src/error/return_type_verifier.dart:150-160`) quando a closure é o `onError` de
  `Future.catchError` (`enclosingExecutable.catchErrorOnErrorReturnType != null`, marcado pelo
  `ErrorHandlerVerifier`, `analyzer/lib/src/error/error_handler_verifier.dart`) e o valor retornado não é
  atribuível a `FutureOr<T>`/`T`.
- **Posição:** a expressão retornada (corpo `=>` ou `return e;`). **Mensagem:** "A value of type '{0}'
  can't be returned by the 'onError' handler because it must be assignable to '{1}'." ({0} tipo da
  expressão, {1} o tipo `T` de `Future<T>`).
- **No DartForge:** não existe; o nosso relata (ou não) `return_of_invalid_type_from_closure` nesses
  casos. Entra na inferência de closures (`crates/types/src/inferencia/`): marcar a closure argumento
  `onError` de `Future.catchError` e trocar o código.

##### `variable_type_mismatch` (perda 4: FP 4)
- **Emissão:** `ConstantEvaluationEngine` (`analyzer/lib/src/dart/constant/evaluation.dart:120-135`):
  constante cujo valor avaliado não casa o tipo declarado em tempo de execução
  (`runtimeTypeMatch`), quando o tipo estático É atribuível.
- **FP:** `const void Function(Z) c13 = prefix.c01;` / `const void Function(double) … = c01` com `c01`
  uma constante de função GENÉRICA: o analyzer instancia implicitamente a tear-off genérica pelo
  contexto (`void Function<X extends num>(X…)` → `void Function(Z)`), então o valor tem o tipo
  instanciado. O nosso avaliador (`crates/types/src/constantes/`) não aplica a instanciação implícita do
  contexto ao valor de uma referência a constante de função genérica.
##### `map_entry_not_in_map` (perda 11: FN 11)
- **Emissão:** `LiteralElementVerifier._verifyElement` (`analyzer/lib/src/error/literal_element_verifier.dart:92-100`),
  criado por `ErrorVerifier._checkForListElementTypeNotAssignable3` (`error_verifier.dart:4111`, lista) e
  `_checkForSetElementTypeNotAssignable3` (`:5292`, conjunto), na fase do ErrorVerifier; também no
  avaliador de constantes (`analyzer/lib/src/dart/constant/evaluation.dart`) para literais `const`.
- **Condição:** um `MapLiteralEntry` (`k: v`) como elemento (inclusive dentro de `if`/`for`) de um literal
  de LISTA ou de CONJUNTO (`<int>{1:2}`, `[1:2]`, `new Map<int>{…}` legado que vira conjunto de 1 arg).
- **Posição:** a entrada inteira (`k: v`). **Mensagem:** "Map entries can only be used in a map literal."
- **No DartForge:** só no avaliador de constantes (`crates/types/src/constantes/avaliador.rs`); falta no
  literal não-const. Lugar: onde a inferência dá `list_element_type_not_assignable`/`set_element_…`
  (`crates/types/src/constantes/verificador.rs:1055-1063`) ou na inferência de literais
  (`crates/types/src/inferencia/`). As 11 amostras: `<int>{1:2}`, `const <int>{1:2}`, `[1:2]`,
  `new Map<int>{…}` (map/literal13_test).

##### `case_expression_type_implements_equals` (perda 10: FN 10)
- **Emissão:** `ConstantVerifier._validateSwitchStatement_nullSafety`
  (`analyzer/lib/src/dart/constant/constant_verifier.dart:1008-1030`).
- **Condição:** SÓ sem o recurso `patterns` (biblioteca < 3.0, p.ex. `// @dart=2.19`): o valor constante
  do `case` não tem igualdade primitiva (`hasPrimitiveEquality`: `double`, classes que sobrescrevem `==`).
- **Posição:** a expressão do case. **Mensagem:** "The switch case expression type '{0}' can't override
  the '==' operator." `{0}` = tipo do VALOR (DartType).
- **No DartForge:** não existe. Amostras: `const/switch2_legacy_test.dart` (`case 0.0:` em 2.19),
  `const/switch2_test.dart`, e classes com `==`. Lugar: `crates/types/src/constantes/verificador.rs`, na
  verificação de `switch` (precisa da versão de linguagem da biblioteca).

##### `integer_literal_out_of_range` (perda 10: FN 10) e `integer_literal_imprecise_as_double` (perda 3: FN 3)
- **Emissão:** `ErrorVerifier._checkForOutOfRange` (`analyzer/lib/src/generated/error_verifier.dart:4985-5021`),
  de visitIntegerLiteral.
- **Condição:** fonte sem `_` (separadores tirados); `treatedAsDouble = staticType == double` (literal
  inteiro em contexto `double`): inválido se não é representável exatamente como double
  (`IntegerLiteralImpl.isValidAsDouble`); senão inválido se não cabe em 64 bits
  (`isValidAsInteger(source, isNegated)`, com `-` imediato contando: `-9223372036854775808` é válido,
  hexadecimal até 0xFFFFFFFFFFFFFFFF é válido).
- **Posição:** o literal (sem o `-`). **Mensagem:** OUT_OF_RANGE "The integer literal {0} can't be
  represented in 64 bits." `{0}` = lexema ORIGINAL (com `_`), prefixado de `-` se negado
  (a amostra `-9223372036854775809` está em col 10 = o literal, e a mensagem mostra o `-`);
  IMPRECISE "The integer literal is being used as a double, but can't be represented as a 64-bit double
  without overflow or loss of precision: '{0}'." + {1} = o double mais próximo como BigInt.
- **No DartForge:** não existe. Lugar: na inferência do literal inteiro
  (`crates/types/src/inferencia/`, onde se decide int vs double pelo contexto).

##### `not_iterable_spread` (perda 9: FN 9) e `not_map_spread` (perda 6: FN 6)
- **Emissão:** `LiteralElementVerifier._verifySpreadForListOrSet` (`literal_element_verifier.dart:176-213`)
  e `_verifySpreadForMap` (`:267-303`).
- **Condição:** tipo da expressão do spread: `dynamic` → só com `strict-casts`; subtipo de `Never` → nada;
  `?...` com tipo anulável → usa o não-nulo; senão `asInstanceOf(Iterable)`/`asInstanceOf(Map)` nulo → erro.
- **Posição:** a expressão espalhada (não o `...`). **Mensagens:** "Spread elements in list or set
  literals must implement 'Iterable'." / "Spread elements in map literals must implement 'Map'."
- **No DartForge:** não existe. Mesmo lugar do `map_entry_not_in_map`; amostras `[...a]` com `a` int,
  dentro de `for`/`if` elementos, `<int,int>{...a}`.

##### `unqualified_reference_to_non_local_static_member` (perda 9: FN 9) e `unqualified_reference_to_static_member_of_extended_type` (perda 8: FN 8)
- **Emissão:** `ErrorVerifier._checkForUnqualifiedReferenceToNonLocalStaticMember`
  (`error_verifier.dart:5659-5700`, identificadores simples), `MethodInvocationResolver` (`:215-226`,
  invocação `a()`), `FunctionReferenceResolver` (`:180-191`).
- **Condição:** nome simples, sem receptor, que resolve para membro ESTÁTICO de um SUPERTIPO (não da
  classe que envolve) → NON_LOCAL; dentro de extensão, membro estático do TIPO ESTENDIDO → EXTENDED_TYPE.
- **Posição:** o identificador. **Mensagens:** "Static members from supertypes must be qualified by the
  name of the defining type." (arg {0} = nome do tipo, só na correção) / "Static members from the
  extended type or one of its superclasses must be qualified by the name of the defining type."
- **No DartForge:** existe em `crates/types/src/inferencia/expr.rs`. FN pelas amostras: invocação de
  GETTER estático que devolve função (`static void Function() get a; … a();` — caminho de
  `FunctionExpressionInvocation`), e casos com classes `abstract base`/`final` e extensões sobre elas
  (conferir se a busca do membro estático no supertipo falha quando a classe tem modificador).

##### `label_undefined` (perda 8: FN 8), `continue_label_invalid` (perda 4: FN 4), `label_in_outer_scope` (perda 2: FN 2)
- **Emissão:** `ResolverVisitor._lookupBreakOrContinueTarget` (`analyzer/lib/src/generated/resolver.dart:5314-5366`),
  de visitBreakStatement/visitContinueStatement.
- **Condição:** `break L`/`continue L`: sem escopo de rótulos ou rótulo não achado → LABEL_UNDEFINED
  (no nome do rótulo, arg = nome); achado mas declarado fora da closure atual → LABEL_IN_OUTER_SCOPE (no
  nome, arg = nome); `continue` cujo alvo não é laço (`do`/`for`/`while`) nem `SwitchMember` →
  CONTINUE_LABEL_INVALID (no STATEMENT inteiro `continue L;`).
- **Mensagens:** "Can't reference an undefined label '{0}'." / "Can't reference label '{0}' declared in
  an outer method." / "The label used in a 'continue' statement must be defined on either a loop or a
  switch member."
- **No DartForge:** não existe. Lugar: passada de corpos (rótulos em escopo por função; o parser já
  guarda os rótulos dos statements). Amostras: `break y;`, `break x;` (x é parâmetro), `continue L` com
  `L:` num bloco ou num `switch` (o rótulo do switch, não de um case).

##### `ambiguous_set_or_map_literal_either` (perda 7: FN 7) e `ambiguous_set_or_map_literal_both` (perda 3: FN 3)
- **Emissão:** `TypedLiteralResolver._inferSetOrMapLiteralType`
  (`analyzer/lib/src/dart/resolver/typed_literal_resolver.dart:≈545-597`).
- **Condição:** literal `{…}` sem argumentos de tipo cujos elementos (spreads de `dynamic`, etc.) e cujo
  contexto não decidem set vs map: se algum elemento obriga map E algum obriga set → BOTH; senão → EITHER.
  O tipo vira `dynamic`.
- **Posição:** o literal inteiro. **Mensagens:** EITHER "This literal must be either a map or a set, but
  the elements don't have enough information for type inference to work." / BOTH "The literal can't be
  either a map or a set because it contains at least one literal map entry or a spread operator
  spreading a 'Map', and at least one element which is neither of these."
- **No DartForge:** não existe. Amostras: `{...a, ...b}` com `a`,`b` dynamic, `{...set, ...map}`. Há um
  FP derivado: `non_bool_negation_expression` em `!{...a, ...b}` (o nosso dá tipo ≠ dynamic ao literal
  ambíguo; o analyzer, `dynamic`, e então não reclama da negação).

##### `collection_element_from_deferred_library` (perda 5: FN 5)
- **Emissão:** `sharedName` de NON_CONSTANT_LIST_ELEMENT/…MAP_KEY/…MAP_VALUE/SET_ELEMENT_FROM_DEFERRED_LIBRARY
  (`messages.yaml:1915-2090`), escolhido em `analyzer/lib/src/dart/constant/evaluation.dart:1895-1915` e
  relatado pelo `ConstantVerifier` (`constant_verifier.dart:680-712`).
- **Posição/condição:** elemento de literal `const` que referencia constante via prefixo `deferred`
  (`const [foo.c]`), na expressão do elemento.
- **No DartForge:** existe em `crates/types/src/constantes/verificador.rs`; FN só em
  `deferred/load_constants_test.dart` (`const [foo.c]` DENTRO DE UMA CLOSURE argumento de
  `Expect.throws`) — conferir se os literais const dentro de closures são verificados.

##### `receiver_of_type_never` (perda 4: FN 4)
- **Emissão:** os resolvedores de binário/postfix/prefix/propriedade/invocação
  (`binary_expression_resolver.dart:425`, `property_element_resolver.dart:96`,
  `method_invocation_resolver.dart:543`, …), `WarningCode.RECEIVER_OF_TYPE_NEVER` no receptor.
- **No DartForge:** existe (`crates/types/src/inferencia/expr.rs`). FN: `never..(_) => 1` (cascata em
  receptor Never, sintaxe nova "anonymous methods" — oráculo 3.13) e `x?[0]` (índice null-aware com
  receptor `Never`).

##### `import_internal_library` (perda 4: FN 4) e `export_internal_library` (perda 2: FN 2)
- **Emissão:** `ErrorVerifier._checkForImportInternalLibrary` (`error_verifier.dart:3800-3822`) e
  `_checkForExportInternalLibrary` (`:3263-3302`).
- **Condição:** a URI resolve para biblioteca do SDK marcada `isInternal` (`dart:_internal`,
  `dart:_wasm`…, conforme `libraries.dart` do SDK). Import: posição = a URI (string), arg = URI;
  export: posição = a DIRETIVA inteira (`export 'dart:_internal';`, col 1), arg = URI.
- **Mensagens:** "The library '{0}' is internal and can't be imported." / "…exported."
- **No DartForge:** não existe. Lugar: a fase de diretivas (`crates/paridade/src/analise.rs` passo 2 /
  `crates/elements` carga), com a lista de bibliotecas internas do SDK (todas `dart:_*`).

##### `shared_deferred_prefix` (perda 4: FN 4)
- **Emissão:** `ErrorVerifier._checkDeferredPrefixCollision` (`error_verifier.dart:1910-1923`).
- **Condição:** dois ou mais imports com o mesmo prefixo e algum `deferred`: cada `deferred` dá erro.
- **Posição:** o token `deferred`. **Mensagem:** "The prefix of a deferred import can't be used in other
  import directives."
- **No DartForge:** não existe; sintático sobre as diretivas.

##### `uri_with_interpolation` (perda 4: FN 4)
- **Emissão:** `LibraryAnalyzer` (`analyzer/lib/src/dart/analysis/library_analyzer.dart:719`, `:947`,
  `:995`) para import/export/part E `@docImport` em comentário de documentação.
- **Posição:** a string da URI. **Mensagem:** "URIs can't use string interpolation."
- **No DartForge:** não existe. Amostras: `export '${'foo'}.dart';`, `/// @docImport '${'foo'}.dart';`.

##### `equal_elements_in_set` (perda 3: FN 3) e `equal_keys_in_map` (perda 3: FN 3)
- **Emissão:** `BestPracticesVerifier._checkForDuplications` (`analyzer/lib/src/error/best_practices_verifier.dart:851-875`).
- **Condição:** literal NÃO const; elementos (set) ou chaves (map) de PRIMEIRO NÍVEL que avaliam como
  constante sem erro (`computeConstantValue`) com valor igual a um anterior.
- **Posição:** a expressão repetida (a segunda). **Mensagens:** "Two elements in a set literal shouldn't
  be equal." / "Two keys in a map literal shouldn't be equal."
- **No DartForge:** não existe (só os `const` com `equal_*_in_const_*`). Amostras: `{a, b}` com
  `const a = 1; const b = 1;`, `{1, one}`. Lugar: avaliador de constantes já existente
  (`crates/types/src/constantes/`), aplicado a literais não-const.

##### `expression_in_map` (perda 3: FN 3)
- **Emissão:** `LiteralElementVerifier._verifyElement` (`literal_element_verifier.dart:72-122`): expressão
  solta (ou `?e` null-aware) num literal de MAPA.
- **No DartForge:** só no caminho de constantes. FN: `<String,int>{'a', 'b' : 2}` não-const e os
  `{0: ?""}` de `null_aware_elements` (sintaxe 3.8 — oráculo 3.13).

##### `doc_directive_missing_closing_brace` (perda 2), `uri_does_not_exist_in_doc_import` (2), `deprecated_new_in_comment_reference` (1), `doc_import_cannot_be_deferred` (1), `doc_import_cannot_have_configurations` (1)
- **Emissão:** comentários de documentação: `DocCommentBuilder` (`analyzer/lib/src/fasta/doc_comment_builder.dart:1196,1231`,
  `{@youtube 600 400` sem `}` → no fim do diretivo), `LibraryAnalyzer` (`library_analyzer.dart:685-694`,
  `@docImport` com URI inexistente — WarningCode, na string), `BestPracticesVerifier` (`:251`, `[new B.c]`
  em comentário, no `new`), `DocCommentVerifier` (`analyzer/lib/src/error/doc_comment_verifier.dart:40-48`,
  `deferred` e `if (…)` em `@docImport`).
- **No DartForge:** não existe (comentários de doc não são analisados). Baixa prioridade.

##### `instance_access_to_static_member` (perda 2: FN 2)
- **Emissão:** `PropertyElementResolver` (`property_element_resolver.dart:334-350`) e
  `MethodInvocationResolver` (`:234-247`). Já publicado.
- **FN:** `a.f = 42` com `static set f(x)` (ESCRITA via instância num setter estático) e `getter/no_setter2_test.dart`
  (getter estático lido via instância): o nosso (`crates/types/src/inferencia/membros.rs`) não cobre o
  lado de atribuição/setter.

##### `rethrow_outside_catch` (perda 2: FN 2)
- **Emissão:** `ErrorVerifier._checkForRethrowOutsideCatch` (`error_verifier.dart:5199-5207`): `rethrow`
  com `catchClauseLevel == 0` do executável atual (uma CLOSURE dentro de `catch` reinicia o nível).
- **Posição:** a expressão `rethrow`. **Mensagem:** "A rethrow must be inside of a catch clause."
- **No DartForge:** não existe; sintático (pilha de funções/catch).

##### `undefined_enum_constant` (perda 2: FN 2)
- **Emissão:** `PropertyElementResolver` (`property_element_resolver.dart:≈690-700`): `E.X` em que `E` é
  enum e não há getter estático `X` → UNDEFINED_ENUM_CONSTANT (em vez de UNDEFINED_GETTER).
- **Posição:** o nome `X`. **Mensagem:** "There's no constant named '{0}' in '{1}'." (nome, enum).
- **No DartForge:** hoje sai `undefined_getter` (provavelmente) — trocar o código quando o alvo é enum.
  Amostras: `E.TWO`, `Enum2._A` de outra biblioteca (privado).

##### `non_bool_negation_expression` (perda 1: FP 1)
- `BoolExpressionVerifier` (`analyzer/lib/src/error/bool_expression_verifier.dart:82`). Único FP:
  `!{...a, ...b}` — consequência de não tratarmos o literal ambíguo como `dynamic` (ver
  `ambiguous_set_or_map_literal_*`).

##### `not_binary_operator` (perda 1: FN 1)
- **Emissão:** `BinaryExpressionResolver` (`analyzer/lib/src/dart/resolver/binary_expression_resolver.dart:73`):
  operador binário que não é operador binário de usuário (`5 ~ 3`, recuperação do parser). Posição: o
  operador; mensagem "'{0}' isn't a binary operator.".

##### `static_access_to_instance_member` (perda 1: FP 1)
- FP em `augmentation_type_parameter_count/…class_s_a61207de.dart` (`A.foo<int>()` com `augment class A`
  e `augment static void foo<T,U>();` sem o experimento — a recuperação do analyzer é outra). Já
  emitido via `crates/paridade/src/ponte.rs`; suprimir em unidades com `augment` sem o experimento.

##### `dot_shorthand_undefined_member` (perda 8: FP 8) — sintaxe nova, oráculo 3.13.4
- FP em `dot_shorthands/equality/equality_extension_override_error_test.dart` e afins: `ExtensionOverride == .member`
  — o 3.13 não relata `dot_shorthand_undefined_member` quando o lado esquerdo do `==` é uma
  sobreposição de extensão (o contexto não é tipo de interface utilizável); nós resolvemos o shorthand
  contra `int`. Descrever pelo corpus: ver os `// [analyzer]` do arquivo.

## D. Constantes

### Especificação — família D (constantes) — r3-d

Fonte: SDK 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\analyzer\lib` = analyzer 6.11; os arquivos
`dart/constant/*.dart`, `generated/error_verifier.dart` e `error/type_arguments_verifier.dart`
são idênticos aos do pub-cache 6.11, então as linhas valem para os dois). Citações como
`analyzer/lib/src/<arquivo>:<linha>`; parser em `_fe_analyzer_shared/lib/src/parser/...`.
Amostras: `E:\dftemp\analise\trab\placar-base-r3.txt`.

Arquitetura no DartForge (vale para quase todos os códigos):
- `crates/types/src/constantes/avaliador.rs` = `ConstantVisitor` + `_InstanceCreationEvaluator`
  (evaluation.dart); `verificador.rs` = `ConstantVerifier` (constant_verifier.dart);
  `potencial.rs` = `potentially_constant.dart`; `valor.rs` = `value.dart`.
- Chamado de `crates/paridade/src/analise.rs:493` (`constantes::verificar`) só para as
  bibliotecas do lote; diagnósticos atribuídos à unidade.
- **Não existe hoje**: (1) grafo de dependências de constantes (`compute.dart` +
  `computeDependencies`, evaluation.dart:223-325) — ciclos só são vistos entre variáveis em
  avaliação, construtores em ciclo viram "desconhecido" em silêncio; (2) avaliação das
  constantes de enum pelo construtor (hoje `valor_de_enum` monta só `index`/`_name`);
  (3) visita de anotações; (4) as regras do `ErrorVerifier` ligadas a const (non-const super,
  mixin com campo, campo não-final, construtor gerador de enum não-const, const com construtor
  indefinido, `_checkForConstWithNonConst`); (5) a troca do inicializador "não serializável"
  pelo marcador do linker (ver `const_initialized_with_non_constant_value`).
- Mecanismo útil novo (já escrito, não validado — ver "Estado do código" no fim): `Motor::parametro_de_tipo`
  decide se um nome de tipo escrito é parâmetro de tipo pelo alcance sintático (classe/mixin/enum/
  extensão/typedef = span da declaração; função/método/local/literal = span da função; tipo de
  função genérico = span do tipo; parâmetro-função antigo = span do parâmetro). Não há tabela
  AST-tipo → TypeId na inferência; isto evita mexer em `resolved.rs`.

---

##### `constant_pattern_never_matches_value_type` (perda 52: FN 52, FP 0, msg 0, pos 0)
- **Emissão:** `ConstantVerifier.visitConstantPattern`, `analyzer/lib/src/dart/constant/constant_verifier.dart:131-160`
  (fase ConstantVerifier, depois da resolução). `_canBeEqual` em `:517-548`.
- **Condição exata:**
  ```
  e = pattern.expression.unParenthesized
  if e.staticType is InvalidType: return
  v = _evaluateAndReportError(e, CONSTANT_PATTERN_WITH_NON_CONSTANT_EXPRESSION)
  if v is DartObjectImpl and feature patterns:
     if v.hasPrimitiveEquality(featureSet):            // value.dart: hasPrimitiveEquality
        ct = v.type                                    // tipo DO VALOR, não o estático
        mt = node.matchedValueType?.extensionTypeErasure
        if mt != null and !_canBeEqual(ct, mt): REPORT
  _canBeEqual(ct, vt):
     ct InterfaceType:
       vt InterfaceType: (ct int && vt double) → true;
                         senão isSubtypeOf(ct, greatestClosure(vt, top=Object?, bottom=Never))
       vt TypeParameterType: b = promotedBound ?? element.bound; se b != null e !hasTypeParameterReference(b):
                         lowest = vt '?' ? makeNullable(b) : b; return _canBeEqual(ct, lowest)
       vt FunctionType: ct Null → isNullable(vt); senão false
     tudo o mais → true
  ```
  `hasPrimitiveEquality` (value.dart): instância genérica sem `==`/`hashCode` próprios (com
  `patterns`, os dois), registros com campos primitivos, `Type`, `Symbol`, listas/conjuntos/mapas
  (são `GenericState`/`ListState`… com igualdade de identidade) — *não* `double`.
- **Posição:** o `ConstantPattern` inteiro (inclui `const` e parênteses do padrão, ex. 185:12
  `!false`, 341:12 `const !false` começa em `const`).
- **Mensagem:** `The matched value type '{0}' can never be equal to this constant of type '{1}'.`
  {0} = mt (DartType → `getDisplayString`, interseção como `T & int`), {1} = tipo do valor
  (`Class`, `GenericClass<dynamic>`, `List<int>`, `Map<dynamic, dynamic>`, `Type`, `Symbol`, `Null`).
- **Supressões:** nada se a expressão não avalia (aí sai `CONSTANT_PATTERN_WITH_NON_CONSTANT_EXPRESSION`);
  nada se `InvalidType`. Sai mesmo com `invalid_constant_pattern_*` do parser no mesmo padrão.
- **No DartForge:** hoje em `crates/types/src/inferencia/padroes.rs:346-382` (`constante_nunca_casa`,
  na inferência, não no verificador) e restrito a literais simples + enums usando o tipo
  ESTÁTICO. Causas dos 52 FN: (a) `const Class()`, `const GenericClass<int>()`, `const []`,
  `const {}`, `#a`, `!false`, `const (GenericClass<int>)`/`GenericClass<int>` (Type) não entram
  (precisam do valor avaliado); (b) `T & int` (tipo casado promovido a interseção) — o nosso
  `pode_ser_igual` desiste com parâmetro de tipo; (c) `case null` depois de outro `case null`
  / `Null _` (bool_switch_test:165, enum_switch_test:249, null_type_test:33,
  object_pattern_switch_test:204): o `matchedValueType` do analyzer é o tipo do scrutinee
  **promovido pelos casos anteriores** (fluxo de padrões: depois de `case null`, o resto vê `bool`
  não anulável); nós usamos o tipo sem a promoção.
  Implementação fiel: mover a regra para `verificador.rs::padrao` (Constant), usando o `Valor`
  avaliado (`v.tipo`, `Motor::igualdade_primitiva`) e o tipo casado — que a inferência precisa
  expor (nova tabela lateral `tipos_casados: HashMap<PatternId, TypeId>` em `UnitBodyTypes`,
  gravada em `padroes.rs` onde `constante_nunca_casa` já recebe `casado`) — e retirar a de
  `padroes.rs`. (c) exige a promoção por casos anteriores na inferência (dono: inferência).

##### `const_with_type_parameters` (perda 39: FN 39)
Um nome, três códigos únicos (`CONST_WITH_TYPE_PARAMETERS`, `_CONSTRUCTOR_TEAROFF`, `_FUNCTION_TEAROFF`).
- **Emissão:**
  1. `visitInstanceCreationExpression` (`constant_verifier.dart:253-258`): se `node.isConst`
     (const escrito **ou** contexto constante), `_checkForConstWithTypeParameters(constructorName.type, CONST_WITH_TYPE_PARAMETERS)`.
  2. `visitConstructorReference` (`:194-201`): se `inConstantContext || inConstantExpression`,
     `_checkForConstWithTypeParameters(constructorName.type, …_CONSTRUCTOR_TEAROFF)`.
  3. `visitFunctionReference` (`:224-238`): idem, para cada argumento de tipo **escrito** → `…_FUNCTION_TEAROFF`.
  4. `visitGenericFunctionType` (`:240-250`): tipo de função genérico que é filho direto de
     `is`/`as` em contexto constante → `CONST_WITH_TYPE_PARAMETERS`.
  5. Avaliador `visitFunctionReference` (`evaluation.dart:852-910`): (a) sem argumentos de tipo
     escritos (instanciação implícita do tear-off), algum `typeArgumentTypes` (com
     `_lexicalTypeEnvironment` aplicado) com `hasTypeParameterReference` → InvalidConstant
     `…_FUNCTION_TEAROFF` no nó (a referência à função); (b) com argumentos escritos, cada um é
     avaliado (`visitNamedType` → `_getConstantValue` → `TypeParameterElement` fora do ambiente
     léxico → `CONST_TYPE_PARAMETER`, `:1847-1859`) e convertido para `…_FUNCTION_TEAROFF` no
     argumento. Sai pelo `_reportError` (código específico, `:672`), no inicializador de const,
     valor padrão etc.
- **Condição `_checkForConstWithTypeParameters`** (`:554-602`): NamedType cujo elemento é
  `TypeParameterElement` não permitido → reporta e para; senão recursão nos argumentos de tipo;
  GenericFunctionType: acrescenta os próprios parâmetros de tipo aos permitidos, checa limites,
  retorno, parâmetros `SimpleFormalParameter`. Registro (`RecordTypeAnnotation`) NÃO é checado.
- **`inConstantExpression`** (`:1417-1450`): sobe a árvore; verdadeiro se passa por valor padrão
  de `DefaultFormalParameter`, ou por inicializador de campo de instância (não estático) de
  `ClassDeclaration` cuja classe tem construtor gerador const. **Não** para enum/mixin/extension type.
- **Posição:** o `NamedType` do parâmetro de tipo (`T`, `T?` inclui `?`); para a instanciação
  implícita (5a), o nó da função (`f`, `prefix.c01`); para criação de const com erro avaliado
  num inicializador (`const A<V>(true)` em `h<V>`), o erro do inicializador é recolocado no
  nó da criação (`InvalidConstant.copyWithEntity(…, _errorNode)`, `evaluation.dart:2773`) → 200:5.
- **Mensagens:** "A constant creation can't use a type parameter as a type argument." /
  "A constant constructor tearoff can't use…" / "A constant function tearoff can't use…".
- **Supressões:** nenhuma; 2/3 coexistem com o erro do avaliador no mesmo ponto (deduplicado).
  `A<T>.new;` fora de contexto const: nada. `final x = f<U>;` sem construtor const: nada.
- **No DartForge:** não existia. Implementado (sem validar) em `verificador.rs::expr`
  (InstanceCreation const, Call implícita em contexto const, Property resolvida
  `Resolved::Constructor` = ConstructorReference, TypeArguments cujo alvo não é tipo =
  FunctionReference, Is/As com `TypeKind::Function`), `checar_parametros_de_tipo`,
  campo `expr_const` (ligado em valores padrão e inicializadores de campo de classe com
  gerador const) e em `avaliador.rs` (`TypeArguments` avalia os argumentos;
  `instanciacao_implicita` usa `UnitBodyTypes::instanciacoes_de_tearoff` + `cx.tipos`).
  Atenção: o alvo de `Call` não é Constructor/FunctionReference (pular).

##### `const_with_non_const` (perda 37: FN 31, FP 6)
- **Emissão:** dois caminhos iguais (deduplicados): `ErrorVerifier._checkForConstWithNonConst`
  (`analyzer/lib/src/generated/error_verifier.dart:2993-3010`, chamado em
  `visitInstanceCreationExpression:1113` quando `node.isConst` e o tipo é InterfaceType) e o
  avaliador `_InstanceCreationEvaluator.evaluate` (`evaluation.dart:3078-3088`).
- **Condição:** `constructorName.staticElement != null && !element.isConst`.
  Construtores encaminhados de aplicação de mixin (`class B = A with M;`):
  `isConst = superCtor.isConst && !mixins.any(m.element.fields.any(!isSynthetic))`
  (`analyzer/lib/src/dart/element/element.dart:612-614`) — **qualquer** campo declarado
  (inclusive estático) num mixin torna o encaminhado não-const; factories não são encaminhadas.
- **Posição:** a palavra `const` (keyword) se escrita; senão o nó da criação inteira.
- **Mensagem:** "The constructor being called isn't a const constructor." (sem argumentos).
- **No DartForge:** `avaliador.rs::avaliar_chamada` só via avaliação; FN = 30 casos de
  `mixin_constructor_forwarding/*` + `ConstWithNonConst__mixinApplication`: a inferência
  (`inferencia/chamadas.rs::construir`, "Construtor encaminhado de aplicação de mixin: sem
  resolução") não grava `Resolved::Constructor` quando o construtor é da superclasse, e
  `verificador.rs::criacao_constante` desiste sem resolução. Correção: no verificador, criação
  const não resolvida cuja classe (do `ty`) é `ClassKind::MixinApplication` → achar o construtor
  da superclasse com o mesmo nome; não-const, ou algum mixin com campo declarado → relatar na
  palavra `const`. FP (6): (a) `bool.fromEnvironment`/`int.fromEnvironment` (4 em
  `const_eval_throws_exception/*fromEnvironme*`) — o SDK carregado aplica o patch
  `_internal/js_dev_runtime/patch/core_patch.dart:454` (`factory bool.fromEnvironment` sem
  `const`) sobre `core/bool.dart:67` (`external const factory`); o analyzer não usa patches.
  Correção: `const_` do construtor deve vir da declaração original (não do patch) — no
  avaliador, para função de unidade `UnitRole::Patch`, consultar a declaração na unidade
  original; (b) `ExperimentalMemberUse__incorrectlyNeste_0e44e3bf.dart:16:11` — parâmetros com
  erro de sintaxe (`{this.y = false}` aninhado): nossa recuperação perde o `const` do construtor
  (ou o resolve para outro); o analyzer mantém o construtor const. Investigar na sonda; se
  for o parser, é de outro dono — no mínimo não relatar CONST_WITH_NON_CONST quando o construtor
  alvo tem erro de sintaxe na lista de parâmetros.

##### `const_not_initialized` (perda 29: FN 29)
- **Emissão:** (a) parser: `parseFieldInitializerOpt` (`_fe_analyzer_shared/lib/src/parser/parser_impl.dart:3943-3948`)
  `ConstFieldWithoutInitializer` → `CONST_NOT_INITIALIZED` via
  `analyzer/lib/src/fasta/error_converter.dart:77-83` — vale para campos (estáticos ou não,
  `external` inclusive) e variáveis de topo; (b) `ErrorVerifier._checkForFinalNotInitialized`
  (`generated/error_verifier.dart:3576-3600`) para variáveis locais (`visitVariableDeclarationStatement:1638`)
  e de topo (`:1558`).
- **Condição:** lista `const` com variável sem inicializador e nome não sintético
  (`external const int v;` também sai — vem do parser).
- **Posição:** o nome da variável (`atToken(variable.name)`).
- **Mensagem:** "The constant '{0}' must be initialized." {0} = nome.
- **Supressões:** `_isInNativeClass` / lista sintética. Duplicata parser+verificador deduplicada.
  Coexiste com `const_instance_field`, `not_initialized_non_nullable_instance_field`, etc.
- **No DartForge:** não existia; implementado (sem validar) em
  `verificador.rs::const_nao_inicializada` chamado em topo, campos e locais (não em `for`).

##### `const_with_undefined_constructor` (perda 25: FN 25)
- **Emissão:** `ErrorVerifier._checkForConstWithUndefinedConstructor`
  (`generated/error_verifier.dart:3020-3045`), de `visitInstanceCreationExpression:1114` com
  `node.isConst` e tipo InterfaceType.
- **Condição:** `constructorName.staticElement == null`; com `.name` escrito → este código;
  sem → `..._DEFAULT` (ver abaixo).
- **Posição:** o nome do construtor (`constructorName.name`, ex. `noSuchConstructor`, `foo`).
- **Mensagem:** "The class '{0}' doesn't have a constant constructor '{1}'." {0} =
  `namedType.qualifiedName` (com prefixo: `a.Future`), {1} = nome escrito.
- **Casos:** 5 em `analyzer/const_with_undefined_constructor/*` (classe, prefixo, enum com
  método/constante de mesmo nome); 20 em `dot_shorthands/equality/equality_ctor_error_test.dart`
  (oráculo 3.13.4: `const .constRegular(1)` sem tipo de contexto → classe `Object`, posição no
  identificador depois do ponto).
- **No DartForge:** não existe. Lugar: `verificador.rs::expr` (InstanceCreation const sem
  `Resolved::Constructor` e cujo `ty` resolve para classe) — precisa saber que o tipo resolveu
  (classe) e o construtor não: usar `program.lookup_na_unidade`/prefixo no `ty` e
  `class.constructors`. Para atalhos de ponto (`DotShorthand { const_: true }`) a classe é o
  `Resolved::Element(Class)` do nó.

##### `const_eval_throws_exception` (perda 21: FN 20, FP 1)
- **Emissão:** `evaluateAndFormatErrorsInConstructorCall` (`evaluation.dart:335-373`): erro com
  `isRuntimeException` vira `CONST_EVAL_THROWS_EXCEPTION` no `errorNode` da criação; e os
  vários `InvalidConstant.forEntity(_errorNode, CONST_EVAL_THROWS_EXCEPTION)` do
  `_InstanceCreationEvaluator` (campo inicializado duas vezes `:2712-2716`, `:2984-2987`;
  `fromEnvironment`/`Symbol` com argumentos errados `:2491-2496`, `:2525-2531`).
  `castToType` (DartObjectComputer, `as` falhando) lança exceção de execução.
- **Casos FN:** (a) constantes de enum (7): o valor de uma constante de enum é a avaliação do
  construtor (`ConstFieldElementImpl` com inicializador sintético `InstanceCreationExpression`;
  `errorNode` = a `EnumConstantDeclaration` inteira, ex. `v(a)` offset 35 len 4); relatado por
  `visitEnumConstantDeclaration` (`constant_verifier.dart:204-217`) com `_reportError(result, null)`
  (só códigos específicos). Nós não avaliamos o construtor de enum (`valor_de_enum`).
  (b) `x as T` / `x as List<T>` em inicializador com `T` do ambiente léxico de tipos (4): o
  nosso `As` desiste quando o tipo alvo menciona parâmetro — tem de aplicar `cx.tipos` antes.
  (c) `fromEnvironment` com argumento errado (3): bloqueado pelo FP de `const_with_non_const`
  (patch do SDK). (d) `ConstConstructorFieldTypeMismatchContex_0a10b92c` (`final T x = y;` com
  `C<String>`): `_checkFields` (`:2619-2650`) usa `isRuntimeException = hasTypeParameterReference(field.type)`
  com o tipo **declarado** (T), não o substituído — nós calculamos sobre o substituído →
  relatamos `CONST_CONSTRUCTOR_FIELD_TYPE_MISMATCH` (FP) em vez da exceção na criação.
  (e) `nnbd/const/potentially_constant_types_error_test` (5): `as`/`is` com tipo genérico da
  classe (mesma causa de (b)).
- **FP (1):** `const/syntax_test.dart:109:20` `static const X = const C1();` num ciclo — o
  analyzer dá `recursive_compile_time_constant` no nome e o construtor em ciclo devolve
  desconhecido (`evaluation.dart:3090-3099`); nós avaliamos e lançamos. Resolve com o grafo de dependências.
- **Posição/mensagem:** nó da criação (ou da constante de enum); "Evaluation of this constant expression throws an exception."

##### `const_initialized_with_non_constant_value` (perda 20: FN 6, FP 6, pos 8)
- **Emissão:** `ConstantVerifier.visitVariableDeclaration` (`constant_verifier.dart:476-511`):
  `_reportError(element.evaluationResult, CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE)` para
  `const`; só códigos não específicos viram este código, no offset/len do erro.
- **Achado central (FN 4):** o `evaluationResult` de variáveis **não locais** (topo, estáticas,
  campos finais de instância, parâmetros, inicializadores de construtor, argumentos de anotação)
  é calculado sobre o inicializador do **elemento** vindo do linker, em que qualquer
  expressão que contenha `ForElement`, `FunctionExpression`, `PatternAssignment` ou
  `SwitchExpression` (em qualquer profundidade) é trocada inteira por um identificador marcador
  `_notSerializableExpression` (`analyzer/lib/src/summary2/not_serializable_nodes.dart:18-58`,
  aplicado em `summary2/detach_nodes.dart:104-112` e `:36-52`, `:129`), que recebe o offset/len
  da expressão original. Avaliar o marcador dá `genericError` (`_getConstantValue` sem
  elemento) → `INVALID_CONSTANT` não específico → `CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE`
  no **inicializador inteiro** (`const x = [for …]` → 1:11 len 31), além de
  `CONST_EVAL_FOR_ELEMENT` do `_ConstLiteralVerifier` (`:1095-1100`) na AST real. Locais
  (`const` dentro de corpo) usam a AST real (sem marcador). Regra para nós: no avaliador, ao
  calcular o valor de variável não local / valor padrão de parâmetro usado numa chamada /
  inicializador de construtor / argumento de anotação, se a expressão contém for-element,
  função literal, atribuição por padrão ou `switch` expressão → `INVALID_CONSTANT` genérico no
  intervalo da expressão inteira.
- **FN outros:** `method/not_found_test.dart:11:24` (`const B()` com `B` método da classe →
  construtor não resolve → `INVALID_CONSTANT` no nó; investigar por que não sai — provável
  `tipos_invalidos`/resolução); `dot_shorthands` (posições).
- **pos (8):** `dot_shorthands/equality/equality_error_test.dart` — atalho de ponto não
  resolvido: o nó do erro é o identificador **depois do ponto** (`.red` → coluna do `r`); nós
  usamos o span com o ponto. Corrigir no `ExprKind::DotShorthand` do avaliador (span do `name`).
- **FP (6):** fromEnvironment (patch do SDK); `ConstEvalTypeString__length_unresolvedType`
  (`B(x)` com `x` indefinido: o argumento não resolvido vira objeto inválido do tipo do
  parâmetro e a avaliação segue; erro final é `CONST_EVAL_TYPE_STRING` na criação — ver esse
  código); `deferred` (`self.E.f` → `_getDeferredLibraryError`, `evaluation.dart:1878-1932`,
  código `..._FROM_DEFERRED_LIBRARY` no identificador); `ConstWithNonConst__mixinApplication_con_b74b8b3c`
  (`class B = A with M` com M sem campos é const → `const B()` válido; nós sem resolução
  → genérico); `ExperimentalMemberUse` (recuperação); `variable/bad_initializer1_test.dart:17:7`
  (`elems` lido dentro do próprio inicializador: ciclo → só `recursive_compile_time_constant`
  no nome e o uso devolve `InvalidConstant(avoidReporting)`).

##### `const_with_non_constant_argument` (perda 19: FN 19)
- **Emissão:** `_validateConstantArguments` (`constant_verifier.dart:779-787`) com
  `_evaluateAndReportError(arg, CONST_WITH_NON_CONSTANT_ARGUMENT)`, chamado de
  `visitAnnotation` (`:103-128`, construtor const com argumentos) e de
  `visitEnumConstantDeclaration` (`:204-212`). E `genericError` de `value.dart:2476-2484`
  (argumento posicional direto de criação const — já temos em `args_de_criacao`).
- **Posição:** a expressão do argumento (sem o rótulo do nomeado).
- **Casos:** anotações (16: `@A(v)`, `@Annotation(foo)` em parâmetros de tipo de
  classe/mixin/extensão/função/método/typedef, com `foo` indefinido no escopo); constante de
  enum `v(a)` com `a` não const (1).
- **No DartForge:** o verificador não visita anotações (`metadata` de decl, membro, parâmetro,
  parâmetro de tipo, constante de enum, diretivas) e, nas constantes de enum, só desce nos
  argumentos (`self.expr`) sem `avaliar_e_relatar`. Precisa: (1) visitar todas as anotações;
  para anotação que resolve para construtor const com argumentos, avaliar cada argumento com
  padrão `CONST_WITH_NON_CONSTANT_ARGUMENT`; (2) enum: idem nos argumentos. Pré-requisito:
  a inferência resolver as expressões dos argumentos de anotação (`Resolved`/`tipos_invalidos`) —
  conferir se `BodyTypes` cobre `ast::Annotation.arguments` (o `undefined_identifier` do oráculo
  sugere que sim no analyzer; nós também emitimos? ver amostras de `undefined_identifier`).

##### `invalid_constant` (perda 17: FN 3, FP 6, pos 8)
- **Emissão:** `_reportNotPotentialConstants` (`constant_verifier.dart:749-763`) em
  `_validateConstructorInitializers` (`:790-808`) e o default de `INVALID_CONSTANT` nos
  `genericError`.
- **FN:** (a) `const <X, String?>{}` em inicializador de construtor const: `_typedLiteral`
  (`potentially_constant.dart:327-362`) exige que os argumentos de tipo de mapa sejam tipos
  **constantes** (`isConstantTypeExpression`, parâmetro de tipo não vale) e os de lista/conjunto
  *potencialmente* constantes; nós não olhamos tipos em `potencial.rs`. Também `as`/`is`
  (`:146-172`, `isPotentiallyConstantTypeExpression`) e argumentos de tipo de
  ConstructorReference/FunctionReference/TypeLiteral (`:187-203`). (b) construtor primário de
  extension type `this : assert(fn(p) > 0)` (3.13). (c) `string/interpolation1_test.dart:14:35`
  (`"$"` com erro de sintaxe: identificador ausente sintético → genérico no ponto).
- **FP (6):** `dot_shorthands/equality/equality_ctor_error_test.dart:11:14…` — `assert(const .constRegular(1) == ctor)`:
  `potencial.rs` não trata `ExprKind::DotShorthand { const_: true }` (é `InstanceCreationExpression`
  const no analyzer → potencialmente constante); só o não-const é nó ruim.
- **pos (8):** atalho de ponto não resolvido — posição no identificador sem o ponto (ver acima).

##### `invalid_type_argument_in_const_literal` (perda 16: FN 16)
- **Emissão:** `TypeArgumentsVerifier.checkListLiteral/checkMapLiteral/checkSetLiteral`
  (`analyzer/lib/src/error/type_arguments_verifier.dart:158-219`), chamado do `ErrorVerifier`
  (`visitListLiteral`/`visitSetOrMapLiteral`); `_checkTypeArgumentConst` em `:501-545`.
- **Condição:** literal com argumentos de tipo escritos e `node.isConst` (const escrito ou
  contexto constante). Para cada argumento: NamedType cujo **tipo** é `TypeParameterType` →
  relata (com nome lexeme); senão recursão nos argumentos; GenericFunctionType: primeiro os
  parâmetros simples (tipo param → relata com `[type]`), depois o retorno; RecordTypeAnnotation:
  cada campo. Diferente de `const_with_type_parameters`: os parâmetros de tipo **do próprio**
  tipo de função genérico também contam (não há conjunto permitido).
- **Posição:** o nó do tipo (`E`, `E?`).
- **Mensagem:** "Constant {list|map|set} literals can't use a type parameter in a type argument,
  such as '{0}'." — códigos únicos `INVALID_TYPE_ARGUMENT_IN_CONST_LIST/MAP/SET`, nome
  `invalid_type_argument_in_const_literal`. {0}: NamedType → `name2.lexeme` (sem `?`);
  função/registro → `DartType` (com `?` se anulável).
- **No DartForge:** não existe. Lugar natural: `verificador.rs::expr` (List/SetOrMap com `c`
  verdadeiro e `type_args` não vazios), usando `Motor::parametro_de_tipo`; conjunto vs. mapa
  pelo tipo estático (como já se faz para `TipoLiteral`) ou pela contagem de argumentos.

##### `non_constant_relational_pattern_expression` (perda 16: FN 4, pos 12)
- **Emissão:** `visitRelationalPattern` (`constant_verifier.dart:381-388`).
- **FN (4):** `pattern_variable_constant_scope_test` — `case == b && var b:`: `b` referencia a
  variável de padrão declarada depois (referenced_before_declaration); o analyzer resolve `b`
  para essa variável local não-const → genérico no `b`. Nós provavelmente resolvemos `b` para
  outro `b` (o de fora, const) ou não resolvemos. Conferir a resolução na inferência.
- **pos (12):** atalhos de ponto (dot shorthand) em condicional: (a) posição do identificador
  sem ponto; (b) com condição `true`, `potencial.rs` acusa o ramo `else` (`const .constNamed`)
  porque não trata `DotShorthand` const → corrigir `potencial.rs`.

##### `recursive_constant_constructor` (perda 14: FN 14)
- **Emissão:** `visitConstructorDeclaration` (`constant_verifier.dart:165-189`): construtor com
  `const`, não factory, `!element.isCycleFree` → relata em `node.returnType`.
- **Condição (`isCycleFree`):** falso quando o construtor está num SCC do grafo de dependências
  (`dart/constant/compute.dart:62-73`, `evaluateScc`). Dependências de um construtor const
  (`evaluation.dart:247-287`): redirecionamento const (só ele); senão (não factory) as
  referências dos inicializadores (`ReferenceFinder`, `utilities.dart`: identificador que
  resolve para variável const; criação const → construtor const; `this(...)`/`super(...)` →
  construtor alvo), o construtor sem nome da superclasse se const e não há `super`/`this`
  explícito, **todos os campos finais/const não estáticos com inicializador**, e os parâmetros
  (valores padrão). Variável: as referências do inicializador. Constante de enum: idem
  (inicializador sintético `E(args)` → construtor). Sementes: `ConstantFinder`
  (`utilities.dart:123-200`).
- **Posição:** `returnType` do construtor (o nome da classe no construtor; `new` em `const new()`;
  nome da classe no cabeçalho do construtor primário). Len = só o identificador.
- **Mensagem:** "The constant constructor depends on itself."
- **No DartForge:** não existe. Precisa do grafo (Tarjan sobre variáveis const, campos finais com
  inicializador de classes com construtor const, construtores const, parâmetros com padrão,
  constantes de enum, anotações) no `Motor`, antes da avaliação, marcando `ciclicos` e
  construtores fora de ciclo. Casos: auto-redirecionamento `this()`, `this.named()` entre
  construtores de enum (primário 3.13), `final m = const A()` no próprio A, `const C(): x = y`
  com `y = const C()`.

##### `const_constructor_with_mixin_with_field` (perda 13: FN 13)
- **Emissão:** `ErrorVerifier._checkForConstConstructorWithNonConstSuper`
  (`generated/error_verifier.dart:2807-2856`), de `visitConstructorDeclaration:597`.
- **Condição:** `_enclosingClass != null`, construtor const, não factory; campos de instância
  de cada mixin da cláusula `with` (ordem dos mixins, ordem de declaração): não estático, não
  sintético, e **não** (`abstract` **e** `final`) — `abstract int a;` conta, `var`, `final a = 0` contam.
  1 campo → `CONST_CONSTRUCTOR_WITH_MIXIN_WITH_FIELD` ['A.a' entre aspas simples]; >1 →
  `…_FIELDS` com a lista `'A.a', 'A.b'` (nome `const_constructor_with_mixin_with_field`). Se
  relatado, não checa non-const super nem non-final field.
- **Posição:** `constructor.returnType` (nome da classe no construtor, `new`, ou nome da classe
  no construtor primário). Len 1 nos exemplos (`B`).
- **Mensagens:** "This constructor can't be declared 'const' because a mixin adds the instance
  field: {0}." / "…because the mixins add the instance fields: {0}." ({0} já com aspas).
- **No DartForge:** não existe. Lugar: `verificador.rs::construtor` (k.const_ && !k.factory),
  classe via `classe_de`, mixins `program.class(k).mixin_classes`, campos `fields` + flags;
  `abstract` do campo só no AST (`VariableList.abstract_`).

##### `const_eval_property_access` (perda 12: FN 1, FP 11)
- **Emissão:** `_evaluatePropertyAccess` (`evaluation.dart:1686-1730`).
- **FP (11):** `A<T>.new`, `Class.named`, `prefix.GenericClass<int>.new` como valor: são
  `ConstructorReference` (`visitConstructorReference`, `evaluation.dart:804-840`: valor
  função), não acesso a propriedade; nós avaliávamos o alvo como `Type` e acusávamos
  `.new`. Corrigido (sem validar) em `avaliador.rs::propriedade` (Property com
  `Resolved::Constructor` → valor função).
- **FN (1):** `d.length` com `d` do tipo `dynamic` cujo valor é `List` (construtor primário 3.13)
  — exige avaliar o construtor primário (fora de escopo agora).
- **Mensagem:** "The property '{0}' can't be accessed on the type '{1}' in a constant
  expression." {1} = `targetType.getDisplayString()` (String já formatada: sem alias).

##### `constant_pattern_with_non_constant_expression` (perda 12: FN 10, pos 2)
- **Emissão:** `visitConstantPattern` (`constant_verifier.dart:131-141`).
- **FN:** (a) `if (x case a)` com `a` local `var`/top-level `final` (2): nosso parser guarda
  `case a` como `PatternKind::Variable` sem `final/var/tipo`; o verificador só usa isso para
  a exaustividade. Regra: num `case`, identificador simples é padrão constante; avaliar a
  referência (local não const / variável não const → genérico no nome). Precisa da resolução
  do nome (escopo local) — hoje não há `ExprId`. (b) `++a`, `--a`, `++variable` (5): o analyzer
  avalia o **operando** primeiro (`visitPrefixExpression`, `evaluation.dart:1142-1172`) → erro
  no `a` (3:16); nós devolvemos genérico no nó inteiro antes de avaliar o operando
  (`avaliador.rs::unario`). (c) `const void fun() {}` (305:26: a função literal depois do nome
  descartado) e `const assert(false)` (316:18) — recuperação do parser.
- **pos (2):** idem (c), pareamento por mensagem.

##### `recursive_compile_time_constant` (perda 12: FN 12)
- **Emissão:** `ConstantEvaluationEngine.generateCycleError` (`evaluation.dart:406-440`), via
  `evaluateScc` (`compute.dart:62-73`): cada **variável** do SCC recebe o erro no próprio
  elemento (`atElement`: o nome) e `evaluationResult = InvalidConstant(RECURSIVE…)`;
  `_reportError` também o deixa passar (específico).
- **FN:** constantes de enum em ciclo (`v1(v2), v2(v1)`, `v(values)` — `values` depende de
  todas as constantes) → no nome da constante (len do nome); campo `final m = const A()` em
  classe com `const A()` (ciclo campo↔construtor); `const y = const C()` com `C(): x = y`;
  `static const X = const C1()` (syntax_test:109:16); local `const elems = const [… elems …]`
  (bad_initializer1_test:8:9). Nós só detectamos ciclo variável→variável durante a avaliação.
- **Mensagem:** "The compile-time constant expression depends on itself."
- **No DartForge:** mesmo grafo de `recursive_constant_constructor`.

##### `const_constructor_with_non_const_super` (perda 11: FN 11)
- **Emissão:** mesmo método (`error_verifier.dart:2858-2893`), depois do teste de mixins.
- **Condição:** classe não enum; construtor sem redirecionamento (`redirectedConstructor == null`);
  `invokedSuper = element.superConstructor` (o do `super(...)`/`super.nome(...)` escrito, ou o
  sem nome da superclasse implícito); relata se `invokedSuper != null && !invokedSuper.isConst`.
- **Posição:** o `SuperConstructorInvocation` se escrito (de `super` até `)`, ex. 3:14 len 7
  `super()`), senão `returnType`.
- **Mensagem:** "A constant constructor can't call a non-constant super constructor of '{0}'."
  {0} = `element.enclosingElement3.displayName` → no 3.6.2 é a **classe do construtor**
  ('B'); o oráculo 3.13.4 (arquivos com sintaxe nova: `const new()`, construtor primário)
  mostra a superclasse ('A'). Para nós: classe envolvente, exceto quando a biblioteca usa
  sintaxe nova (heurística: construtor primário ou `new` como nome) → superclasse.
- **No DartForge:** não existe. Superclasse via `supertype_class` (com mixins à parte em
  `mixin_classes`), construtores em `class.constructors` (`""` = sem nome); Object → const.

##### `non_const_generative_enum_constructor` (perda 11: FN 11)
- **Emissão:** `ErrorVerifier._checkForNonConstGenerativeEnumConstructor`
  (`generated/error_verifier.dart:4710-4719`) de `visitConstructorDeclaration:594`.
- **Condição:** classe envolvente enum, sem `const`, sem `factory`.
- **Posição:** `atConstructorDeclaration` (`analyzer/lib/error/listener.dart:72-95`): de
  `returnType.offset` até o fim do nome se houver nome (`E.named` len 7), senão só o
  `returnType` (`E`, `new`).
- **Mensagem:** "Generative enum constructors must be 'const'."
- **Casos:** construtores comuns; `augment` (3.13, com recuperação); construtor com tipo de
  retorno recuperado (`static int ConflictClassStatic()` → nome na coluna do nome).
- **No DartForge:** não existe; lugar: `verificador.rs::construtor` (enum, !const, !factory),
  span `class_name.start .. name.end` (ou `class_name`).

##### `const_constructor_with_non_final_field` (perda 9: FN 9)
- **Emissão:** `_checkForConstConstructorWithNonFinalField` (`error_verifier.dart:2898-2916`),
  só se o teste de super/mixin não relatou.
- **Condição:** construtor const e gerador; classe é `ClassElement` (não enum/mixin/extension
  type); `hasNonFinalField` (`dart/element/element.dart:329-358`): BFS na classe, seus mixins e
  superclasses — algum campo não final, não const, não estático, não sintético (inclui
  `abstract int x;`).
- **Posição:** `atConstructorDeclaration` (como acima: `A`, `A.named`, nome da classe no primário).
- **Mensagem:** "Can't define a const constructor for a class with non-final fields."
- **No DartForge:** não existe; `verificador.rs::construtor`.

##### `const_type_parameter` (perda 9: FN 9)
- **Emissão:** `_getConstantValue` → `TypeParameterElement` sem entrada no ambiente léxico
  (`evaluation.dart:1847-1859`, `errorNode2`) e `visitNamedType` com
  `isTypeLiteralInConstantPattern && hasTypeParameterReference` (`:1066-1074`).
- **FN:** (a) padrões constantes `case T` (2:14) e `const (List<T>)` (2:21, o NamedType
  `List<T>` len 7); (b) anotações em parâmetros de tipo `@Annotation(T)` (7) — exige visitar
  anotações (ver `const_with_non_constant_argument`).
- **No DartForge:** (a) `case T`: o parser guarda como `PatternKind::Variable` (ver acima);
  `const (List<T>)`: `TypeArguments` em padrão constante — no avaliador, literal de tipo
  genérico dentro de padrão constante com argumento que é parâmetro de tipo →
  `CONST_TYPE_PARAMETER` no tipo (usar `Motor::parametro_de_tipo`).

##### `const_constructor_field_type_mismatch` (perda 8: FN 1, FP 7)
- **Emissão:** `_checkFields` (`evaluation.dart:2619-2650`, erro no inicializador do campo,
  `isRuntimeException = hasTypeParameterReference(field.type declarado)`) e `_checkInitializers`
  (`:2730-2752`, erro no inicializador se estaticamente atribuível, senão no `_errorNode`).
- **FP (7):** (a) `ConstConstructorFieldTypeMismatchContex_0a10b92c`: usar o tipo declarado para
  `isRuntimeException` (ver throws_exception (d)); (b) `instantiated_function_constant(_error)_test`
  (6): `x7 = b ? f1 : f2` — o valor de `f1` com instanciação implícita deve ter o tipo
  instanciado **e substituído** pelo `_substitution` do visitante (`_instantiateFunctionType`,
  `evaluation.dart:1939-1962`); nós usamos o tipo estático com `U` livre → falso desencontro.
- **FN (1):** constante de enum `v;` com `const E() : x = ''` → precisa avaliar o construtor da
  constante de enum (erro no `_errorNode` = a constante, 2:3 len 1).

##### `const_eval_method_invocation` (perda 8: FP 8)
- **Emissão:** `visitMethodInvocation` (`evaluation.dart:1023-1063`).
- **FP:** criação implícita de **extension type** (`A(true)`, `Bool(true)`, `_ConstE(0)`,
  `E(0)`): a inferência não grava `Resolved::Constructor` para o construtor primário de
  extension type (`construtor_ou_primario` → `Some(None)`), então o `Call` cai em "invocação de
  método". No analyzer é `InstanceCreationExpression` (avalia para o valor da representação).
  E construtor gerador de enum chamado (`NoConstructorCalls(3)`, erro
  `invalid_reference_to_generative_enum_constructor`): o analyzer resolve o construtor e não dá
  erro de avaliação. Correção no avaliador: `Call` cujo alvo resolve para classe
  (`Element::Class` de extension type/enum, ou tipo estático do nó = esse tipo) é criação;
  para extension type devolver o valor do argumento da representação.

##### `const_eval_type_bool` (perda 6: FP 6)
- **Emissão:** `visitConditionalExpression` (`evaluation.dart:757-800`).
- **FP:** `string/multiline_newline_test.dart` — `const c1 = cr.s == crlf.s ? true : null;` com
  strings multilinha em arquivos CR/CRLF/LF: o Dart normaliza CR e CRLF para LF em literais
  multilinha; nosso texto não normaliza → `==` falso → `c1 = null` → condição não-bool.
  Correção na origem (`crates/frontend`, `ast::StringPart::Text` de `'''`/`"""`) ou em
  `avaliador.rs::string` para literais multilinha.

##### `const_constructor_param_type_mismatch` (perda 4: FN 2, FP 2)
- **Emissão:** `_checkParameters` (`evaluation.dart:2870-2960`).
- **FN (2):** argumentos de constantes de enum (`v(0)` para `String`, `v('')` para
  `String this.x` com campo `int`) — precisa avaliar construtores de enum; posição no argumento.
- **FP (2):** `const B({super.a})` sem argumento: parâmetro super sem padrão próprio herda o
  padrão do parâmetro do super (`SuperFormalParameterElementImpl`), não `null`; nós passamos
  `null` → desencontro. Corrigir `chamar_gerador` (super param opcional sem padrão → padrão do
  parâmetro correspondente do construtor super; se não há, omitir).

##### `const_constructor_with_field_initialized_by_non_const` (perda 3: FN 1, FP 2)
- **Emissão:** `_validateFieldInitializers` (`constant_verifier.dart:842-881`) no token `const`.
- **FN:** `final x = f<U>;` (4562b1b6): resolve com a avaliação de `TypeArguments` (já feita).
- **FP:** `final x = A<T>.new;` (resolvido pela ConstructorReference) e `final int i = d.length`
  (primário 3.13: `d` parâmetro do construtor primário — escopo de inicializador primário).

##### `non_constant_default_value` (perda 3: FP 3)
- **Emissão:** `_validateDefaultValues` (`constant_verifier.dart:811-840`): se o tipo estático
  do padrão é `InvalidType` não avalia.
- **FP:** `{int x = X}` com `X` indefinido (2): o analyzer não relata (InvalidType) — nosso
  `tipos_invalidos` não marca o identificador indefinido nesse caso; e `A({E a = const E(0)})`
  com extension type (criação de extension type não resolvida, ver method_invocation).

##### `non_constant_type_argument` (perda 3: FN 3)
- **Emissão:** FFI, `FfiVerifier` (`analyzer/lib/src/generated/ffi_verifier.dart:1138`, `:1214`):
  `Pointer.ref`/`asFunction<R>()`/`allocate` com argumento de tipo que é parâmetro de tipo ou
  não nativo; mensagem "The type arguments to '{0}' must be known at compile time, so they can't
  be type parameters." Não é do verificador de constantes; fica para a família FFI.

##### `const_with_undefined_constructor_default` (perda 2: FN 2)
- Mesmo `_checkForConstWithUndefinedConstructor` (`error_verifier.dart:3036-3043`): sem nome de
  construtor → no `constructorName` inteiro (`B`, len 1), args `[namedType.qualifiedName]`
  (o alias escrito: 'B'). "The class '{0}' doesn't have an unnamed constant constructor."

##### `non_constant_annotation_constructor` (perda 2: FN 2)
- `ConstantVerifier.visitAnnotation` (`constant_verifier.dart:103-115`): elemento é construtor não
  const → no nó `Annotation` inteiro (de `@` até `)`, 4:1 len 12). Exige visitar anotações e
  resolver o construtor da anotação (resolução de metadados).

##### `non_constant_case_expression` (perda 2: FN 2)
- `_validateSwitchStatement_nullSafety` (`constant_verifier.dart:1009-1040`), biblioteca sem
  `patterns` (< 3.0): `case ERROR_B = 1:` — atribuição a const como expressão de caso →
  genérico no nó (18:10). Nosso ramo `!padroes_ligados` em `verificador.rs::stmt` existe;
  causa provável: o parser não produz `PatternKind::Constant(Assign)` nesses arquivos legados
  (ou `desparentizar`/tipo inválido corta) — conferir na sonda.

##### `const_constructor_throws_exception` (perda 1: FN 1)
- `ErrorVerifier._checkForConstEvalThrowsException` (`error_verifier.dart:2936-2943`) de
  `visitThrowExpression:1550`: `throw` dentro de construtor const → no `throw` inteiro
  (`throw ''`, 2:36). Lugar: `verificador.rs::expr` com "construtor const envolvente"
  (inicializadores e corpo do construtor const).

##### `const_eval_type_string` (perda 1: FN 1)
- `o.length` em inicializador (`B(String o) : l = o.length`) chamado com argumento não
  resolvido `B(x)`: `_valueOf` (`evaluation.dart:2005-2025`) dá objeto **inválido** do tipo do
  parâmetro (String sem valor); `stringLength` de objeto inválido lança `CONST_EVAL_TYPE_STRING`
  (value.dart `StringState`/`isInvalid`) → recolocado na criação (8:11). Nosso
  `valor_de` cria `Estado::Null { invalido: true }`, que não é String → `length` vira
  `CONST_EVAL_PROPERTY_ACCESS`/outro. Modelar "inválido de tipo T".

##### `const_initialized_with_non_constant_value_from_deferred_library` (perda 1: FN 1)
- `_getDeferredLibraryError` (`evaluation.dart:1878-1932`): referência através de prefixo
  `deferred` → código conforme o ancestral (VariableDeclaration →
  `CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE_FROM_DEFERRED_LIBRARY`) no identificador final
  (`f` em `self.E.f`, 7:16). Nós: precisamos saber que o prefixo é `deferred`
  (`Import.deferred`) em `propriedade`/`valor_constante`.

##### `non_constant_list_element` (perda 1: FP 1)
- `variable/bad_initializer1_test.dart:17:7` — `elems` dentro do próprio inicializador local:
  com o ciclo detectado, o uso devolve `InvalidConstant(avoidReporting: true)` e nada sai no
  elemento. Resolve com o grafo de dependências (ciclo de local).

---

#### Estado do código (antes da ordem de parar)
Edições minhas, ainda não compiladas (build na fila em `E:\dftemp\r3-r3-d\run1.sh`):
- `avaliador.rs`: `escopos_de_tipo`, `parametro_de_tipo`, `tipo_escrito_e_parametro`,
  `instanciacao_implicita`; `propriedade` trata ConstructorReference; `TypeArguments` avalia os
  argumentos de tipo (FUNCTION_TEAROFF).
- `verificador.rs`: `expr_const`, `checar_parametros_de_tipo`, `argumentos_de_tipo_do_construtor`,
  `e_tipo`, `const_nao_inicializada`; ramos de `expr` para criação/tear-offs/is-as; alvo de
  `Call` não é mais visitado como tear-off.
Se o build falhar, corrijo só isso (ou desfaço) antes de qualquer outra coisa.

#### Conferido com o oráculo vivo (sonda, E:\dftemp\r3-r3-d\c1)
- `const_not_initialized`: sai em `static const v;`, topo `const t;`, local `const l;`; **não** sai em `for (const i; ;)`.
- `const_with_type_parameters`: `const G<T>()` (no `T`), `const x = G<T>.new` (no `T`, CONSTRUCTOR_TEAROFF; hoje damos FP `const_eval_property_access` em `.new`), `const void Function(int) z = id<U>` (no `U`).
- `invalid_type_argument_in_const_literal`: `const y = <T>[]` (no `T`).
- `const_constructor_with_mixin_with_field`: `class C extends S with M { const C(); }` com `final int x = 0` no mixin → no `C` do construtor, 'M.x'.
- `const_constructor_with_non_const_super` (3.6.2): implícito no `D` (arg 'D', a própria classe); explícito no `super()` (len 7, arg 'D2').
- `recursive_constant_constructor`: `const R() : this.a(); const R.a() : this();` → nos dois `R` (len 1).

## E. Parser e scanner

### Especificação — família E (parser/scanner do analyzer 3.6.2) — r3-e

Fonte: `E:\references\dart-sdk-3.6.2\pkg` (citada como `fe:` = `_fe_analyzer_shared/lib/src/` e
`an:` = `analyzer/lib/src/`). Amostras: `E:\dftemp\analise\trab\placar-base-r3.txt`.
Base r3: placar 18535/23030.

#### 0. Regras transversais (valem para vários códigos)

**T1 — Qual analyzer é o oráculo de um arquivo.** O oráculo é o 3.13.4 quando (a) a biblioteca tem
versão de linguagem > 3.6 (grupo `*-3.13`, marcador `@dart` novo) ou (b) a unidade usa sintaxe que o
3.6.2 não conhece (`corpus/diagnosticos/sintaxe-nova.json`); senão é o 3.6.2. O parser consegue
decidir isso sozinho no fim da unidade: (a) é `features.versao() > PISO`; (b) é "há algum
`experiment_not_enabled` com recurso em {dot-shorthands, primary-constructors,
private-named-parameters}" (a mesma lista de `analise::duplicatas::RECURSOS_POSTERIORES_AO_3_6`).
Mensagens que mudaram entre 3.6.2 e 3.13.4 (null-aware-elements, representation_field_modifier)
escolhem-se por esse predicado (`parser/mod.rs`, pós-processamento em `parse_lexed_com`).

**T2 — `_reportFeatureNotEnabled`** (`an:fasta/ast_builder.dart:6078`): versão citada =
`feature.releaseVersion ?? ExperimentStatus.currentVersion` → recurso sem versão de lançamento no
3.6.2 cita a versão do SDK, `3.6.0`. `null_aware_elements` tem `releaseVersion: null`
(`an:dart/analysis/experiments.g.dart:400-408`). O parser do fasta (`reportExperimentNotEnabled`,
`fe:parser/parser_impl.dart:9440`) usa `x.y.0`; o AstBuilder do 3.13.4 usa `x.y` para os relatos que
ele mesmo faz (`exigir_no_ast` no nosso código).

**T3 — Erros do scanner** (`fe:scanner/errors.dart:15 translateErrorToken`): `UNTERMINATED_STRING_LITERAL`
e `UNTERMINATED_MULTI_LINE_COMMENT` em `endOffset - 1` (:36, :42); `MISSING_DIGIT`/`MISSING_HEX_DIGIT`
em `endOffset - 1` (:46-56); demais (`ILLEGAL_CHARACTER`, `UNSUPPORTED_OPERATOR`) no `charOffset` do
ErrorToken; comprimento sempre 1 (`ErrorToken.length`, `fe:scanner/error_token.dart:73`). O scanner
NUNCA para: cada erro vira `ErrorToken` prefixado (`prependErrorToken`,
`fe:scanner/abstract_scanner.dart:499`) e o token "bom" é sintetizado. O parser pula os
ErrorToken; os demais erros sintáticos continuam sendo relatados.

**T4 — Identificador com palavra-chave** (`fe:parser/identifier_context_impl.dart`): cada contexto tem
recuperação própria; em expressão (`ExpressionIdentifierContext`, :313) palavra reservada que não
abre comando vira identificador com `EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD`; `as`/`is` e o que abre
comando (`looksLikeStatementStart`) viram `MISSING_IDENTIFIER` + identificador sintético; em
interpolação `$palavra` sempre `EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD` (:347-353).

**T5 — Tipo × nome em declaração** (`fe:parser/type_info.dart:159 computeType` e
`fe:parser/type_info_impl.dart:486 looksLikeName`): `X Y` só é tipo+nome se `Y` "parece nome";
`typedef` seguido de identificador NÃO parece nome (abre a declaração seguinte). Logo
`augment typedef A = int;` e `base typedef E = Enum;` são o campo sem tipo `augment`/`base`
(`MISSING_CONST_FINAL_VAR_OR_TYPE` e `EXPECTED_TOKEN ';'` nele), e não "tipo + nome que falta".

---

##### `experiment_not_enabled` (perda 231: FN 99, FP 35, msg 96, pos 1)
- **Emissão:** três fontes. (1) parser do fasta `reportExperimentNotEnabled` (`fe:parser/parser_impl.dart:9440`)
  → `listener.handleExperimentNotEnabled` → `an:fasta/ast_builder.dart` reporta
  `ParserErrorCode.EXPERIMENT_NOT_ENABLED` com `[flag.name, versão x.y.0]`; (2) AstBuilder
  `_reportFeatureNotEnabled` (`an:fasta/ast_builder.dart:6078`) — null-aware (`handleLiteralMapEntry` :4811,
  `handleNullAwareElement` :5127), `reportErrorIfNullableType` :5830 etc.; (3) no 3.13.4, o
  AstBuilder relata os recursos 3.10–3.13 (dot-shorthands, primary-constructors, private-named-
  parameters) nos tokens próprios (T2, versão `x.y` quando é o AstBuilder).
- **Condição:** recurso desligado para a versão da biblioteca (ou experimento não habilitado).
  Recursos anteriores ao 3.6 só disparam em biblioteca antiga: `class-modifiers` e `sealed-class`
  (3.0) nos modificadores `base`/`interface`/`final`/`mixin` de classe e `sealed`;
  `generic-metadata` (2.14) no `<` de `@A<T>()`; `nonfunction-type-aliases` (2.13) no `=` de
  `typedef X = <tipo que não é Function>`.
- **Posição:** o token do recurso (len do token). Casos: `const .x(…)` → DOIS relatos, no `const` e no
  `.` (oráculo `UnusedElement__dotShorthand_private_con_96861705.dart:6:7` e `:6:13`); construtor primário:
  no `(`/`.` do cabeçalho e em cada `var` declarante (forma do AstBuilder, `x.y`); `final` declarante só
  em parâmetro-função (`final void _f()`), não em `final int x`; mapa null-aware: UM relato por
  entrada, no `?` da chave se houver, senão no do valor (`ast_builder.dart:4816-4822`).
- **Mensagem:** `This requires the '{0}' language feature to be enabled.` / correção `…minimum SDK
  constraint to {1} or higher…`. `{1}` para null-aware-elements: `3.6.0` no oráculo 3.6.2 (T2, 96
  casos), `3.8.0` no 3.13.4 — escolher por T1.
- **Supressões:** em arquivo de oráculo 3.6.2 que usa sintaxe nova (os 8 FP de
  `dot_shorthands/equality/equality_extension_override_error_test.dart`, que o 3.6.2 lê como
  `missing_identifier` no `.`), o 3.6.2 nem conhece o recurso — não é possível acertar pelo parser
  sem saber que o arquivo está fora de sintaxe-nova.json (ficam como FP aceito/documentado).
- **No DartForge:** `parser/mod.rs exigir`, `exigir_no_ast`, `exigir_versao` (novo); feito nesta
  rodada (código já escrito, compilação pendente na fila): dupla em `const .x`
  (`expressions.rs parse_dot_shorthand`), `var` declarante (`declarations.rs parse_cabecalho_primario_opt`),
  null-aware 3.6.0 (`mod.rs versao_do_null_aware_no_3_6`), um relato por entrada de mapa, `exigir_versao`
  para class-modifiers/sealed-class/generic-metadata/nonfunction-type-aliases. FP de
  `private_named_parameters/declaring_parameter_collision_error_test` e `private_optional_parameter/*class_primary*`:
  sem primary-constructors o parâmetro declarante não inicializa campo → o 3.13.4 relata
  `PRIVATE_OPTIONAL_PARAMETER`, não o recurso (`types.rs nome_publico_do_nomeado`, corrigido).
  FP `unnamed_new_error_test:35:11` (`factory new()` em 3.6): fasta lê a factory chamada `new`
  (`parseFactoryMethod`, `fe:parser_impl.dart:5104`; o ErrorVerifier relata `FACTORY_CONSTRUCTOR_NEW_NAME`)
  — corrigido em `parse_member`/`parse_constructor`.

##### `expected_token` (perda 85: FN 49, FP 33, msg 1, pos 2)
- **Emissão:** `ensureSemicolon` (`fe:parser_impl.dart:4293`, no token anterior), `ensureCloseParen` (:4234),
  `ensureBlock` (:4200), `templateExpectedButGot`/`ExpectedAfterButGot` espalhados; scanner
  (`ScannerErrorCode.EXPECTED_TOKEN`, `fe:scanner/errors.dart:70-88`) para fecho que falta no EOF.
- **Condição/posição por grupo de amostras restantes:**
  - `for (var (a, b) = e; …)` (FP `in`/`;`, 6 casos): código válido que recusamos — ver §FOR.
  - Scanner abortando (`3457e`, `0x;`, `'…\` + quebra) — ver §SCANNER; hoje o arquivo inteiro some.
  - `import<int> x = []` (FP `(`): `computeType` builtin + `<…>` + nome (`type_info.dart:164-172`) é
    tipo recuperado; hoje lemos função `import<int>`.
  - `new C(;` (FN `)` no `;`, 3 casos): `(` sem `)` casado no scanner → `)` sintético mal posto,
    `ensureCloseParen` o move e o relato sai no token corrente (`parser_impl.dart:4239-4245`).
  - `X<out String> bar;` (variance): expressão `X < out`, `;` que falta no `out`.
  - `int class = 10;` local: `;` no `int`, depois `class = 10` como expressão (T4).
  - `case 1 ?? 2:` / `case void fun() {}:` (pos): padrão constante recupera até a igualdade
    (`parsePrecedenceExpression(EQUALITY_PRECEDENCE)`), depois `ensureColon`.
  - `operator ===(…)` / `super ===` (FP): o scanner tem `===`/`!==` como token (§SCANNER).
  - `case ERROR_B = 1:` em biblioteca 2.19 (FP `:`): antes da 3.0 o `case` lê expressão, não padrão
    (`allowPatterns`); exige mudar a árvore de código 2.x válido → fora.
  - `typedef T10<X extends void Function({T10<Never> x})> = List<X>;` (FP `>`, 4): código VÁLIDO
    recusado — o `{` dentro dos parâmetros de tipo descarta os `<` abertos no nosso
    `fim_do_grupo_lt`; o fasta usa `computeTypeParamOrArg`, que reconstrói o grupo. Corrigir a
    leitura de parâmetros de tipo (não depender de `endGroup` quando há `{`).
- **Mensagem:** `Expected to find '{0}'.`
- **No DartForge:** `parser/mod.rs erro_esperado`/`garantir_*`; mensagens 3.6/3.13 iguais.

##### `missing_identifier` (perda 42: FN 22, FP 17, pos 3)
- **Emissão:** `templateExpectedIdentifier` dos `IdentifierContext` (T4); scanner `$` sem nome
  (`fe:scanner/errors.dart:88`).
- **Casos restantes:** `E(0) == .member` em oráculo 3.6.2 (FN no `.`, 8): o 3.6.2 não conhece `.x`
  → `MISSING_IDENTIFIER` no `.`; só dá para acertar sabendo que o arquivo é 3.6.2 sem sintaxe nova
  (o arquivo NÃO está em sintaxe-nova.json — inconsistência do corpus; FN aceito);
  `super.const()` no inicializador (`invalid_super_in_initializer`); `print(is String)` (T4: `is` →
  MISSING_IDENTIFIER e identificador sintético; hoje EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD);
  `typedef L = Function({x})` (nomeado sem nome em tipo de função: `{x}` é tipo `x` + nome que
  falta, no `}`); `factory C.foo() = C.bar();` (`(` depois do alvo: nome que falta no `(`);
  FP `case ++a` (padrão constante `++a`: o fasta lê `++` prefixo em constante sem erro de
  identificador); FP `C<>` (`expected_type_name`, não identificador); FP `typedef T10…` (ver
  expected_token); FP `augment typedef`/`base typedef`/`sealed typedef` (T5).
- **Posição:** o token que deveria ser o identificador (len do token; EOF len 0→1).
- **No DartForge:** `erro_identificador`, `nome_de_declaracao`, recuperação de topo em
  `parse_function_or_variables` (lista de palavras de topo — remover `typedef` quando seguido de
  identificador, T5).

##### `expected_identifier_but_got_keyword` (perda 15: FN 9, FP 5, pos 1)
- **Emissão:** T4 (`identifier_context_impl.dart`, vários contextos).
- **Casos:** `typedef void as();`, `typedef as = …;`, `typedef Function = …;` (FN 5): contexto
  `typedefDeclaration` relata `EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD` para builtin como nome (além do
  `BUILT_IN_IDENTIFIER_IN_DECLARATION` do verificador); `"$class"` (interpolação, T4 :347);
  `int class = 10;` (FN); `case` dentro de bloco (`label8`, FN); FP `() async => return x`
  (`parsePrimary` :6659: `return` → `UNEXPECTED_TOKEN` e segue, não este código); FP `case assert(false):`
  (`parsePrimary` → `parseAssert(Assert.Expression)`; `AssertAsExpression` não é relatado pelo
  analyzer, `an:fasta/error_converter.dart` caso `codeAssertAsExpression`); FP `case const void fun() {}`
  (expressão de função nomeada: `NAMED_FUNCTION_EXPRESSION` no nome, `parser_impl.dart:5288`).
- **Mensagem:** `'{0}' can't be used as an identifier because it's a keyword.`

##### `representation_field_modifier` (perda 12: msg 11, FP 1)
- **Emissão:** `endPrimaryConstructor` (`an:fasta/ast_builder.dart:2843`), :2885-2893: primeiro
  parâmetro com `keyword` ≠ `const` → no keyword.
- **Mensagem:** 3.6.2: `Representation fields can't have modifiers.`; 3.13.4: `Representation fields
  can't have the modifier '{0}'.` (arg = lexema). Escolha por T1 (os 11 msg≠ estão em
  `linguagem-3.13` ou em sintaxe-nova.json).
- **FP:** `extension type ET3(final i, final x)` no 3.13.4 com 2 parâmetros: só
  `MULTIPLE_REPRESENTATION_FIELDS` (corrigido: sem o modificador quando >1 e versão > 3.6).

##### `missing_const_final_var_or_type` (perda 8: FN 3, FP 5)
- **Emissão:** `parseFields` (`fe:parser_impl.dart:3655`): `typeInfo == noType && varFinalOrConst == null`
  → no nome; e `parseVariablesDeclaration…` em comandos.
- **FN:** `augment/base/sealed typedef X = …;` (T5). **FP:** `unnamed_new_error_test` (3.13 `new`
  como membro, versão 3.6) e `named_aggregated_test:20` (`f(a [b = 42])`: falta `,` → fasta
  `ensureCloseParen` com `EXPECTED_TOKEN ')'` em `[`, sem cascata; nós lemos `[b` como membro).

##### `unexpected_token` (perda 7: FN 7)
- **Emissão:** `templateUnexpectedToken` — `parsePrimary` `return` (:6659), `skipChainedAsIsOperators`
  (:7914), `parseAssert` (:9131), cabeçalho de mixin, `parseSwitchExpression` (:10355),
  `ensureColon` em `case … :` (o `:` que sobra depois da recuperação do padrão).
- **Restantes:** `() async => return x` (FN); `case 1 ?? 2:`/`case void fun() {}:`/`label8` (o `:` em
  `case` de padrão quebrado); mixin com construtor primário no 3.13 (já tratado).

##### `expected_class_member` (perda 6: FP 6)
- `unnamed_new_error_test:71,103` — membros `new`/`.new` da 3.13 numa biblioteca 3.6; o 3.6.2
  (oráculo) lê `C.new` como construtor nomeado `new` e não relata. Ajustar `parse_member` para
  aceitar `Nome.new` (já existe `identifier_or_new`) sem cascata.

##### `extraneous_modifier` (perda 6: FN 5, FP 1)
- **Emissão:** `modifier_context.dart` (`reportExtraneousModifier`, :130-134 e por contexto).
- **Restantes:** `primary_constructors/syntax/const_in_header_and_body_error_test` (3.13: `const`
  repetido no cabeçalho e no `this`), `required required int i` no 3.13.4 (o 3.13 relata
  extraneous no 1º além de DUPLICATED), FP `empty_body_error_test:18:6` (3.13 `class const`).
  Todos do grupo 3.13 — regras do parser 3.13.4 (fora da fonte 3.6.2).

##### `unterminated_string_literal` (perda 6: FN 6) — §SCANNER
- `abstract_scanner.dart:2055 unterminatedString`: token STRING sintético (o texto até o fim da
  linha + aspa sintética) e `UnterminatedString` em `errorStart`; relato em `endOffset - 1` (T3).
- Hoje o arquivo inteiro aborta no 1º erro léxico (`parse_lexed_com` devolve unidade vazia).

##### `expected_representation_type` (perda 5) / `expected_representation_field` (perda 4)
- **Emissão:** `endPrimaryConstructor` :2871-2877 (`leftParenthesis.next`, primeiro parâmetro sem tipo)
  e :2909-2914 (primeiro não simples).
- **Restantes:** todos em `extension_type/regress_53625_error_test` (3.13) e augmentations: a
  posição é `leftParenthesis.next` — quando há metadata/modificador antes, é o PRIMEIRO token
  depois do `(` (`@`, `required`…), não o parâmetro; nós usamos o início do parâmetro com o
  modificador já consumido (+3/+1). FN 46:26/186:26/192:20: primeiro parâmetro `final x` sem tipo
  no 3.13 (`var`/`final` sem tipo também → EXPECTED_REPRESENTATION_TYPE no 3.13.4).

##### `missing_statement` (perda 5: FN 1, FP 4)
- `parseStatement` → `ExpectedStatement`. FP: `for (var (a,b) = …)` (§FOR) e `int class = 10;` (T4).
  FN: `label8` (`case` fora de switch).

##### `unsupported_operator` (perda 5: FN 5) — §SCANNER
- `abstract_scanner.dart:1149 tokenizeExclamation`/`:1170 tokenizeEquals`: `===`/`!==` são um token
  (`EQ_EQ_EQ`/`BANG_EQ_EQ`, precedência de igualdade) + `UnsupportedOperator` no início, len 1,
  arg = lexema (`fe:scanner/errors.dart:66`).

##### `const_primary_constructor_with_body` (perda 3), `extraneous_modifier_in_primary_constructor` (3), `expected_body` (3)
- 3.13.4 (fora da fonte 3.6.2): parte `this` de extension type com corpo; `covariant` em
  declarante de classe sem o recurso (relatado em `elaborar_construtor_primario` só com `var`?).
- `expected_body`: `switch(i) L: {` (`EXPECTED_SWITCH_STATEMENT_BODY` no `)`) e
  `test_runner/impl/*part*.dart` (`class X` sem `{`, part — arquivo de part não analisado como
  biblioteca? conferir o grupo).

##### `missing_function_body` (perda 3)
- `augment Object? foo();` (FN 4:13): depende de `?` solto + função `foo()` no topo (já ok no v4?) —
  conferir; `MissingRequiredParam__constructor_field_ce0ca2ef.dart:7:1` (corpo ausente no EOF).

##### Cauda (1–2 casos cada; origem → mudança)
- `annotation_with_type_arguments_uninstantiated` (2): `@A<int>` sem `(` → `an:fasta/ast_builder.dart`
  (`handleAnnotation`…) no `<…>`; ler type args de metadata sem argumentos e relatar.
- `empty_record_type_named_fields_list` (2): `({})` em tipo record → `fe:parser_impl.dart:1745`, no `{`.
- `record_type_one_positional_no_trailing_comma` (2): `(int)` em posição de tipo record →
  `fe:parser_impl.dart:1680`, no `)`.
- `expected_executable` (2), `expected_type_name` (2: `C<>` → no `>`; `typedef` augment), 
  `missing_function_parameters` (2), `missing_method_parameters` (FP 2: 3.13 `new`).
- `missing_digit` (2), `missing_hex_digit` (1), `illegal_character` (pos 1: UTF-16 × byte?),
  `invalid_unicode_escape_started` (1: `'…\` + quebra) — §SCANNER.
- `break_outside_of_loop` / `continue_outside_of_loop` / `continue_without_label_in_case` (1 cada):
  `fe:parser_impl.dart:9119/9213/9219` — exigem o `LoopState` do fasta (dentro de laço / switch);
  acrescentar ao parser um estado de laço.
- `external_constructor_with_field_initializers` (1), `external_factory_redirection` (1:
  `parser_impl.dart:5148`, no `=`), `invalid_super_in_initializer` (1), `named_function_expression`
  (1: `:5288`, no nome), `static_constructor` (1), `constructor_with_return_type` (1).

---

#### §SCANNER — recuperação do scanner do fasta (código JÁ escrito, compilação pendente)

- `crates/frontend/src/lexer.rs`: `lex_recuperando(source) -> (Vec<Token>, Vec<Diagnostic>)` com modo
  `recuperar` — caractere ilegal pulado (`ILLEGAL_CHARACTER`); string sem fecho termina na quebra
  de linha/EOF com `quote = 0` (o texto todo é conteúdo, como o `appendSyntheticSubstringToken`
  de `abstract_scanner.dart:2063`), erro em `endOffset - 1`; comentário sem fecho até o fim; `0x`
  sem dígito → `Int` + `MISSING_HEX_DIGIT` no `x` (`:1332`); expoente sem dígito → `Double` + `MISSING_DIGIT`
  no último caractere (`:1431`); `===`/`!==` → `EqEq`/`BangEq` de 3 caracteres + `UNSUPPORTED_OPERATOR`.
  `lex()` continua igual para quem o usa (para no 1º erro; agora também em `0x`, `1e`, `===`).
- `parser/mod.rs parse_lexed_com`: com `Err` do léxico, relê com `lex_recuperando` e analisa
  normalmente, pondo os erros léxicos na lista (sem mudar API; `elements/load.rs` herda).
- Validação pendente: comparação HEAD × novo nos 31.679 arquivos (SDK lib + pub cache) — código
  sem erro léxico não muda de tokens (o caminho `Ok` é o mesmo).

#### §FOR — `for (var|final <padrão> = e; c; u)` (ESPECIFICADO, AST não tocada, aguardando ok)
- Regra: `fe:parser/parser_impl.dart:8335 parseForStatement`, :8341-8352 (`patternKeyword` +
  `=` → `handleForInitializerPatternVariableAssignment`), `parseForLoopPartsStart` :8373;
  `an:fasta/ast_builder.dart` `handleForInitializerPatternVariableAssignment` → `ForPartsWithPatternImpl`.
- AST mínima: `ForInit::Pattern { final_: bool, pattern: PatternId, value: ExprId }`
  (`crates/frontend/src/ast.rs:725`).
- Impacto (matches que deixam de compilar): analise/locais.rs:456; types/inferencia/instrucoes.rs:694,
  1111, 1189; types/constantes/verificador.rs:449; mundo/impacto.rs:486; emit_js/body.rs:1291,
  expr.rs:1170; emit_native/lower/comandos.rs:275, captura.rs:324 e 529, literais.rs:512. `if let`
  só de `Variables` (compilam): analise/duplicatas.rs:345, types/constantes/avaliador.rs:1603/2373,
  emit_js/expr.rs:1003/1456, emit_native/comandos.rs:1020/1058, literais.rs:547, lsp/navegacao.rs:188.
  Detalhes enviados ao coordenador por mensagem.

## F. Nomes e tipos não resolvidos, não usados

### Especificação — família F (nomes/tipos não resolvidos e não usados)

Autor: r3-f. Fonte: `E:\references\dart-sdk-3.6.2\pkg\analyzer\lib` (citado como `analyzer/lib/...:linha`), `..\_fe_analyzer_shared\lib`, `messages.yaml`; 3.13 só onde dito. Amostras: `E:\dftemp\r3-f\amostras-<codigo>.txt` (placar base r3). Uma seção por código, na ordem de perda de `familia-F.txt`.

**Estado do código:** antes da mudança de método, r3-f já tinha implementado (compilando, build release ok, sem placar) a parte de `undefined_identifier`/`undefined_prefixed_name`/`undefined_identifier_await`/`creation_with_non_type` descrita em "Mudança (feita)" nas seções — arquivos `crates/types/src/scope.rs` (`deve_ignorar_indefinido`), `crates/types/src/inferencia/expr.rs` e duas edições atômicas em `crates/types/src/inferencia/chamadas.rs` (`chamada`, `criacao_sem_classe`). Ainda não validado no placar.

##### `undefined_identifier` (perda 169: FN 73, FP 95, msg 1, pos 0)

Fonte: `analyzer/lib/...` do `E:\references\dart-sdk-3.6.2\pkg\analyzer` (idêntico ao analyzer 6.11 do pub-cache nos arquivos citados, salvo `element.dart`, cujas linhas citadas conferem).

- **Emissão** (todas na resolução, `ResolverVisitor` e auxiliares; nenhuma no `ErrorVerifier`):
  1. `SimpleIdentifierResolver._resolve1` — `analyzer/lib/src/dart/resolver/simple_identifier_resolver.dart:209-224`: leitura de um identificador simples (`visitSimpleIdentifier` → `resolve`).
  2. `ResolverVisitor.resolveForWrite`, ramo `SimpleIdentifierImpl` — `analyzer/lib/src/generated/resolver.dart:1487-1503`: lado esquerdo de atribuição composta, `++x`, `x++` (`hasRead: true`) quando a leitura não tem elemento.
  3. `AssignmentVerifier.verify` — `analyzer/lib/src/error/assignment_verifier.dart:97-112`: escrita (`x = v`, composta, `++`/`--`) sem setter nem recuperação, chamado de `PropertyElementResolver.resolveSimpleIdentifier` (`property_element_resolver.dart:276-288`).
  4. `ResolutionVisitor.visitAssignedVariablePattern` — `analyzer/lib/src/dart/resolver/resolution_visitor.dart:188-207`: `(x) = 0` / `[x] = …` com `x` sem getter no escopo.
  5. `FunctionReferenceResolver._resolvePrefixedIdentifierFunction` — `function_reference_resolver.dart:414-426` (argumento `'p.nome'`, no **prefixo**) e `_resolveSimpleIdentifierFunction` — `:664-680` (`f<int>` fora de classe/extensão).
- **Condição exata:**
  ```
  // leitura (1)
  if node.isSynthetic || node.inDeclarationContext() || parent é FieldFormalParameter/… : nada
  readLookup = LexicalLookup.resolveGetter(scopeLookupResult)        // lexical_lookup.dart:21-36
            ?? (thisType != null ? ThisLookup.lookupGetter(thisType) : null)   // this_lookup.dart:20-56
  element = readLookup?.requested
  if element == null:
     if name == 'await' && enclosingFunction != null: UNDEFINED_IDENTIFIER_AWAIT
     elif !shouldIgnoreUndefinedIdentifier(node): UNDEFINED_IDENTIFIER [name]
  ```
  - `resolveGetter`: getter no escopo → `requested`; **só setter não de instância** (de topo, estático, importado, de extensão) → `recovery` (o resultado não é nulo, então **não** há busca pelo `this`, e a leitura fica sem elemento → `UNDEFINED_IDENTIFIER`); só setter de instância → `null` (segue pelo `this`).
  - O escopo léxico de classe/mixin/enum/extensão tem só os membros **declarados** (estáticos e de instância); herdados vêm só pelo `this`.
  - `thisType` (`_setupThisType`, `resolver.dart:4180-4193`) vale em **todo** membro de classe/extensão — método estático, fábrica, inicializador de construtor, inicializador de campo estático ou de instância —, não só onde `this` é permitido. `ThisLookup` chama `TypePropertyResolver.resolve` (`type_property_resolver.dart:62-232`): interface (`inheritance.getMember`), depois extensões aplicáveis, e, sem membro de instância, **recuperação estática** `lookupStaticGetter ?? lookupStaticMethod` na classe, nos mixins (último primeiro) e na cadeia de superclasses, só acessíveis (`type_property_resolver.dart:251-281`, `element.dart:5383-5408`). Achado por aí, não há `UNDEFINED_IDENTIFIER`: o erro passa a ser `INSTANCE_MEMBER_ACCESS_FROM_STATIC`/`_FROM_FACTORY`/`IMPLICIT_THIS_REFERENCE_IN_INITIALIZER` (`error_verifier.dart:3976-4038`) ou `UNQUALIFIED_REFERENCE_TO_NON_LOCAL_STATIC_MEMBER`/`…_OF_EXTENDED_TYPE` (`error_verifier.dart:5660-5700`, não para tear-off de método: `if (element is MethodElement) return`).
  - Receptor `this` anulável (extensão `on T?`/`on T`): outro caminho (erros de nulo), sem `UNDEFINED_IDENTIFIER` garantido.
  - `shouldIgnoreUndefined` (`analyzer/lib/src/dart/element/element.dart:1186-1221`): para cada import da unidade e das que a envolvem, com o mesmo prefixo (aqui: nenhum) e biblioteca importada sintética (alvo inexistente): com prefixo e sem `show` → ignora; com `show` que cita o nome → ignora. Sem prefixo e nome `_$…`: parte gerada (`file_paths.isGenerated`) que não existe → ignora.
  - (2) e (3) **não** consultam `shouldIgnoreUndefined`. (3): `requested == null` e `recovery` não é tipo/função/método/prefixo/getter/`MultiplyDefined` → `UNDEFINED_IDENTIFIER` (`receiverType == null` no identificador simples). Escrita: `LexicalLookup.resolveSetter` (`lexical_lookup.dart:38-55`) `?? ThisLookup.lookupSetter` (setter de instância, de extensão, ou recuperação estática `lookupStaticSetter`; senão `recovery` = o getter achado pelo `this`).
  - Invocação `nome(args)` sem alvo **não** dá `UNDEFINED_IDENTIFIER` (é `MethodInvocationResolver._resolveReceiverNull`, `method_invocation_resolver.dart:559-664`): sem getter léxico e sem `thisType` → `UNDEFINED_FUNCTION` (com `shouldIgnoreUndefined`); com `thisType` e só um setter léxico de topo/extensão/estático → `UNDEFINED_METHOD [nome, nome da classe]`; senão `_resolveReceiverType` no `thisType`: estático herdado → `UNQUALIFIED_REFERENCE_TO_…` (`:208-227`), nada → `UNDEFINED_METHOD [nome, receiverClassName]` (`:866-880`; nome do elemento sem argumentos de tipo, `'Function'`, ou `'<unknown>'`). `p.nome(args)` com `p` prefixo → `_resolveReceiverPrefix` (`:666-722`): `loadLibrary` de import adiado único resolve; senão getter do escopo do prefixo; nada → `UNDEFINED_FUNCTION [nome]` (com `shouldIgnoreUndefined(prefix: p)`).
  - `p.nome` (leitura/escrita) com `p` prefixo → `UNDEFINED_PREFIXED_NAME`, não `UNDEFINED_IDENTIFIER` (ver seção própria).
  - `a<T>.b(…)` sem `new`, `a` indefinido → `InstanceCreationExpression` (`ast_rewrite.dart:38-115` não reescreve) → `CREATION_WITH_NON_TYPE 'a'`; `x.a<T>.b(…)` com `x` que não é prefixo → `MethodInvocation` de `FunctionReference` → (5) `UNDEFINED_IDENTIFIER ['x.a']` no `x`.
- **Posição:** o token do identificador (no caso 5 prefixado, o identificador do prefixo, com o argumento `'prefixo.nome'`).
- **Mensagem:** `Undefined name '{0}'.` (`messages.yaml:17396`), `{0}` = o nome escrito (`node.name`).
- **Supressões e ordem:** identificador sintético (recuperação do parser) não é relatado; nome em contexto de declaração; o mesmo nó não é relatado duas vezes (leitura e escrita de `x += 1` dão o mesmo diagnóstico, que o `ErrorReporter` não deduplica — mas o oráculo só tem um: (2) e (3) só ocorrem juntos quando os dois falham e o resultado é igual). Num arquivo com erro de sintaxe, a árvore recuperada do fasta é resolvida normalmente (o código sai), mas a nossa recuperação difere — daí o filtro da fase 6.
- **No DartForge:**
  - Hoje: `crates/types/src/inferencia/expr.rs` `identificador` (braços `ThisImplicito`/`Ausente` e `Nenhum`), `tipo_de_escrita_nome` (escrita), `propriedade` (`p.nome` com prefixo) e a leitura do alvo de toda invocação `nome(args)` (`chamadas.rs::chamada`, braço `_`, que infere o alvo como identificador). `resolver_nome` (busca léxica) não distingue "só setter" de getter, não faz busca pelo `this` em contexto estático (devolve `Nenhum` se `cx.tipo_this` é `None`, e `Corpo` só põe `tipo_this` onde `this` vale), não tem recuperação estática, não conhece `loadLibrary` nem `shouldIgnoreUndefined`. A paridade (`crates/paridade/src/analise.rs` fase 6) descarta o código em arquivo com erro de recuperação do parser.
  - Causas dos **95 FP** (placar base): invocação sem alvo relatada como nome indefinido em vez de `UNDEFINED_FUNCTION`/`UNDEFINED_METHOD` (~30: `sdk_version_since` A(…)/foo(…), `regress11724`, `regress17382`, `lib/a.dart h()`, `top_level/unresolved_method`, `UnknownType()`, `abs()`/`bOnly()`/`X()` em classe → `UNDEFINED_METHOD`); prefixo de import inexistente (`shouldIgnoreUndefined`, 12: `creation_with_non_type/…implicit_*p*`, `sdk_version_since/…prefixed`); `p.nome` sem o nome → `UNDEFINED_PREFIXED_NAME`/`UNDEFINED_FUNCTION` (12: `const/instantiated_function_constant_error`, `if_null/assignment_behavior`, `import/self`, `prefix/transitive_import`, `private/access`, `regress27572`); `loadLibrary` de import adiado (5); membro achado pelo `this` do analyzer em contexto estático/inicializador (6: `instance_member_access_from_static`, `factory/and_instance_variable`, `implicit_this_reference_in_initializer`) e estático herdado (16: `unqualified_reference_to_*`); `await` num corpo síncrono → `UNDEFINED_IDENTIFIER_AWAIT` (1); padrão relacional `== b && var b` → `referenced_before_declaration` (4, `patterns/pattern_variable_constant_scope`); `A<B>.foo()` → `creation_with_non_type` (2: `regress34495`, `…class_constructor_name`); lexer (`3457e`, `0x`, 3: o analyzer não produz o identificador `e`/`x`); sintaxe 3.13 de construtor primário (5, bibliotecas julgadas pelo 3.13).
  - Causas dos **73 FN**: leitura de nome que só tem setter (de topo, ou de instância no contêiner sem getter herdado) — 30 (`static_extension_internal_basename_shadowing_error` 24, `static_extension_internal_resolution_4` 3, `compoundAssignment`/`postfix`/`prefix` com `set foo`, `inheritedGetter_sh…`, `setter/no_getter`); argumentos de anotação em parâmetro de tipo não inferidos (15: `UndefinedIdentifier__annotation_*`, `metadata/type_parameter_scope_inner`); padrões — constante `case foo`, `case unresolved:`, atribuição por padrão `(x) = 0` (5); membro privado de outra biblioteca achado pelo `this` (`private/member1`, 1: `buscar_membro` não respeita a privacidade); `dart:core` importado com `show` ainda dá o `String` implícito (`importCore_withShow`, 1 — escopo do `elements`); arquivos com erro de sintaxe filtrados pela fase 6 (≈17: `augmentation_formal_parameter`, `class/keyword`, `label8`, `variance`, `invalid_const_pattern`, `yield 0`, `$$x`, `never..(_)`, `mixin_class_syntax_error`); `@Annotation(Bar)` com classe sombreada (1).
  - Mudança (já parcialmente no código desta rodada, compilando — ver relatório): `expr.rs` ganhou `tipo_this_do_analyzer`, `busca_lexica_de_leitura` (getter / só setter de instância / só setter solto / nada), `buscar_pelo_this` (+ `estatico_na_cadeia`, recuperação estática), `nome_lido_indefinido`, `nome_escrito_indefinido`, `invocacao_sem_alvo_indefinida`, `load_library`, `avisar_nome_prefixado_indefinido`, `erro_de_instancia_sem_this`; `crates/types/src/scope.rs::deve_ignorar_indefinido` (porta de `shouldIgnoreUndefined`); `chamadas.rs::chamada` intercepta `nome(args)` e `p.nome(args)` antes de inferir o alvo. Falta: anotações de parâmetro de tipo, padrões (`visitAssignedVariablePattern`, constante), privacidade na busca pelo `this`, o `show` de `dart:core`, `x.a<T>` (5) e o relacional `== b && var b`.

##### `undefined_class` (perda 124: FN 116, FP 8, msg 0, pos 0)

- **Emissão:** `NamedTypeResolver.resolve` (`analyzer/lib/src/dart/resolver/named_type_resolver.dart:84-131`), chamado por `ResolutionVisitor.visitNamedType` (`analyzer/lib/src/dart/resolver/resolution_visitor.dart:1096-1105`; argumentos de tipo visitados **antes** do nome, `:1097`), na passada `ResolutionVisitor` de `LibraryAnalyzer._resolveFile` (`analyzer/lib/src/dart/analysis/library_analyzer.dart:811-863`) sobre a AST recuperada pelo fasta. Sai em `_resolveToElement` (`named_type_resolver.dart:304-322`) → `_ErrorHelper.reportNullOrNonTypeElement` (`:518-651`).
- **Condição exata:**
  ```
  resolve(node):
    importPrefix != null:
      pe = nameScope.lookup(prefixo).getter   // escopo léxico completo
      pe == null → _resolveToElement(node, null)
      pe Interface|TypeAlias → _rewriteToConstructorName (NOT_A_TYPE 'P.N' fora de ConstructorName)
      pe Prefix → _resolveToElement(node, pe.scope.lookup(name2).getter)
      senão → PREFIX_SHADOWED_BY_LOCAL_DECLARATION
    senão: 'void' → void; senão _resolveToElement(node, nameScope.lookup(name2).getter)
  _resolveToElement(node, null): if !shouldIgnoreUndefinedNamedType(node) → reportNullOrNonTypeElement(node, null)
  reportNullOrNonTypeElement(node, el) (:518-650), em ordem:
    sintético → nada; 'boolean' → UNDEFINED_CLASS_BOOLEAN; catch → NON_TYPE_IN_CATCH_CLAUSE;
    as → CAST_TO_NON_TYPE; is → TYPE_TEST_WITH_NON_TYPE / _UNDEFINED_NAME; redirecionamento → REDIRECT_TO_NON_CLASS;
    TypeArgumentList → NON_TYPE_AS_TYPE_ARGUMENT; criação → CONST/NEW_WITH_NON_TYPE;
    Extends/Implements/With/ClassTypeAlias → nada (MixinOnClause NÃO está na lista);
    local/função local → REFERENCED_BEFORE_DECLARATION; el != null → NOT_A_TYPE;
    sem prefixo e 'await' → UNDEFINED_IDENTIFIER_AWAIT; senão UNDEFINED_CLASS
  ```
  O `dart:core` implícito só entra se a unidade definidora não importa `dart:core` explicitamente (`analyzer/lib/src/dart/analysis/file_state.dart:289`, `:1050-1052`). `dynamic`/`Never` são getters de `dart:core` (`analyzer/lib/src/dart/element/scope.dart:288-292`): com `import 'dart:core' as core;`, `dynamic` e `int` sem prefixo ficam indefinidos.
- **Posição:** `_getErrorRange(node)` (`named_type_resolver.dart:653-666`): do prefixo, se houver, ao fim de `name2` (sem argumentos nem `?`).
- **Mensagem:** `Undefined class '{0}'.` (`analyzer/messages.yaml:16701`), `{0}` = `name2.lexeme`.
- **Supressões e ordem:** `shouldIgnoreUndefined` (`analyzer/lib/src/dart/element/element.dart:1186-1221`: import sintético de mesmo prefixo sem `show`; `show` que cita o nome; `_$…` com parte gerada inexistente); nome sintético (`:519`); `MultiplyDefinedElement` (`:316-319`); cláusulas de herança. Recuperação do fasta: `augment`/`base`/`sealed` diante de `mixin|extension|…` sem o experimento viram o **tipo** de uma variável de topo de nome sintético (`TopLevelDeclarationIdentifierContext.ensureIdentifier`, `_fe_analyzer_shared/lib/src/parser/identifier_context_impl.dart:1056-1094`, `looksLikeStartOfNextTopLevelDeclaration` `:1330-1332`; `parseFields`, `_fe_analyzer_shared/lib/src/parser/parser_impl.dart:3634-3652`); `augment class` não (`computeType` devolve `noType` diante da palavra reservada, `parser_impl.dart:3521-3550`; `augment` vira o **nome**, MISSING_CONST_FINAL_VAR_OR_TYPE).
- **No DartForge:** outline em `crates/types/src/resolve.rs` (`resolve_annotation`, ~1390), corpos em `crates/types/src/inferencia/tipos.rs` (`resolver_anotacao`, ~339), cláusulas em `crates/analise/src/clausulas.rs`, filtro `nomes_de_clausulas` em `crates/paridade/src/analise.rs:577`. Causas:
  - **Parser descarta `augment mixin/extension/extension type …` (FN 112)** — ex.: `AugmentationModifierExtra__mixin_nothin_8b7e1a8b.dart`, `AugmentationOfDifferentDeclarationKind__0fc33736.dart`. Em `crates/frontend/src/parser/declarations.rs::parse_function_or_variables` (~2066-2095) o ramo do contexto de topo relata MISSING_IDENTIFIER e devolve `Err(";")`, e o `NamedType` some (`augment base mixin` já funciona: `base` vira o nome). O parser (outro dono) precisaria devolver `Variables{ ty: Some(tipo), variables: [nome sintético vazio] }` sem consumir a palavra-chave; o nome vazio precisa ser ignorado por `duplicate_definition`, `unused_*` e pelos elementos; os filtros "span vazio = sintético" de `resolve.rs` já evitam relatar o nome. Sem o parser não há nó (reconstruir pelos tokens é frágil: não recomendado).
  - **`dart:core` implícito sempre e `dynamic` embutido (FN 2)** — `UndefinedClass__dynamic_coreWithPrefix.dart`, `AugmentationReturnTypeMismatch__topLeve_549a3325.dart`. `crates/elements/src/outline.rs:460-468` injeta o core sempre; `resolve.rs` (passo 2) e `tipos.rs` dão `dynamic`/`Never` sem escopo. Mudança: não injetar com import explícito de `dart:core` (corrige também `show`/`hide`, cf. `UndefinedIdentifier__importCore_withShow.dart`); `dynamic`/`Never` embutidos só com core sem prefixo.
  - **Parâmetro sem `)` derruba a função (FN 1)** — `variance/syntax/variance_type_parameter_error_syntax_test.dart`; `parse_parameter_list` (`crates/frontend/src/parser/types.rs:620-673`) usa `expect_op(RParen)?`; o fasta usa `ensureCloseParen`. Trocar por `garantir_fecha_parenteses` (`parser/mod.rs:574-587`) — parser.
  - **Identificador embutido como tipo (FN 1)** — `UndefinedClass__builtInIdentifier.dart` (`import<int> x = [];`): o fasta faz membro de topo (`parser_impl.dart:723-736`) com BUILT_IN_IDENTIFIER_AS_TYPE (`identifier_context_impl.dart:1219-1222`) — parser.
  - **`A.foo` com `A` classe (FP 2 → FN de `not_a_type`)** — `NotAType__class_{constructor,method}.dart`; o ramo prefixado de `resolve_annotation` (~1584-1667) cai em UNDEFINED_CLASS 'foo'. Ver `not_a_type`.
  - **`shouldIgnoreUndefined` ausente no outline (FP 5)** — `UndefinedClass__ignore_libraryImport_prefix.dart`, `…_show_it.dart`, `…_part_notExist_ur_*`, `SdkVersionSince__class_typeAnnotation_prefixed.dart`. `crate::scope::deve_ignorar_indefinido` existe (desta rodada); chamar nos dois ramos `None` de `resolve_annotation` e de `tipos.rs::resolver_anotacao`.
  - **Argumentos de tipo de constante de enum sem os parâmetros do enum (FP 1)** — `TypeParameterReferencedByStatic__enum_c_742d0786.dart` (`v<T>()`); `resolve.rs:~703` usa `&HashMap::new()`/`Normal`; o analyzer usa o escopo do enum (`resolution_visitor.dart:496-543`). Passar o escopo de parâmetros de tipo, o contêiner e `ArgumentoDeTipo` (sem `membro_estatico`).
  - Lacuna (sem amostra): `resolve_annotation` não resolve os argumentos de tipo quando a cabeça é parâmetro de tipo ou não resolve (ver `non_type_as_type_argument`).

##### `unused_element` (perda 74: FN 74, FP 0, msg 0, pos 0)

Citações `analyzer/lib/...` do 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\analyzer\lib`), salvo as marcadas **[3.13]** (`E:\references\dart-sdk\pkg\analyzer\lib`). `unused_field_from_primary_constructor` e `unused_element_parameter` (como nome próprio) só existem no 3.13; no 3.6.2, `UNUSED_ELEMENT_PARAMETER` tem `sharedName: UNUSED_ELEMENT` e sai como `unused_element`. Os 166 FN das amostras foram classificados um a um; nenhum FP/msg/pos.

- **Emissão:**
  - Coleta: `GatherUsedLocalElementsVisitor` (`analyzer/lib/src/error/unused_local_elements_verifier.dart:25-486`) em todas as unidades; conjuntos juntados por `UsedLocalElements.merge` (`analyzer/lib/src/dart/analysis/library_analyzer.dart:316-324`).
  - Verificação: `UnusedLocalElementsVerifier` por arquivo em `_computeWarnings` (`library_analyzer.dart:459`, chamada em `:513-520`).
  - Relatam: `_visitClassElement` (`:1008`), `_visitConstructorElement` (`:1015`), `_visitFunctionElement` (`:1034`), `_visitMethodElement` (`:1059`), `_visitPropertyAccessorElement` (`:1066`), `_visitTopLevelVariableElement` (`:1073`), `_visitTypeAliasElement` (`:1080`); forma de parâmetro em `visitFormalParameterList` (`:602-610`).
  - Portão: só `_analysisOptions.warning` (`library_analyzer.dart:315`). **Não há portão por erro de sintaxe.**
- **Condição exata:**
  ```
  report(E) se !used(E):
  classe/mixin/enum/ext type/typedef/var de topo/função de topo ou local
     → _isUsedElement (:860): isSynthetic→usado; público (exceto locais)→usado;
       @pragma('vm:entry-point')→usado; senão usado ⇔ E ∈ elements
  função local: com wildcard_variables, "_" não é relatado (:1036-1038);
     SEM o recurso (padrão no 3.6), "_" e "__" SÃO relatados
  método/getter/setter/construtor → _isUsedMember (:926): _isPubliclyAccessible→usado;
       synthetic; pragma; E ∈ members ∪ elements; ou sobrescreve membro usado
       (_overridesUsedElement, :964, recursivo via getOverridden2)
  _isPubliclyAccessible (:799): privado→não; ctor generativo de enum→não;
       classe privada e (static ou ctor)→não; em extensão → extension.isPublic,
       e extensão SEM NOME é privada (element.dart:2765-2771)
  construtor: só nomeado (node.name != null, :535) e só se a classe tem
       constructors.length > 1 (:1020); em extension type o primário conta
  ```
  O que entra em `elements`/`members`:
  - `visitSimpleIdentifier` (`:255-328`): ignora declarações e comentários; liga pelo **elemento resolvido** (`readElement`, `writeElement`, `staticElement`); ignora referências ao próprio `_enclosingClass` (só `ClassDeclaration`, `:75-90`) e ao próprio `_enclosingExec` (a `FunctionDeclaration`/`MethodDeclaration` mais interna, funções locais sim, closures não: `:136-144`, `:201-209`, `:383`) — recursão não é uso.
  - Getter e setter ligados pelo contexto: `x = e` só setter; leitura só getter; `x += e` os dois.
  - `NamedType` (`:221`, `:388-401`): `InterfaceElement` **não** conta no tipo de `VariableDeclarationList` cujo pai não é `FieldDeclaration` (local, de topo, de `for`), nem no tipo de `is` (`:189-198`). Tipo de campo conta.
  - Operadores: `BinaryExpression`/`PrefixExpression`/`PostfixExpression` por `staticElement`; `IndexExpression` por `writeOrReadElement` (`:176`) — em `a[i] += 1`/`??=` só `[]=`. `AssignmentExpression` faz `members.add(staticElement)` sem desembrulhar `ExecutableMember` (`:41-47`): operador composto de extensão/classe genérica não conta.
  - `FunctionExpressionInvocation.staticElement` (`:155`): `call` implícito.
  - Construtores: enum constant (`:124`), `typedef` público usa os construtores públicos (`:161-173`).
- **Posição:** `element.nameOffset`/`nameLength` (`:993-1006`): construtor → o nome depois do ponto; operador → o token do operador; função local `_` → o `_`.
- **Mensagem:** `messages.yaml:27333` "The declaration '{0}' isn't referenced." `{0}` = `displayName` (`E.named`, `[]=`, `_s` sem `=`). Forma de parâmetro `messages.yaml:27393` "A value for optional parameter '{0}' isn't ever given." (código `unused_element`).
- **Supressões e ordem:** só `// ignore`. Uso medido na biblioteca inteira, relato por arquivo; sem duplicata; erro de sintaxe não suprime.
- **No DartForge:** membros/topo em `crates/analise/src/privados.rs::nao_usados` (chamado de `crates/paridade/src/analise.rs:354`, ligação **por nome de token**); funções locais em `crates/analise/src/locais.rs::nao_usados_sem_filtro` (`Especie::Funcao`, `:875`); parâmetros em `crates/types/src/parametros.rs`. Causas dos 74 FN:
  1. **Funções locais `_`/`__` filtradas (14)** — `locais.rs:869` aplica `so_sublinhados` também a `Funcao` (ex.: `UnusedElement__localFunction_underscores.dart`, `DuplicateDefinition__block_localFunctio_93d08194.dart`). Mudança: pular só `curinga && nome == "_"`.
  2. **Portão de sintaxe na biblioteca (10)** — `privados.rs:223` e `analise.rs:348-354`. Erros: `constructor_with_return_type` (`linguagem/enum/enhanced_enums_error_test.dart` 481–497, 5), `const_constructor_with_body` (`ConstructorBody__enum_secondaryConstruc_815c9994.dart`), `missing_const_final_var_or_type` do `augment` (`UnusedElement__parameter_isUsed_overrid_387a0899.dart`, 4). Mudança: esses erros na lista "sem descarte" de `recuperacao_do_parser` (`analise.rs:726`); portão por declaração (só o candidato cuja declaração contém erro de recuperação); em `privados.rs:253` ignorar `decl.augment` sem o recurso.
  3. **Membros de extensão sem nome tratados como públicos (11)** — `privados.rs:330` `None => (String::new(), false)`; o analyzer a trata como privada (ex.: `super_in_extension/...2100d7fc.dart`, `UnusedElement__method_notUsed_unnamedExtension.dart`, `ffi_address_of_cast/...no_error.dart`, `issue_45551_error_test.dart`, `nullable_expression_call_error_test.dart` 46/91). Mudança: `None => (String::new(), true)` (operador/`call` dependem da causa 4).
  4. **Operadores e `call` de extensão privada nunca candidatos (7)** — `privados.rs:162` (`sem_nome`) os exclui (`UnusedElement__method_notUsed_privateEx_*`, `…isUsed_privateExt_{75655fd4,d8f24a17}`: só `false[3] ??= 1`/`+= 1`, e `[]` não é marcado). Mudança: decidir pelo uso resolvido (`BodyTypes`, como `parametros.rs`) com as regras de índice/composto/`call`; parcial seguro: relatar só se a biblioteca não tem nenhuma expressão com o operador.
  5. **`NamedType` em tipo de variável local/de topo ou `is` contando como uso (9)** — `UnusedElement__class_isUsed_variableDeclaration.dart`, `…isUsed_native.dart`, `…notUsed_isExpression*`, `…extensionType_isUsed_var_*`. Mudança: tokens dentro desses tipos não contam para classe/mixin/enum/extension type (typedef sim).
  6. **Autorreferência (6)** — `UnusedElement__getter_notUsed_reference_2e4123d6.dart`, `…method_notUsed_reference_2c22e70c`, `…setter_notUsed_reference_5abca3fa`, `…topLevelFunction_notUsed_4c91bd9e`; local recursiva `…functionLocal_notUsed_re_2a750abc`, `nnbd/static_errors/local_function_inference_test.dart:19`. Mudança: token não conta para X se a declaração nomeada mais interna que o contém é X; em `locais.rs`, pilha de função local corrente em `referir` (`:114`).
  7. **Construtor primário fora da contagem (6)** — extension type (3: `UnusedElement__extensionTypePrivate_pub_d20244f3.dart`, `…extensionType_privateCon*`): contar `construtores + 1`; enum com primário 3.13 (3: `non_redirecting_generative_constructor_with_primary/...`, `primary_constructors/header/constructor_name_error_test.dart:42`): tirar o `continue` de `privados.rs:297`.
  8. **Getter/setter não separados pelo contexto (2)** — `UnusedElement__getter_notUsed_invocatio_7d9034d5.dart` 3 e 13.
  9. **Ligação por nome conta outro elemento (5)** — parâmetro de tipo `_` (`conflicting_type_variable_and_member/...`), construtor sintético de mixin (`UnusedElement__constructor_isUsed_mixin_32fb43cd.dart`), mesmo nome em outra classe (`…method_notUsed_hasSameNameAsUsed.dart`), `p._x` por prefixo (`import/self_test.dart:16`). Mudança: parâmetros de tipo em `declaracoes`; construtor só por token qualificado pela própria classe/`this.n`/`super.n`/`= C.n`; descartar tokens depois de `prefixo.`; o caso de outra classe exige resolução.
  10. **`super.e` posicional obrigatório marca o do super (1)** — `UnusedElement__classPrivate_secondaryCo_2f78d312.dart`; só `DefaultFormalParameter` marca (`:114-121`). Mudança: `parametros.rs:291` só com `p.kind != Required`.
  11. **Artefato 3.13 `'<unnamed>'` em 1:1 (2)** — `augmentation_modifier_{extra,missing}/...`: FN conhecido.

##### `unused_local_variable` (perda 43: FN 43, FP 0, msg 0, pos 0)

- **Emissão:** `UnusedLocalElementsVerifier._visitLocalVariableElement` (`analyzer/lib/src/error/unused_local_elements_verifier.dart:1045-1057`), de `visitVariableDeclarationStatement` (`:738-746`), `visitForPartsWithDeclarations` (`:613`), `visitDeclaredIdentifier` (`:544`, for-in), `visitCatchClauseParameter` (`:520`), `visitDeclaredVariablePattern` (`:550-564`, sem `_patternVariableElements` ativo) e `visitPatternVariableDeclaration` (`:672-699`, agrupa). Caminho: `LibraryAnalyzer._computeDiagnostics` (`analyzer/lib/src/dart/analysis/library_analyzer.dart:315-330`) roda `GatherUsedLocalElementsVisitor` em todas as unidades (`UsedLocalElements.merge`), e `_computeWarnings` (`:459`, `:512-520`) roda o verificador por arquivo.
- **Condição exata:**
  ```
  relata(e) = !_isUsedElement(e) && !_isNamedWildcard(e)
  // UNUSED_CATCH_CLAUSE / UNUSED_CATCH_STACK no catch, senão UNUSED_LOCAL_VARIABLE
  _isUsedElement(local) = e.isSynthetic || usedElements.elements.contains(e)   // :860-923
  _isNamedWildcard(e)   = nome só de '_' && !(len>1 && wildcard_variables)      // :775-791
  ```
  Uso (`visitSimpleIdentifier`, `:255-282`): fora de declaração e de `CommentReference`; elemento `LocalVariableElement` e `_isReadIdentifier(node)`; `JoinPatternVariableElement` adiciona as `transitiveVariables` (`:1145-1153`). `_isReadIdentifier` (`:416-440`): só contexto de leitura (`x = e` não lê); se `parent.parent is ExpressionStatement` e o pai é **qualquer** `PrefixExpression`/`PostfixExpression` (`-x;`, `!x;`, `~x;`, `++x;`, `x++;`, `x!;`) não lê; composto com `x` à esquerda lê só com `??=`. Padrões: em `PatternVariableDeclaration` (inclusive `ForPartsWithPattern`) o grupo só se relata se nenhuma for lida; `DeclaredVariablePattern` fora disso (`case`, `if-case`, padrão de atribuição `[x, final y] = …`) um a um; `isDuplicate` não; guarda `when` lê a variável do próprio `case`, corpo compartilhado lê a join. `catch`: exceção usada se há pilha ou não há `on` (`:57-71`). Roda **com** erro de sintaxe.
- **Posição:** o nome declarado (`:993-1006`). **Mensagem:** "The value of the local variable '{0}' isn't used." (`displayName`).
- **Supressões:** só `// ignore` (`library_analyzer.dart:540`).
- **No DartForge:** `crates/analise/src/locais.rs::nao_usados` (`:812`), integrado em `crates/paridade/src/analise.rs` com os erros de `recuperacao_do_parser`. Causas:
  1. **Filtro de erro de sintaxe (25)** (`locais.rs:814-825`): `await/backwards_compatibility_test.dart` (3), `patterns/version_2_32_changes_error_test.dart` (8), `string/unicode1..4_test.dart` (4), `string/interpolate1/3_test.dart` (4), `map/literal13_test.dart`, `operator/unary_plus_test.dart`, `enum/is_keyword_test.dart`, `string/bad_raw_string_test.dart`, `patterns/invalid_const_pattern_test.dart`, `UnawaitedReturnInTryBlock__insideClosur_c0f7d494.dart`. Proposta segura: (a) erros de escape do scanner (`invalid_unicode_escape_u_no_bracket`, `invalid_unicode_escape_u_bracket`, `invalid_hex_escape`, `invalid_code_point`) fora de `recuperacao_do_parser`; (b) só reter se, depois da declaração e até o fim do executável, houver token com o texto do nome.
  2. **Prefixo/sufixo como comando (7)** (`locais.rs:161-170` só trata `++`/`--`): `UndefinedOperator__unaryMinus_null.dart`, `UncheckedUseOfNullableValue__operatorUn_*` (2), `implicit_downcast_during/not_test.dart`, `nnbd/definite_assignment/read_error_test.dart` (3). Proposta: todo `UnaryOp` como comando com operando `Identifier` direto não lê.
  3. **Variável declarada em padrão de atribuição (5)** (`padrao_de_atribuicao`, `:417-420`, trata `final y`/`int y`/`var y` como escrita): `patterns/declared_variable_in_pattern_assignment_error_test.dart` (4), `var_keyword_in_typed_variable_pattern_error_test.dart`. Proposta: declarar como variável sem grupo.
  4. **`for (var (p) = e; …)` (3)**: não existe `ForInit::Pattern` (`frontend/src/parser/statements.rs:78-87`): `UnusedLocalVariable__forPartsWithPattern_notUsed.dart`, `DeadCode__flowEnd_forParts_*`, `…forElementParts_*`. Proposta: o parser (outro dono) aceitar o padrão; `for_init` como `PatternVariables`.
  5. **Guarda em `case` compartilhado (2)**: `UnusedLocalVariable__switchStatement_sh_{8d5b3b84,b7ecf69a}.dart`. Proposta: join sintética como raiz; a guarda marca só a variável do caso.
  6. **Corpo não-bloco sem escopo (1)**: `while (false) var x = 0;` (`variable/ref_before_declaration_test.dart:109`). Proposta: escopo em volta do corpo de `while`/`do`/`else`/`Labeled` que não é `Block`.

##### `unused_import` (perda 42: FN 41, FP 1, msg 0, pos 0)

- **Emissão:** `ImportsVerifier.generateUnusedImportHints` (`analyzer/lib/src/error/imports_verifier.dart:238-274`), de `LibraryAnalyzer._computeWarnings` (`library_analyzer.dart:498-509`).
- **Condição exata:**
  ```
  para cada ImportDirective:
    tracking = importsTracking.map[prefixo]
    se tracking.hasPrefixUsedInCommentReference: pula   // `[p]` sozinho (comment_reference_resolver.dart:148-151)
    se uri.library.isDartCore ou isSynthetic: pula
    usado = tracking.importToUsedElements.containsKey(import); senão UNUSED_IMPORT
  ```
  Uso (`analyzer/lib/src/dart/element/scope.dart:119-255`): `PrefixScope.lookup` que devolve elemento marca **todos** os imports do mesmo prefixo cujo namespace (com `show`/`hide`) o fornece; `MultiplyDefinedElement` não marca; inclui o `dart:core` implícito (`Future` marca `dart:async`); o `PrefixScope` só é consultado se a busca léxica não achou antes; `p.X` marca pelo escopo de `p`, `p` sozinho não; `loadLibrary` não marca; extensão implícita: `ExtensionMemberResolver.findExtension` (`extension_member_resolver.dart:89-115`) chama `notifyExtensionUsed` só com escolha única/mais específica única (ambíguo não marca); combinadores não rastreiam. Import duplicado continua `unused_import`.
- **Posição:** `importDirective.uri`. **Mensagem:** "Unused import: '{0}'." (`relativeUriString`).
- **Supressões:** nada sai se `_hasDiagnosticReportedThatPreventsImportWarnings` (`library_analyzer.dart:588-613`) acha na biblioteca `AMBIGUOUS_IMPORT`, `CONST_WITH_NON_TYPE`, `EXTENDS_NON_CLASS`, `IMPLEMENTS_NON_CLASS`, `MIXIN_OF_NON_CLASS`, `NEW_WITH_NON_TYPE`, `NOT_A_TYPE`, `PREFIX_IDENTIFIER_NOT_FOLLOWED_BY_DOT`, `UNDEFINED_ANNOTATION`, `UNDEFINED_CLASS`, `UNDEFINED_FUNCTION`, `UNDEFINED_IDENTIFIER`, `UNDEFINED_PREFIXED_NAME`, `DEPRECATED_EXPORT_USE`. Erro de sintaxe não suprime.
- **No DartForge:** `crates/analise/src/importacoes.rs::nao_usados` (fase 5 de `paridade/src/analise.rs`), uso aproximado sintaticamente. Causas:
  1. **Import com extensão conta sempre como usado (35)** (`traz_extensao`, `importacoes.rs:155-158`; `dart:async` exporta `FutureExtensions`…, `dart:collection` `IterableExtensions`…): `invalid_returns/*` (12), `nonfunction_type_aliases/generic_usage_*` (5), `BuiltInIdentifierAsPrefixName__*` (6), `PrefixShadowedByLocalDeclaration__local_*` (3), `mixin_class_legacy_interactions_forward_error_test.dart`, `static_extension_prefix_import_conflict_test.dart` (2), `static_extension_internal_resolution_3_error_test.dart`, `null_aware_elements/type_inference_simple_error_test.dart`. Proposta: registrar a extensão escolhida (única) em `types::inferencia::membros::membro_de_extensao`, `membro_de_extensao_explicita` e `scope.rs::lookup_extension_member`, exposto em `BodyTypes` (incluindo as passadas de inicializadores); o import é usado por extensão só se fornece uma extensão usada.
  2. **Prefixo sombreado conta como uso (3)**: `prefix/shadow_test.dart` (2), `variable/ref_before_declaration_test.dart`. Proposta: contar `p.X` só com `Resolved::Prefix`; meta: gravar em `types` cada busca que atravessa o escopo de import.
  3. **`loadLibrary` (1)**: `deferred/load_library_wrong_args_test.dart` — o FP `undefined_identifier 'loadLibrary'` suprimia; resolver `p.loadLibrary` (feito na seção de `undefined_identifier`) e não contar como uso.
  4. **`dart:_wasm` não carregado (1)**: `ImportInternalLibrary__wasm_fromTest.dart`; carregar as bibliotecas internas que faltam.
  5. **FP nosso suprimindo (1)**: `primary_constructors/wildcard_declaring_parameters_error_test.dart` (`undefined_identifier '_'` falso).
  - **FP:** `static_extension_internal_resolution_4_error_test.dart:24` — o oráculo tem `undefined_identifier` (236, 242, 248) que nós não emitíamos (leitura de nome só com setter, corrigida na seção de `undefined_identifier`), então a porta não suprimia.

##### `unused_field_from_primary_constructor` (perda 39: FN 39, FP 0, msg 0, pos 0)

Só existe no 3.13 (o 3.6.2 não tem construtor primário); oráculo 3.13 (bibliotecas em `sintaxe-nova.json`).

- **Emissão [3.13]:** `UnusedLocalElementsVerifier.visitPrimaryConstructorDeclaration` (`analyzer/lib/src/error/unused_local_elements_verifier.dart:1022-1057`), em `_computeWarnings` (`library_analyzer.dart:572`), sem portão de sintaxe.
- **Condição [3.13]:**
  ```
  pai ClassDeclaration ou EnumDeclaration (extension type: nunca):
    para cada parâmetro: FieldFormalParameterElementImpl && isDeclaring ('final'/'var')
       && field != null && !_isReadMember(field) (:1192-1220) → relatar
  ```
  Na prática só privados; dois declarantes de mesmo nome são ambos relatados; `_` no inicializador de campo não lê (`wildcard_declaring_parameters_error_test.dart:33,45`).
- **Posição:** o nome do parâmetro. **Mensagem [3.13]:** `messages.yaml:31462` "The value of the field '#fieldName' isn't used." (correção com `#keyword` = `final`/`var`); WARNING.
- **No DartForge:** não existe; `privados.rs:174` exclui os campos `de_primario`, enums com primário pulam membros (`:297`). 39/39 não implementado (classe com `_` duplicado 3, enum 3, `private_optional_parameter/...class_primary_*` 5, `unused_field_from_primary_constructor/*` 5, `wildcard_declaring_parameters_error_test.dart` 9, `declaring_parameter_collision_error_test.dart` 14). Mudança: entrada em `SUPLEMENTO_3_13` (`crates/paridade/examples/gerar_codigos.rs:37`) com dois argumentos e regenerar `codigos_g.rs`; regra `CampoDePrimario` (classe e enum) com o critério de `Campo`, `[nome, final|var]`, só em biblioteca de sintaxe nova; depende da causa 1 de `unused_field`.

##### `unused_field` (perda 27: FN 27, FP 0, msg 0, pos 0)

- **Emissão:** `UnusedLocalElementsVerifier._visitFieldElement` (`unused_local_elements_verifier.dart:1027-1032`), de `visitFieldDeclaration` (`:591`) e `visitEnumConstantDeclaration` (`:567`); mesmo caminho de `unused_element`, sem portão de sintaxe.
- **Condição exata (`_isReadMember`, `:827-858`):**
  ```
  público e não (static em classe/extensão privada) → lido
  isSynthetic → lido
  campo sem getter → não lido; senão E := getter
  lido ⇔ getter ∈ readMembers ou nome ∈ unresolvedReadMembers
  static → para; instância → ou sobrescreve membro usado
  ```
  `readMembers` só via `_isReadIdentifier` (`:416-440`): `x = e` não lê; identificador **simples** cujo pai é filho direto de `ExpressionStatement` não lê quando o pai é `++`/`--` ou composto (exceto `??=`); `this._f++;` lê (pai `PropertyAccess`). `values` de enum lê todas as constantes (`:311-318`). `PatternField`/`RelationalPattern` leem. Rótulo de argumento nomeado `C(_x: 1)` não lê; referência resolvida a parâmetro/local que sombreia não lê.
- **Posição:** o nome do campo/constante. **Mensagem:** `messages.yaml:27401` "The value of the field '{0}' isn't used.".
- **Supressões:** só `ignore`; sem portão de sintaxe.
- **No DartForge:** `privados.rs` `Regra::Campo`; `atribuido` (`:376`) só trata `x =` como escrita. Causas:
  1. **Tokens de declaração/outro elemento contados como leitura (22)** — parâmetro comum `int _`/`String? _foo` (`DuplicateDefinition__parameters_constru_*`, `duplicate_private_named_parameter/...`), `super._`, parâmetro do primário 3.13 (`…_primary_*`, `private_named_parameters/declaring_parameter_collision_error_test.dart` 16/73/108, `initializing_formal_collision_error_test.dart:10`), rótulo nomeado (`use_of_private_parameter_name/...andVoidLhsError.dart`), sombreamento (`UnusedField__isUsed_underscoreField_*`, `wildcard_variables/.../this_initializer_access_error_test.dart` 8/18). Mudança: registrar em `declaracoes` todos os parâmetros, `super.x`, locais e parâmetros de tipo; `this.x` do primário como escrita; descartar rótulos `nome:` de argumentos; excluir identificadores que `locais.rs` liga a local/parâmetro em escopo.
  2. **Composto/`++`/`--` como comando contados como leitura (3)** — `UnusedField__notUsed_{compoundAssign,postfixExpr,prefixExpr}.dart`. Mudança: implementar `_isReadIdentifier` no laço de tokens (também para variável de topo).
  3. **Portão de sintaxe (2)** — `field/decl_missing_var_type_test.dart:10`, `parameter/named_aggregated_test.dart:25`. Mesma mudança da causa 2 de `unused_element`.

##### `creation_with_non_type` (perda 22: FN 6, FP 16, msg 0, pos 0)

- **Emissão:** `_ErrorHelper.reportNewWithNonType` — `analyzer/lib/src/dart/resolver/named_type_resolver.dart:497-518`, chamado de `reportNullOrNonTypeElement` (`:598`), de `_verifyTypeAliasForContext` (`:478-481`, alias que não é de interface) e de `_rewriteToConstructorName` (`:370-372`, `C.x.y()` com `C` classe e nome de construtor já presente); via `NamedTypeResolver.resolve` (`:79-130`) na resolução do `ConstructorName` de uma `InstanceCreationExpression`. Os nomes reportados são `NEW_WITH_NON_TYPE` / `CONST_WITH_NON_TYPE`, ambos com `sharedName: CREATION_WITH_NON_TYPE` (`messages.yaml:3491`, `:3542`).
- **Condição exata:**
  ```
  NamedType dentro de ConstructorName dentro de InstanceCreationExpression ('new', 'const', ou sem palavra-chave quando o AstRewriter não reescreve):
    prefixo p: lookup(p).getter == null → _resolveToElement(null)
               PrefixElement → element = p.scope.lookup(nome).getter; _resolveToElement(element)
               classe/alias → _rewriteToConstructorName (p é a classe, nome é o construtor)
               outro → PREFIX_SHADOWED_BY_LOCAL_DECLARATION
    _resolveToElement(null): if !shouldIgnoreUndefinedNamedType(node) → reportNullOrNonTypeElement
    reportNullOrNonTypeElement: (boolean, catch, as, is, redirecionamento, argumento de tipo antes) → reportNewWithNonType
    elemento que não é tipo (função, variável, prefixo…) → reportNewWithNonType
  ```
  Sem `new`/`const`, o `AstRewriter.instanceCreationExpression` (`ast_rewrite.dart:38-115`) transforma `f<T>.x()`/`p.f<T>.x()` em invocação quando `f` é função/método/acessor (ou alias de função), e `x.a<T>.b()` com `x` que não é prefixo em `MethodInvocation` de `FunctionReference`; com `a` indefinido ou classe, mantém a criação.
- **Posição:** `_getErrorRange(node, skipImportPrefix: true)` (`:631-643`): do prefixo ao fim do nome, mas sem o prefixo quando ele é `PrefixElement` (só `X` em `p.X`).
- **Mensagem:** `The name '{0}' isn't a class.`, `{0}` = `node.name2.lexeme` (a última parte do nome do tipo).
- **Supressões:** `shouldIgnoreUndefinedNamedType` (`element.dart:1240-1245`): prefixo de import inexistente sem `show`, `show` que cita o nome, `_$` de parte gerada inexistente — só quando o elemento é nulo. Nome sintético (`name2.isSynthetic`) não sai.
- **No DartForge:** `crates/types/src/inferencia/chamadas.rs::criacao_sem_classe` (chamada de `instanciacao` para `new`/`const` e de `chamada` para `A<T>.x()`). Os **16 FP** são todos `new/const prefix.X…` com `import 'test.dart' as prefix` inexistente: o `elements` registra o prefixo mesmo sem biblioteca, então `prefixo_de_import_nao_resolvido` não dispara e o código sai. Mudança (feita): consultar `scope::deve_ignorar_indefinido(Some(p), X)` quando `p` é prefixo e, sem prefixo, quando o nome não resolve. **FN 6:** `UnresolvedClass<int>.named()`, `A<int>.named()`, `T<Null>.named()` (alias de tipo não interface — `generic_usage_dynamic/void_error`), `A<B>.foo()` (`regress34495`), `const B()` em inicializador estático (`method/not_found` — o inicializador de campo estático é inferido? confirmar): a forma `a<T>.b()` sem `new` chega como `Call{Property{TypeArguments{Identifier}}}` e não passa por `criacao_sem_classe` quando `a` não resolve; precisa do caminho do `ast_rewrite` (nome indefinido/alias não-interface → criação).

##### `unused_element_parameter` (perda 21: FN 21, FP 0, msg 0, pos 0)

Nome próprio só no 3.13; no 3.6.2 é `WarningCode.UNUSED_ELEMENT_PARAMETER` com `sharedName: UNUSED_ELEMENT` (`messages.yaml:27393`).

- **Emissão:** 3.6.2 `visitFormalParameterList` (`unused_local_elements_verifier.dart:602-610`); [3.13] `:917-929`.
- **Condição (`_isUsedElement`; 3.6.2 `:860-924`, [3.13] `:1222-1287`):**
  ```
  dono não é Constructor/Function(topo)/Method → usado
  !isOptional → usado
  construtor de classe genérica → usado
  construtor cujo superConstructor tem parâmetro correspondente obrigatório → usado
  executável genérico → usado
  _isPubliclyAccessible(executável) → usado (ctor generativo de enum: não; classe privada + ctor/static: não)
  _overridesUsedParameter → usado; pragma → usado
  senão usado ⇔ param ∈ elements
  ```
  `elements`: `staticParameterElement` dos argumentos (criação, invocação, `super(...)`, enum constant; `:360-365`); `super.x` **opcional** marca o do super (`:114-121`); redirecionamento de fábrica (`:93-111`); tear-off marca todos (`:288-305`); leitura.
- **Posição:** o nome do parâmetro. **Mensagem:** "A value for optional parameter '{0}' isn't ever given." ([3.13] `messages.yaml:31385`).
- **No DartForge:** `crates/types/src/parametros.rs::parametros_nao_usados`, chamado em `analise.rs:524-532` só fora de `libs_com_erro_de_sintaxe` e `libs_com_sintaxe_nova`. 21/21 vêm do pulo de `libs_com_sintaxe_nova` (`analise.rs:529`): primário de classe privada 19 (`UnusedElement__classPrivate_primaryCons_*`: `int? a`, `this.f`, `[super.a]`), primário de enum 2 (`InvalidReferenceToThis__enum_primaryCon_899bf0ed.dart`, `primary_constructors/header/optional_parameter_nonconstant_default_error_test.dart:15`). Mudança: entrada `unused_element_parameter` em `SUPLEMENTO_3_13` (com `unico` distinto); rodar `parametros_nao_usados` também em `libs_com_sintaxe_nova` e recodificar; conferir o primário em `program.functions` e o enum generativo como não público; corrigir `super_` posicional obrigatório (`parametros.rs:291`); os 2 `a` do arquivo `augment` pedem tirar do portão de sintaxe.

##### `prefix_shadowed_by_local_declaration` (perda 18: FN 18)

- **Emissão:** só `NamedTypeResolver.resolve` (`named_type_resolver.dart:119-125`), fase `ResolutionVisitor`; todo `NamedType` com prefixo (anotações, argumentos de tipo, `ConstructorName` de `new/const p.C(...)`).
- **Condição:** `pe = nameScope.lookup(prefixo).getter` não nulo e não é `PrefixElement`/`InterfaceElement`/`TypeAliasElement` (local, parâmetro, parâmetro de tipo, função/variável de topo, getter/método do contêiner, extensão, `dynamic`). Escopo léxico do ponto: locais de bloco valem desde a entrada (`resolution_visitor.dart:212-223`, `_buildLocalElements` `:1460-1468`), então a local posterior também esconde; retorno e tipos de parâmetro são resolvidos antes de `_defineParameters` (`:1039-1052`) — `a.Future? f(int a)` não dá erro. Membro só-setter: o oráculo não relata nada (`…__shado_2b7253a5.dart`).
- **Posição:** o token do prefixo. **Mensagem:** `The prefix '{0}' can't be used here because it's shadowed by a local declaration.` (`messages.yaml:13797`).
- **Supressões:** tipo inválido, sem outro erro de nome/criação; com local posterior também REFERENCED_BEFORE_DECLARATION no mesmo token (`error_verifier.dart:1081-1085`, `:5155-5169`); não suprime `unused_import`.
- **No DartForge:** não emitido. Criação com prefixo que é função de topo (FN 10, `CreationWithNonType__new_nonPrefix_named.dart`, `…const_nonType_named.dart`): `criacao_sem_classe` (`chamadas.rs`) já detecta e faz `return` mudo → emitir em `primeiro.span`. Criação com prefixo escondido por local/parâmetro de tipo (FN 2, `class/variable_shadow_class_test.dart`, `prefix/shadow_test.dart`): `instanciacao` não consulta o escopo léxico → `cx.buscar`, `nome_no_conteiner`. Anotação de corpo `a.Future? x` (FN 3, `PrefixShadowedByLocalDeclaration__local_*`): `tipos.rs::resolver_anotacao` precisa da busca léxica. Campo/getter do contêiner com o nome do prefixo (FN 3, `…__shado_a453bd79.dart`, `regress/regress34498_test.dart`): ramo prefixado de `resolve_annotation` olhar `type_param_scope`/`no_conteiner` (`SoSetter` mudo).

##### `prefix_identifier_not_followed_by_dot` (perda 15: FN 15)

- **Emissão:** (1) leitura: `SimpleIdentifierResolver._resolve1` (`simple_identifier_resolver.dart:199-205`); (2) `p(...)` sem alvo: `MethodInvocationResolver._resolveReceiverNull` (`method_invocation_resolver.dart:592-597`, relato `:272-278`); (3) escrita: `PropertyElementResolver.resolveSimpleIdentifier` (`property_element_resolver.dart:271-287`) → `AssignmentVerifier.verify` (`assignment_verifier.dart:66-71`).
- **Condição:** (1) elemento `PrefixElement` e `!_isValidAsPrefix` (`simple_identifier_resolver.dart:96-107`: só `ImportDirective.prefix`, pai `PrefixedIdentifier`, ou alvo de `MethodInvocation` com `.`) — `h?.x`, `h?.f()`, `h[0]`, argumento, cascata são erro; (2) getter léxico `PrefixElement`; (3) `requested == null` e `recovery` é `PrefixElement`. O prefixo é léxico (vence membro herdado).
- **Posição:** o identificador. **Mensagem:** `The name '{0}' refers to an import prefix, so it must be followed by '.'.` (`messages.yaml:13755`).
- **Supressões:** sintético; tipo inválido sem erros de membro; um por ocorrência; suprime avisos de import (já em `importacoes.rs:37`).
- **No DartForge:** não emitido. Valor/alvo genérico (FN 6: `FfiFromFunctionInvalidCode__fromFunctio_bd83e62a.dart`, `prefix/unqualified_invocation_test.dart`, `unsorted/illegal_invocation_test.dart`, `if_null/assignment_behavior_test.dart:203`): braço `RefNome::Prefixo` de `identificador` (`expr.rs`) só devolve `dynamic` → emitir. Atribuição (FN 4: `prefix/assignment_test.dart`, `PrefixIdentifierNotFollowedByDot__assig_3598171e.dart`, `h ??= null`): braços `Prefixo` em `ler_para_escrita`, `tipo_de_escrita_nome` e atribuição simples. `?.` tratado como `p.x` (FN 5: `null_aware/{access,assignment,invocation}_test.dart`): exigir `!null_aware` em `propriedade`, `escrita_propriedade` e no `p.f(args)` de `chamadas.rs`. Testar contra FP: `p.x`, `p.f()`, `p.C()`, `p.C<T>.n()`, `p.E.v`, `p.loadLibrary()`, `@p.x`, `p.C.new`.

##### `not_a_type` (perda 8: FN 8; código PUBLICADO)

- **Emissão:** `_ErrorHelper.reportNullOrNonTypeElement`, ramo `element != null` (`named_type_resolver.dart:624-633`), e `_rewriteToConstructorName` (`:372-380`, prefixo que é classe/alias fora de `ConstructorName`).
- **Condição:** (a) elemento que não é tipo, sem contexto anterior, e não local/função local (parâmetro, variável de topo, getter, método, função de topo → NOT_A_TYPE); (b) `A.n` com `A` classe/alias fora de `ConstructorName` — sempre, **antes** do contexto (também em `is`/`as`/argumento de tipo).
- **Posição:** (a) `_getErrorRange`; (b) de `importPrefix.offset` a `nameToken.end`. **Mensagem:** `{0} isn't a type.` (`messages.yaml:12395`): `name2` em (a), `'A.foo'` em (b).
- **Supressões:** sintético, cláusulas; redirecionamento de fábrica `= A.named` é `ConstructorName` (não sai); suprime avisos de import.
- **No DartForge:** (a) já em `resolve.rs::codigo_de_nome_de_tipo`. `A.foo` com `A` classe (FN 2, `NotAType__class_{constructor,method}.dart`): no ramo prefixado de `resolve_annotation`/`tipos.rs`, `Class`/`Typedef` → NOT_A_TYPE `"A.foo"` na faixa `name[0].start..name[1].end` — **nunca** no alvo de redirecionamento nem no tipo de `new/const` (o parser também produz `[A, foo]` ali). Método descartado pelo `)` que falta (FN 4, `base_class_syntax_error_test.dart:20`, `sealed_class_syntax_error_test.dart:20`) e declaração de topo descartada (FN 2, mesmas fontes 64/67): correções de parser (`garantir_fecha_parenteses`; variável de nome sintético). Opcional nos corpos: parâmetro achado por `cx.buscar` → NOT_A_TYPE (exige marcar `Local` como parâmetro).

##### `undefined_prefixed_name` (perda 7: FN 7)

- **Emissão:** `PropertyElementResolver._resolveTargetPrefixElement` — `analyzer/lib/src/dart/resolver/property_element_resolver.dart:744-786`, via `resolvePrefixedIdentifier` (leitura `p.x` de `PrefixedIdentifier`, e escrita por `resolveForWrite`).
- **Condição exata:** `lookup = prefix.scope.lookup(nome)`; `if (hasRead && lookup.getter == null || hasWrite && lookup.setter == null) && !forAnnotation && !shouldIgnoreUndefined(prefix: p, name: nome)` → relata. O escopo do prefixo devolve `loadLibrary` (a função sintética, `Future<dynamic> Function()`) quando algum import com esse prefixo é `deferred` e a biblioteca existe (`analyzer/lib/src/dart/element/scope.dart:576-583`). Invocação `p.x()` não passa aqui (é `UNDEFINED_FUNCTION`). Atenção: escrita em `p.x = v` com `x` final de topo (sem setter) também cai aqui.
- **Posição:** o identificador `x` (depois do ponto).
- **Mensagem:** `The name '{0}' is being referenced through the prefix '{1}', but it isn't defined in any of the libraries imported using that prefix.` — `{0}` = nome, `{1}` = prefixo.
- **Supressões:** anotações (`forAnnotation`), `shouldIgnoreUndefined`.
- **No DartForge:** `expr.rs::propriedade` (leitura com prefixo) relatava `UNDEFINED_IDENTIFIER`; mudança (feita): `load_library` e `avisar_nome_prefixado_indefinido` (`UNDEFINED_PREFIXED_NAME` com `shouldIgnoreUndefined`). Escrita (`escrita_propriedade` com prefixo) hoje não relata nada — falta, com cuidado com a variável final (setter ausente).

##### `non_type_as_type_argument` (perda 6: FN 5, FP 1)

- **Emissão:** `reportNullOrNonTypeElement`, ramo `_isTypeInTypeArgumentList` (`named_type_resolver.dart:585-595`, `:708-710`); argumentos resolvidos antes da cabeça (`resolution_visitor.dart:1097`).
- **Condição:** `node.parent is TypeArgumentList` (qualquer origem, inclusive em cláusulas); `element == null` (com `shouldIgnore`) ou elemento não tipo, inclusive local (antes do REFERENCED_BEFORE_DECLARATION). Precedido por sintético, `boolean`, catch/as/is, redirect.
- **Posição:** `_getErrorRange`. **Mensagem:** `The name '{0}' isn't a type, so it can't be used as a type argument.` (`messages.yaml:12143`).
- **No DartForge:** `ContextoDeTipo::ArgumentoDeTipo`. Cabeça parâmetro de tipo com argumentos (FN 3, `malformed/inheritance_test.dart:47/64/81`): `resolve.rs` passo 1 (~1415-1427) e `tipos.rs` (~361-374) retornam sem resolver `args` → resolver sempre os argumentos antes. Local como argumento de tipo (FN 2, `patterns/version_2_32_changes_error_test.dart:112/121`): `tipos.rs` não consulta locais → `cx.buscar` (confirmar variáveis do padrão no escopo durante o inicializador, `resolution_visitor.dart:1138-1147`). FP 1 (`NonTypeAsTypeArgument__issue54388.dart`): oráculo vazio — provável exceção do analyzer (`driver.dart:1472-1500`); excluir o arquivo do grupo, não mudar a regra.

##### `unused_label` (perda 5: FN 5, FP 0, msg 0, pos 0)

- **Emissão:** `DeadCodeVerifier._withLabelTracker` (`analyzer/lib/src/error/dead_code_verifier.dart:159-174`), primeiro verificador de `_computeWarnings` (`library_analyzer.dart:468`), sem portão de sintaxe.
- **Condição:**
  ```
  visitLabeledStatement (:97-101): novo _LabelTracker(outer, labels) em volta do corpo
  visitSwitchStatement (:104-112): UM tracker com os labels de TODOS os membros
  break/continue (:43-50): recordUsage(label?.name) — tracker atual, senão o externo
  ao sair: relata label não usado (:583-589)
  ```
  Sem reinício em fronteira de função; `continue` para rótulo de outro `switch` não acha; nomes repetidos: o mapa guarda o **último** índice; `L:` depois de `case 17:` é `LabeledStatement` e `break;` sem rótulo não o usa; switch expression não tem rótulos.
- **Posição:** o nó `Label`: identificador até o `:` inclusive (`analyzer/lib/src/dart/ast/ast.dart:11052`).
- **Mensagem:** `messages.yaml:27470` "The label '{0}' isn't used.".
- **No DartForge:** não existe (`locais.rs:683` só desce no corpo). 5/5 não implementado: `LabelUndefined__{break,continue}.dart`, `const_functions/const_functions_switch_statements_error_test.dart:16`, `switch/switch7_test.dart:9`, `switch/case_fallthrough_legacy_error_test.dart:244`. Mudança: passada nova (`crates/analise/src/rotulos.rs`) com pilha de trackers sobre todos os comandos; emitir `UNUSED_LABEL` do nome ao `:`; sem filtro de sintaxe, salvo o comando com erro de recuperação.

##### `ambiguous_import` (perda 3: FN 2, FP 0, msg 1, pos 0)

- **Emissão:** `ErrorVerifier._checkForAmbiguousImport` (`analyzer/lib/src/generated/error_verifier.dart:2116-2130`), de `visitNamedType` (`:1268`) e `visitSimpleIdentifier` (`:1412`).
- **Condição:** o elemento resolvido é `MultiplyDefinedElementImpl` — `PrefixScope._merge` (`scope.dart:669-692`) o cria quando dois imports do mesmo prefixo trazem elementos diferentes de mesmo nome (SDK × não-SDK: vence o de fora, sem erro). Vale também para `p.foo` em expressão.
- **Posição:** o token do nome. **Mensagem:** "The name '{0}' is defined in the libraries {1}." — `{1}`: `_getLibraryName` (`:6278-6320`) de cada conflitante, ordenado, `quotedAndCommaSeparatedWithAnd`; URI da biblioteca **declarante**, com ` (via X)` se não importada diretamente.
- **Supressões:** nenhuma; suprime os avisos de import.
- **No DartForge:** só de anotação de tipo (`types/src/resolve.rs:1485`), traduzida em `paridade/src/ponte.rs:149` com `vec![nome, ""]` (daí a mensagem errada em `AmbiguousImport__systemLibrary_systemLibrary.dart`); `Binding.ambiguous` (`elements/src/model.rs:212`) é só `bool`. FN: `prefix/import_collision_test.dart` (`lib2.foo = 1`), `regress/regress19413_test.dart` (`foo.f()`). Proposta: mapa lateral `Library.ambiguidades` com os elementos (preenchido em `merge_binding_com`, `outline.rs:1569`); montar `{1}`; na inferência, alvo prefixado/identificador ambíguo → `AMBIGUOUS_IMPORT` no nome, tipo `dynamic`.

##### `part_of_non_part` (perda 2: FN 2)

- **Emissão:** `LibraryAnalyzer._resolvePartDirective` (`library_analyzer.dart:971-1032`).
- **Condição:** URI com interpolação/ inválida/ inexistente têm seus códigos; arquivo existente cujo `kind` não é `PartFileKind` (sem `part of`) → `PART_OF_NON_PART` (gerado → `URI_HAS_NOT_BEEN_GENERATED`), antes de `DUPLICATE_PART`: `part` do próprio arquivo e `part 'dart:_foreign_helper'` também.
- **Posição:** `directive.uri`. **Mensagem:** "The included part '{0}' must have a part-of directive." (`includedFile.uriStr`: `dart:…`, `package:…`, `file:///abs`).
- **No DartForge:** não emitido (`elements/src/load.rs:572-580` pula a parte que já é unidade; `verify_part_of` (`:1398-1407`) gera diagnóstico sem código no span 0). Exemplos: `import/internal_library_test.dart`, `part/self_test.dart`. Proposta: fase 2 de `paridade/src/analise.rs` para `Part` com alvo existente sem `PartOf`.

##### `type_test_with_undefined_name` (perda 2: FP 2)

- **Emissão:** `reportNullOrNonTypeElement`, ramo `_isTypeInIsExpression` com `element == null` (`named_type_resolver.dart:555-573`).
- **Condição:** pai `IsExpression`; nulo → este código; qualquer elemento (inclusive local) → TYPE_TEST_WITH_NON_TYPE; busca léxica. **Mensagem:** `The name '{0}' isn't defined, so it can't be used in an 'is' expression.` (`messages.yaml:16515`).
- **No DartForge:** local tratado como indefinido (FP 1, `is/not_class2_test.dart:19`): `tipos.rs::resolver_anotacao` sem `cx.buscar`. Typedef descartado pelo parser (FP 1, `typedef/bad_typedef_test.dart:18`): `garantir_fecha_parenteses`.

##### `ambiguous_export` (perda 1: FN 1)

- **Emissão:** `ErrorVerifier._checkForAmbiguousExport` (`error_verifier.dart:2085-2111`), de `visitExportDirective` (`:731-739`).
- **Condição:** exports em ordem, mapa compartilhado nome→elemento; `prev != null && prev != element` → relata e **retorna** (o resto da diretiva não entra); mesmo elemento por dois caminhos não é erro.
- **Posição:** `uri` da segunda diretiva. **Mensagem:** "The name '{0}' is defined in the libraries '{1}' and '{2}'." (URIs das declarantes).
- **No DartForge:** não existe; exemplo `export/ambiguous_main_test.dart`. Proposta: passada em `crates/analise` sobre `program.library(lib).exports`.

##### `deprecated_member_use` (perda 1: FN 1)

- **Emissão:** `BestPracticesVerifier.visitSimpleIdentifier` (`analyzer/lib/src/error/best_practices_verifier.dart:728-732`) → `BaseDeprecatedMemberUseVerifier.simpleIdentifier` → `_checkForDeprecated` (`deprecated_member_use_verifier.dart:151-246`) → `reportError` (`:340-372`).
- **Condição:** todo `SimpleIdentifier` fora de declaração, inclusive nome de `show` (`hide` excluído, `:136-139`); elemento `@Deprecated`; não relata dentro de declaração depreciada; mesmo pacote → `…_FROM_SAME_PACKAGE`.
- **Posição:** o identificador. **Mensagem:** "'{0}' is deprecated and shouldn't be used." ou, com mensagem, `…_WITH_MESSAGE` "'{0}' is deprecated and shouldn't be used. {1}".
- **No DartForge:** não existe; exemplo `type_promotion/closure_test.dart:7` (`show virtual`). Proposta mínima: nomes de `show` cujo elemento é depreciado.

##### `type_annotation_deferred_class` (perda 1: FN 1)

- **Emissão:** `ErrorVerifier._checkForTypeAnnotationDeferredClass` (`analyzer/lib/src/generated/error_verifier.dart:5369-5377`), de `visitAsExpression` (`:342`), `visitIsExpression` (`:1138`), catch (`:442`), `FieldFormalParameter`/`SimpleFormalParameter` (`:899`, `:1406`), retornos de função/método/parâmetro funcional (`:963`, `:1030`, `:1179`), **cada elemento de toda `TypeArgumentList`** (`:1576-1580`), limite de parâmetro de tipo (`:1588`), `VariableDeclarationList` (`:1630`). Fora: cláusulas, typedef, `GenericFunctionType`, record, padrões, `for-in`, `new lib.K()`.
- **Condição:** `type is NamedType && type.isDeferred` (`analyzer/lib/src/dart/ast/ast.dart:12695-12705`): prefixo `PrefixElement` com **exatamente um** import, e ele `deferred`; independe de o nome resolver.
- **Posição:** o `NamedType` inteiro. **Mensagem:** `The deferred type '{0}' can't be used in a declaration, cast, or type test.` (`messages.yaml:16288`), `{0}` = `prefixo.Nome`.
- **No DartForge:** não implementado; FN `regress/regress23408a_test.dart:17` (`factory C.l() = A<lib.K>;`). Proposta: verificador sintático em `crates/analise` nas posições acima (pode reaproveitar `Leitor::adiado`, `clausulas.rs:200-210`, exigindo `count == 1` e prefixo não escondido).

##### `undefined_identifier_await` (perda 1: FN 1)

- **Emissão:** `SimpleIdentifierResolver._resolve1` (`simple_identifier_resolver.dart:209-214`) e `_ErrorHelper.reportNullOrNonTypeElement` (`named_type_resolver.dart:621-627`, tipo chamado `await`).
- **Condição:** leitura de `await` sem elemento e `_resolver.enclosingFunction != null` (qualquer corpo de função, método, construtor, closure; não inicializador de variável de topo/campo); no tipo: `importPrefix == null && name == 'await'` depois dos outros contextos.
- **Posição:** o identificador. **Mensagem:** `Undefined name 'await' in function body not marked with 'async'.` (sem argumentos).
- **No DartForge:** o tipo já sai (`resolve.rs::codigo_de_nome_de_tipo`); a expressão saía como `undefined_identifier`. Mudança (feita): `nome_indefinido_sem_this` relata `UNDEFINED_IDENTIFIER_AWAIT` quando há corpo de função (`cx.raiz` não `Nada` ou `cx.funcoes` não vazio).

##### `undefined_shown_name` (perda 1: FN 1)

- **Emissão:** `DeadCodeVerifier._checkCombinator` (`analyzer/lib/src/error/dead_code_verifier.dart:131-155`), de `visitImportDirective` (`:81-92`)/`visitExportDirective` (`:53-62`).
- **Condição:** biblioteca alvo existe e não é sintética; nome de `show` (`hide` → `UNDEFINED_HIDDEN_NAME`) sem `n` nem `n=` no `exportNamespace`. Não depende de uso nem da porta de supressão.
- **Posição:** o identificador do combinador. **Mensagem:** "The library '{0}' doesn't export a member with the shown name '{1}'." (`{0}` = URI da biblioteca).
- **No DartForge:** não existe (`importacoes.rs:173-174` só pula); exemplo `UnusedShownName__unresolved.dart`. Proposta: em `importacoes`, fora da porta, emitir para nomes ausentes de `alvo.exported`.

##### Códigos vizinhos que a correção de `undefined_identifier` passa a emitir

Não estão na lista F, mas são o "lugar" dos FP acima (sem eles o FP vira FN do outro código): `undefined_function` (perda 25, FN 25), `undefined_method` sem alvo em classe (parte dos FN 56), `instance_member_access_from_static` (FN 18), `implicit_this_reference_in_initializer` (FN 17, só os de inicializador de construtor), `unqualified_reference_to_non_local_static_member` (FN 9), `unqualified_reference_to_static_member_of_extended_type` (FN 8). Regras e linhas na seção de `undefined_identifier`.
