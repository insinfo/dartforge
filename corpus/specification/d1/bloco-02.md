
#### 2. `ConstantVisitor` (`evaluation.dart:525-2027`)

Um visitante por avaliação: `ConstantVisitor(engine, library, errorReporter, {lexicalEnvironment,
lexicalTypeEnvironment, substitution})` (`:556-570`). `_lexicalEnvironment` (nome → valor) e
`_lexicalTypeEnvironment` (parâmetro de tipo → tipo) só existem no visitante dos inicializadores de um
construtor em avaliação (`_initializerVisitor`, `:2428-2435`); `_substitution` = a da classe instanciada.
`evaluateConstant(node)` = `node.accept(this)` (`:606-615`); tudo o que não tem `visit*` próprio cai em
`visitNode` → `InvalidConstant.genericError(node)` (`:1091-1095`).

##### 2.1 `InvalidConstant` (`value.dart:2382-2496`)

| campo | significado | quem liga |
|---|---|---|
| `offset`, `length` | intervalo do relato | `forEntity(entidade)`: nó ou token; `forElement(elemento)`: `nameOffset`/`nameLength` |
| `errorCode`, `arguments` | código e argumentos da mensagem | — |
| `contextMessages` | mensagens "The error is in…", "The exception is…", "The evaluated constructor…" | §3.4 |
| `avoidReporting` (mutável) | não relatar em lugar nenhum | leitura de const cujo valor é inválido (`:1782-1784`); literal `{…}` ambíguo (`:1281-1287`) |
| `isRuntimeException` | "lançaria em execução": vira `CONST_EVAL_THROWS_EXCEPTION` no **nó da criação** mais externa que o contém | `~/` por zero (`:2147-2156`), `assert` falso (`:2815-2827`), desencontro de tipo estaticamente atribuível em parâmetro/campo (`:2909-2920`, `:2735-2751`), campo de tipo genérico (`:2636-2645`) |
| `isUnresolved` | expressão não resolvida / const ainda sem valor: como argumento de criação é trocada por objeto inválido (`_valueOf`) | `:1054`, `:1771`, `:1784`, `:1865` |

Fábricas: `forEntity` (`:2454-2469`), `forElement` (`:2435-2450`), `copyWithEntity(outro, entidade)` — mesmo
erro, novo intervalo, **mesma lista** de contextos (`:2421-2432`) —, e
`genericError(node, {isUnresolved})` (`:2472-2486`):
```
se node.parent is ArgumentList e node.parent.parent is InstanceCreationExpression com isConst:
     CONST_WITH_NON_CONSTANT_ARGUMENT em node        // só argumento POSICIONAL direto de criação const
senão INVALID_CONSTANT em node                       // argumento nomeado: o pai é NamedExpression → INVALID_CONSTANT
```
Anotação (`@A(v)`) e constante de enum (`v(a)`) não são `InstanceCreationExpression`: o genérico dos
argumentos é `INVALID_CONSTANT`, e o `CONST_WITH_NON_CONSTANT_ARGUMENT` visto ali vem do **código padrão** do
verificador (`_validateConstantArguments`, `constant_verifier.dart:779-786`).

##### 2.2 Tabela dos `visit*` (ordem de avaliação = ordem das colunas "avalia"; o **primeiro** `InvalidConstant` é devolvido tal qual, salvo indicação)

| nó | linha | avalia | resultado / erro (código @ nó) |
|---|---|---|---|
| `BooleanLiteral`, `DoubleLiteral`, `NullLiteral`, `SimpleStringLiteral`, `InterpolationString` | `:748`, `:843`, `:1098`, `:1321`, `:979` | — | valor (`BoolState`, `DoubleState`, `NullState.NULL_STATE`, `StringState(node.value)`) |
| `IntegerLiteral` | `:949-962` | — | `staticType == double` → `DoubleState(value?.toDouble())`; senão `IntState(node.value)` (valor `null` = literal fora da faixa → desconhecido) |
| `SymbolLiteral` | `:1335-1349` | — | `SymbolState` com os componentes unidos por `.` |
| `AdjacentStrings`, `StringInterpolation` | `:618`, `:1330`, `_concatenateNodes` `:1652-1679` | cada parte, na ordem | `concatenate` acumulado @ nó inteiro; zero partes → `String` desconhecida |
| `InterpolationExpression` | `:965-976` | a expressão | `!isBoolNumStringOrNull` → `CONST_EVAL_TYPE_BOOL_NUM_STRING` @ o `${…}`/`$x` inteiro; senão `performToString` |
| `ParenthesizedExpression`, `NamedExpression` | `:1103`, `:1062` | o interior | idem (o erro fica no nó interno, sem os parênteses) |
| `PrefixExpression` | `:1142-1173` | 1º: operador de extensão/extension type → `CONST_EVAL_EXTENSION_METHOD` / `CONST_EVAL_EXTENSION_TYPE_METHOD` @ nó; 2º: **o operando** (erro dele volta antes de olhar o operador) | `!` → `logicalNot`; `~` → `bitNot`; `-` → `negated`; outro (`++`, `--`) → genérico @ nó. Exceção @ nó inteiro |
| `BinaryExpression` | `:636-745` | §2.3 | §2.3 |
| `ConditionalExpression` | `:757-801` | condição | `!isBool` → `CONST_EVAL_TYPE_BOOL` @ **condição**; `true`: `_reportNotPotentialConstants(else)` (1º nó não potencialmente constante → `INVALID_CONSTANT` nele), depois avalia `then`; `false`: simétrico; **desconhecida**: avalia `then` e `else` (erro de qualquer um volta) e devolve `validWithUnknownValue(node.staticType)` |
| `AsExpression` | `:623-633` | expressão, depois o tipo | `castToType` (§4.3) @ nó `x as T` inteiro |
| `IsExpression` | `:988-998` | expressão, depois o tipo | `hasType` + `logicalNot` se `is!` (§4.3) |
| `NamedType` | `:1066-1088` | — | literal de tipo em padrão constante com parâmetro de tipo → `CONST_TYPE_PARAMETER` @ nó; `isDeferred` → erro de deferred (§2.6) @ `name2`; senão `_getConstantValue(element: node.element, givenType: tipo com _substitution aplicada)` |
| `TypeLiteral` | `:1352` | `node.type` | idem `NamedType` |
| `GenericFunctionType`, `RecordTypeAnnotation` | `:914`, `:1246` | — | `TypeState(node.type)` (sem substituição) |
| `SimpleIdentifier` | `:1306-1318` | — | `_lexicalEnvironment[name]` se existe (com instanciação implícita de tear-off); senão `_getConstantValue` (§2.4) |
| `PrefixedIdentifier` | `:1107-1139` | prefixo | prefixo é `PrefixElement`: se `isDeferred` → erro de deferred @ `identifier`; senão direto a `_getConstantValue`. Prefixo é extensão: direto a `_getConstantValue`. Senão avalia o prefixo (erro volta); se o prefixo **não** é `InterfaceElement`: `_evaluatePropertyAccess` (§2.5) |
| `PropertyAccess` | `:1176-1212` | alvo | alvo `PrefixedIdentifier` que é extensão/extension type: deferred → erro @ `target.identifier`; senão `_getConstantValue`. Senão avalia o alvo (erro volta) e `_evaluatePropertyAccess`; se devolver `null`, `_getConstantValue` |
| `MethodInvocation` | `:1023-1059` | — | `identical` de `dart:core` com exatamente 2 argumentos: avalia arg0, arg1, `isIdentical2`; `staticType is InvalidType` → `INVALID_CONSTANT` @ nó com `isUnresolved`; qualquer outra → `CONST_EVAL_METHOD_INVOCATION` @ nó (sem avaliar alvo nem argumentos) |
| `InstanceCreationExpression` | `:923-946` | — | `!isConst` → genérico @ nó; construtor não resolvido → `INVALID_CONSTANT` @ nó; senão `evaluateAndFormatErrorsInConstructorCall` (§3) |
| `ConstructorReference` | `:804-840` | — | tipo não é função ou construtor não resolvido → `INVALID_CONSTANT` @ nó; senão `FunctionState(construtor, typeArguments do tipo de retorno, viaTypeAlias se o alias não é renome próprio)` com tipo `node.staticType` |
| `FunctionReference` | `:852-911` | a função | sem argumentos escritos: algum `typeArgumentTypes` (com `_lexicalTypeEnvironment` aplicado aos que são parâmetro de tipo) com referência a parâmetro de tipo → `CONST_WITH_TYPE_PARAMETERS_FUNCTION_TEAROFF` @ nó; senão `_instantiateFunctionType` (`:1939-1961`, aplica `_substitution`). Com argumentos escritos: avalia cada um; `CONST_TYPE_PARAMETER` é trocado por `…_FUNCTION_TEAROFF` @ o argumento; não-tipo → `INVALID_CONSTANT` @ argumento; nº errado → `WRONG_NUMBER_OF_TYPE_ARGUMENTS_FUNCTION` (função é `SimpleIdentifier`; args `[nome, formais, dados]`) ou `…_ANONYMOUS_FUNCTION` @ a lista `<…>` (`typeInstantiate`, `:2308-2338`) |
| `ListLiteral` | `:1001-1020` | elementos (§2.7) | `!isConst` → `MISSING_CONST_IN_LIST_LITERAL` @ nó |
| `SetOrMapLiteral` | `:1255-1303` | elementos | mapa = `!node.isSet`. `!isConst` → `MISSING_CONST_IN_MAP_LITERAL` / `…_SET_LITERAL` @ nó. Erro em literal que não é `isMap` nem `isSet` (ambíguo): `avoidReporting = true` |
| `RecordLiteral` | `:1215-1243` | campos na ordem escrita | `RecordState`; tipo = registro dos **tipos dos valores**. Não há checagem de `const` (registro em contexto não constante avalia igual) |
| qualquer outro (`AssignmentExpression`, `PostfixExpression`, `FunctionExpression`, `ThrowExpression`, `CascadeExpression`, `IndexExpression`, `AwaitExpression`, `ThisExpression`, `SuperExpression`, `SwitchExpression`, `PatternAssignment`, `FunctionExpressionInvocation`, `NullAssertion`=`PostfixExpression`, `IsExpression` é próprio…) | `:1091-1095` | — | genérico @ nó inteiro |

Conferido (`c34`): `const a = v++` → `off=21 len=3` (nó); `const b = ++v` → `off=38 len=1` (**o operando `v`**, avaliado
antes do operador); `-v` → em `v`; `v = 2` → nó (`len=5`); `() {}` → nó; `v!` → nó (`len=2`); `(v)` → em `v`
(sem parênteses); `v..toString()` → nó (`len=13`). Todos como `const_initialized_with_non_constant_value`
(código padrão do uso, §5).

`inConstantContext` (`ast.dart:6208-6260`) sobe por `Expression`, `ArgumentList`, `IfElement`, `ForElement`,
`MapLiteralEntry`, `SpreadElement`, `VariableDeclaration`; qualquer outro nó **interrompe** a subida — em
particular `InterpolationExpression` (não é `Expression`): `const b = '${[1]}'` → a lista não está em contexto
constante → `MISSING_CONST_IN_LIST_LITERAL` → relatada como `const_initialized_with_non_constant_value off=62
len=3` (`c25`).

##### 2.3 `visitBinaryExpression` (`:636-745`)

```
1. operador declarado em extensão → CONST_EVAL_EXTENSION_METHOD @ nó; em extension type → …_EXTENSION_TYPE_METHOD @ nó
   (operador não resolvido, staticElement null, segue: `const c = a + 1` com `a` extension type de int dá 2 sem erro, c37)
2. esquerda = evaluateConstant(left); inválida → devolve
3. `&&`: se esquerda == false:  _reportNotPotentialConstants(right) (1º nó ruim → INVALID_CONSTANT nele)
         lazyAnd(node, esquerda, computeRight)
   `||`: se esquerda == true:   idem;  lazyOr(...)
   `??`: se esquerda != null:   idem;  lazyQuestionQuestion: esquerda nula → evaluateConstant(right) (erro volta intacto); senão esquerda
   computeRight (para && e ||): avalia a direita; se inválida LANÇA EvaluationException(código dela)        (:659-667)
        → o DartObjectComputer captura e cria InvalidConstant.forEntity(NÓ BINÁRIO INTEIRO, código)        (:2169-2185)
        → perdem-se offset, argumentos, contextos e as marcas isUnresolved/isRuntimeException da direita
4. demais: direita = evaluateConstant(right); inválida → devolve; depois a operação; exceção @ nó inteiro
   operadores: & | ^ == != > >= >> >>> < <= << - % + * / ~/ ; qualquer outro token → genérico @ nó      (:702-744)
```
Efeito visível do passo 3 (`u01`): `const b = false || 'a'.isEmpty;` →
`const_eval_property_access off=52 len=20 2:11 | The property '{0}' can't be accessed on the type '{1}' in a
constant expression.` (nó inteiro, **mensagem com os marcadores sem preencher**: os argumentos se perderam);
`const a = true && ('x'.length ~/ 0 == 1);` → `const_eval_throws_idbze off=10 len=30` (nó inteiro);
`const e = u && v2` com `u` desconhecido e `v2` não const → `const_initialized_with_non_constant_value off=147
len=7` (o `&&` inteiro, `t14`). Com `??` o erro da direita fica na direita (`const c = null ?? 'a'.isEmpty` →
`off=92 len=11`).

Tabela operador × estados (esq., dir.) → resultado ou código (tudo @ nó binário; `?` = valor desconhecido do
mesmo tipo; `N` = `NullState`). Fonte: `value.dart` — `DartObjectImpl` `:286-872`, `InstanceState` `:1551-1949`,
`IntState` `:1952-2379`, `DoubleState` `:1014-1311`, `BoolState` `:27-131`, `StringState` `:2972-3049`.

| operador | esquerda | direita | resultado |
|---|---|---|---|
| `+` | int/double | int/double | soma (int+int → int; senão double); algum `?` → `?` do tipo do resultado |
| `+` | int/double | `N` | `CONST_EVAL_THROWS_EXCEPTION` (`assertNumOrNull` passa, cai no `throw` final, `:2000`, `:1057`) |
| `+` | int/double | outro (String, bool, lista…) | `CONST_EVAL_TYPE_NUM` (`1 + 'a'`) |
| `+` | String | String | concatenação |
| `+` | String | não String (num, `N`) | `CONST_EVAL_THROWS_EXCEPTION` (`'a' + 1`; `InstanceState.add` `:1584-1591`: os dois passam em `assertNumStringOrNull`) |
| `+` | String | bool, lista, objeto… | `CONST_EVAL_TYPE_NUM_STRING` |
| `+` | `N` | num/String/`N` | `CONST_EVAL_THROWS_EXCEPTION` (`null + 1`) |
| `+` | bool, lista, objeto, tipo… | qualquer | `CONST_EVAL_TYPE_NUM_STRING` (`true + 1`, `[] + []`, `int + 1`) |
| `-` `*` | num | num | aritmética (int∘int → int; senão double) |
| `-` `*` `/` `%` `~/` `<` `<=` `>` `>=` | num | `N` | `CONST_EVAL_THROWS_EXCEPTION` |
| idem | num | não num, não `N` | `CONST_EVAL_TYPE_NUM` (`1 < 'a'`) |
| idem | `N` | num/`N` | `CONST_EVAL_THROWS_EXCEPTION`; dir. outro → `CONST_EVAL_TYPE_NUM` |
| idem | não num, não `N` | qualquer | `CONST_EVAL_TYPE_NUM` (`'a' * 2`, `t08`) |
| `/` | num | num | **sempre double**, sem erro por zero (`1 / 0` → infinito, `c05`); int `?` → double `?` |
| `~/` | int | int 0 | `CONST_EVAL_THROWS_IDBZE` com `isRuntimeException: true` (`:2147-2149`; o `integerDivide` do computador é o **único** que repassa a marca, `evaluation.dart:2147-2157`) |
| `~/` | int | int ≠ 0 | `IntState(a ~/ b)` |
| `~/` | int | double | `a / b` finito → `toInt()`; não finito → `CONST_EVAL_THROWS_EXCEPTION` |
| `~/` | double | int/double | quociente finito → `toInt()`; não finito (`1.5 ~/ 0`) → `CONST_EVAL_THROWS_EXCEPTION` (**não** IDBZE) |
| `%` | int | int 0 | `CONST_EVAL_THROWS_EXCEPTION` (**não** IDBZE; `:2299-2309`) |
| `%` | int | int ≠ 0 / double | `a % b` |
| `<<` `>>` | int | int ≥ 0 com `bitLength ≤ 31` | deslocamento; `bitLength > 31` → int `?` (`:2322`, `:2342`) |
| `<<` `>>` | int | int negativo | `CONST_EVAL_THROWS_EXCEPTION` (`1 << -1`) |
| `>>>` | int | int ≥ 64 → 0; 0..63 → `(a >> b) & ((1 << (64 - b)) - 1)`; negativo → `CONST_EVAL_THROWS_EXCEPTION` | |
| `<<` `>>` `>>>` | int | `N` | `CONST_EVAL_THROWS_EXCEPTION`; não int → `CONST_EVAL_TYPE_INT` |
| `<<` `>>` `>>>` | não int (double incluso), não `N` | — | `CONST_EVAL_TYPE_INT`; `N` à esquerda → `CONST_EVAL_THROWS_EXCEPTION` (ou `TYPE_INT` se a direita não é int/`N`) |
| `&` `\|` `^` | bool | bool | lógica (algum `?` → bool `?`) |
| `&` `\|` `^` | int | int | bit a bit |
| `&` `\|` `^` | qualquer outra combinação (`true & 1`, `1 & 1.0`, `null & 1`) | | `CONST_EVAL_TYPE_BOOL_INT` (`eagerAnd/Or/Xor`, `:414-476`: testa `isBool && isBool` senão `isInt && isInt`) |
| `&&` | bool `false` | (não avaliada; só potencialmente constante) | `false` |
| `&&` | bool `true`/`?` | bool | `true` → valor da direita; `?` → bool `?` |
| `&&` | bool `true`/`?` | não bool | `CONST_EVAL_TYPE_BOOL` (`true && 1`) |
| `&&` `\|\|` | não bool | — | `CONST_EVAL_TYPE_BOOL` (`1 && true`; `InstanceState.lazyAnd` `:1762-1770`) |
| `\|\|` | simétrico de `&&` com `true` como curto-circuito | | |
| `??` | não nulo | (só potencialmente constante) | a esquerda |
| `??` | `N` | qualquer | o valor da direita |
| `==` `!=` | algum lado `N` | | `isNull && isNull` (sem erro para nenhum tipo) |
| `==` `!=` | com `patterns` (≥ 3.0): esquerda `DoubleState` ou `hasPrimitiveEquality` | qualquer | `state.equalEqual` = `isIdentical` do estado (§4.2) |
| `==` `!=` | sem `patterns`: esquerda `isBoolNumStringOrNull` | qualquer | idem |
| `==` `!=` | senão (instância cuja classe redefine `==`/`hashCode`, registro com campo assim) | | `CONST_EVAL_TYPE_BOOL_NUM_STRING` |

Só a **esquerda** decide se `==` é permitido. Conferido (`c25`, `t06`, `t08`): `const O() == const O()`,
`[1] == [1]`, `(1, 2) == (1, 2)`, `#a == #a`, `1.0 == 1`, `'a' == 1`, `0.0 == -0.0`, `t == int` → sem erro;
`const Q() == const Q()` com `Q` redefinindo `==` → `const_eval_type_bool_num_string off=263 len=22`.

Prefixos: `!` sobre não bool → `CONST_EVAL_TYPE_BOOL` (`!1`, também sobre int **desconhecido**: o teste é do
estado, não do valor); `!null` → `CONST_EVAL_THROWS_EXCEPTION` (`NullState.logicalNot`, `:2739-2741`); `~` sobre
não int → `CONST_EVAL_TYPE_INT` (`~1.5`), `~null` → `CONST_EVAL_THROWS_EXCEPTION`; `-` sobre não num →
`CONST_EVAL_TYPE_NUM` (`-'a'`), `-null` → `CONST_EVAL_THROWS_EXCEPTION`.

##### 2.4 `_getConstantValue` (`:1736-1871`) — identificador, membro prefixado, nome de tipo

Entrada: `errorNode`, `expression` (o nó como expressão; `null` para `NamedType`), `identifier` (o identificador
final; `null` para `NamedType`), `element` (→ `declaration`; getter → `variable2`).

| ordem | condição | resultado |
|---|---|---|
| 0 | `expression` é `SimpleIdentifier` com `tearOffTypeArgumentTypes` contendo parâmetro de tipo | `CONST_TYPE_PARAMETER` @ expressão (`:1751-1756`) |
| 1 | `VariableElementImpl` **const**, `evaluationResult == null` | genérico @ `errorNode`, `isUnresolved: true` (`:1767-1771`) |
| 1 | idem, resultado `DartObjectImpl` | o valor (com instanciação implícita por `tearOffTypeArgumentTypes` do identificador, `:1967-1987`); `identifier == null` → `INVALID_CONSTANT` |
| 1 | idem, resultado `InvalidConstant` | `INVALID_CONSTANT` @ `errorNode`, `isUnresolved: true`, `avoidReporting: true` (`:1779-1784`) |
| 1 | `VariableElementImpl` **não const** (variável comum, `final`, parâmetro fora do ambiente léxico, local) | cai para o fim (genérico) |
| 2 | `ConstructorElementImpl` e `expression != null` | `FunctionState(construtor)`, tipo `expression.staticType` (`:1787-1793`) |
| 3 | `ExecutableElementImpl` **estático** (função de topo, método estático) | `FunctionState(elemento)` de tipo `element.type`, mais instanciação implícita (`:1794-1806`); não estático (método de instância, getter não sintético) → fim |
| 4 | `InterfaceElement` (classe, enum, mixin, extension type) | `TypeState(givenType ?? instanciação com dynamic)` (`:1807-1819`) |
| 5 | `dynamic` | `TypeState(dynamic)`; `Never` → `TypeState(Never)` (`:1820-1844`) |
| 6 | `TypeAliasElement` | `TypeState(givenType ?? instanciação com os limites, ou dynamic)` (`:1826-1838`) |
| 7 | `TypeParameterElement` com `constructor-tearoffs` ligado (≥ 2.15) | `_lexicalTypeEnvironment[elemento]` → `TypeState(argumento)`; ausente → `CONST_TYPE_PARAMETER` @ `errorNode2` (`:1845-1859`). Sem a feature: cai para o fim |
| fim | `expression.staticType is InvalidType` | genérico @ `errorNode`, `isUnresolved: true` (`:1864-1866`) |
| fim | resto | genérico @ `errorNode2` (`:1870`) |

O genérico é `CONST_WITH_NON_CONSTANT_ARGUMENT` se o nó é argumento posicional direto de criação const, senão
`INVALID_CONSTANT` (§2.1). Getter de topo/estático **não sintético** (`int get x => 1`) é
`PropertyAccessorElement` cuja `variable2` é uma variável sintética não const → genérico (`c08`: `const c = x` →
`const_initialized_with_non_constant_value off=144 len=1`).

##### 2.5 `_evaluatePropertyAccess(alvo, identifier, errorNode)` (`:1686-1727`)

```
p = identifier.staticElement
p é getter ESTÁTICO                                   → null  (quem chamou segue para _getConstantValue)
p declarado em extensão                               → CONST_EVAL_EXTENSION_METHOD @ errorNode
p declarado em extension type                         → CONST_EVAL_EXTENSION_TYPE_METHOD @ errorNode
identifier.name == 'length' e alvo.type é dart:core String → stringLength(alvo)   (StringState: valor.length; desconhecida → int ?)
p é ExecutableElement estático (método estático)      → null
senão → CONST_EVAL_PROPERTY_ACCESS @ errorNode, args [identifier.name, alvo.type.getDisplayString()]
```
- O teste de `length` usa o **tipo do valor** (`targetResult.type`), não o estático, e vem **depois** do teste
  de extensão. Um argumento não resolvido vira objeto inválido cujo `type` é o tipo do parâmetro (§3.1): se o
  parâmetro é `String`, `o.length` chama `stringLength` sobre `NullState` → `assertString` →
  `CONST_EVAL_TYPE_STRING` (`s09`).
- `errorNode` é o `PrefixedIdentifier`/`PropertyAccess` **inteiro** (`o.f` len 3; `const A(1).f` len 12).
- Propriedade inexistente também dá `CONST_EVAL_PROPERTY_ACCESS` (o elemento é `null`, nenhum teste casa):
  `'a'.foo` → `const_eval_property_access` + `undefined_getter` (`c33`). Mas `A.undefinedThing` (prefixo é
  `InterfaceElement`: pula `_evaluatePropertyAccess`) cai em `_getConstantValue` → genérico não resolvido (`c30`:
  `const_initialized_with_non_constant_value off=274 len=16`).
- Em `PrefixedIdentifier` o prefixo é **avaliado primeiro** mesmo quando é uma classe (`A.k`: avalia `A` →
  `Type`, descarta, e vai a `_getConstantValue`); prefixo que não avalia (variável não const) devolve o erro
  **do prefixo** (`v.length` com `v` não const → genérico em `v`, não no nó).

##### 2.6 Deferred — `_getDeferredLibraryError(node, errorTarget)` (`:1878-1934`)

Sobe de `node` pelos pais; o **primeiro** ancestral que casa decide o código; o erro fica em `errorTarget` (o
identificador depois do prefixo; `name2` no `NamedType`):

| ancestral | código |
|---|---|
| `Annotation` | `INVALID_ANNOTATION_CONSTANT_VALUE_FROM_DEFERRED_LIBRARY` |
| `ConstantContextForExpressionImpl` (raiz de inicializador de elemento), `VariableDeclaration` | `CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE_FROM_DEFERRED_LIBRARY` |
| `DefaultFormalParameter` | `NON_CONSTANT_DEFAULT_VALUE_FROM_DEFERRED_LIBRARY` |
| `IfElement` cuja condição é o próprio `node` | `IF_ELEMENT_CONDITION_FROM_DEFERRED_LIBRARY` |
| `InstanceCreationExpression` | `CONST_CONSTRUCTOR_CONSTANT_FROM_DEFERRED_LIBRARY` |
| `ListLiteral` | `NON_CONSTANT_LIST_ELEMENT_FROM_DEFERRED_LIBRARY` (nome compartilhado `collection_element_from_deferred_library`) |
| `MapLiteralEntry` | chave → `NON_CONSTANT_MAP_KEY_…`; valor → `NON_CONSTANT_MAP_VALUE_…` |
| `RecordLiteral` / `SetOrMapLiteral` / `SpreadElement` | `NON_CONSTANT_RECORD_FIELD_…` / `SET_ELEMENT_…` / `SPREAD_EXPRESSION_…` |
| `SwitchCase` / `SwitchPatternCase` | `NON_CONSTANT_CASE_EXPRESSION_…` / `PATTERN_CONSTANT_…` |
| nenhum | `INVALID_CONSTANT` @ `node` |

Quem chama: `visitNamedType` (`node.isDeferred`, erro em `name2`), `visitPrefixedIdentifier` (prefixo de
importação com `node.isDeferred`, erro em `node.identifier`) e `visitPropertyAccess` quando o alvo é
`prefixo.Extensão`/`prefixo.ExtensionType` deferred (erro em `target.identifier`). Para `prefixo.Classe.membro`
com classe comum o alvo `prefixo.Classe` é avaliado por `visitPrefixedIdentifier` e o resultado é o mesmo: nos
dois casos o erro fica no nome **logo depois do prefixo**, não no membro final. **Corrige §D
`const_initialized_with_non_constant_value_from_deferred_library` ("no identificador final (`f` em `self.E.f`,
7:16)"):** a coluna 16 é o `E`. Conferido (`s21`): `const a = self.E.f;` → `off=94 len=1 3:16` (o `E`);
`const c = self.a;` → `off=140 len=1` (o `a`); `c18`: `const a = m.pi` → em `pi`; `[m.pi]` →
`collection_element_from_deferred_library` em `pi`; `([x = m.pi])` → `non_constant_default_value_from_deferred_library`.

##### 2.7 Coleções (`_buildListConstant` `:1360-1469`, `_buildSetConstant` `:1565-1647`, `_buildMapConstant` `:1477-1557`)

| elemento | lista / conjunto | mapa |
|---|---|---|
| expressão | avalia; erro volta | `EXPRESSION_IN_MAP` @ elemento |
| `ForElement` | `CONST_EVAL_FOR_ELEMENT` @ elemento | idem |
| `IfElement` | avalia a condição (erro volta); **desconhecida** → devolve a coleção inteira como desconhecida (`validWithUnknownValue`), sem olhar os ramos; `toBoolValue() == null` (não bool) → `NON_BOOL_CONDITION` @ condição; `true` → ramo `then`; `false` → `else` se houver. O ramo não tomado **não** é avaliado nem checado aqui | idem |
| `MapLiteralEntry` | `MAP_ENTRY_NOT_IN_MAP` @ elemento | avalia chave **e** valor (os dois, sempre); erro da chave tem prioridade; `map[k] = v` |
| `SpreadElement` | avalia; nulo com `...?` → pula; `toListValue() ?? toSetValue()` nulo → `CONST_SPREAD_EXPECTED_LIST_OR_SET` @ expressão | nulo com `...?` → pula; `toMapValue()` nulo → `CONST_SPREAD_EXPECTED_MAP` @ expressão |
| `NullAwareElement` (`?x`) | avalia; nulo → pula; senão reprocessa `[value]` e **retorna** o resultado (`:1440-1457`) | `EXPRESSION_IN_MAP` |

O tipo do literal é o do nó (`staticType`), sem `_substitution`. Conjunto e mapa são `Set`/`Map` de Dart sobre
`DartObjectImpl` (igualdade = tipo em tempo de execução igual **e** estado igual, `value.dart:278-284`):
duplicatas somem no valor; o relato de duplicata é do verificador (D.R4-2).

##### 2.8 Regra geral de propagação

1. Avaliação da esquerda para a direita, em profundidade; o **primeiro** `InvalidConstant` interrompe e sobe
   intacto (mesmo offset/código/argumentos), exceto: (a) direita de `&&`/`||` (reancorado no nó binário, §2.3);
   (b) fronteira de construtor (§3.4: `copyWithEntity` para o nó da criação, ou conversão em
   `CONST_EVAL_THROWS_EXCEPTION`); (c) leitura de variável const inválida (§2.4: vira `avoidReporting`);
   (d) argumento não resolvido de criação (§3.1: vira objeto inválido).
2. O `ConstantVisitor` sozinho dá **um** erro por expressão raiz. Vários erros numa mesma constante aparecem
   porque o verificador avalia também as sub-raízes (cada elemento de literal const, cada argumento de anotação,
   cada criação const) — `c33`: `const b = ['a'.foo, 1 ~/ 0]` → `const_eval_property_access off=40 len=7` e
   `const_eval_throws_idbze off=49 len=6` (um por elemento); `const e = (1 ~/ 0) + 'a'.foo` → só
   `const_eval_throws_idbze off=127 len=6` (a soma para na esquerda).
3. Valor **desconhecido** (`isUnknown`) nunca é erro: propaga como desconhecido do tipo do resultado (§4.1).
