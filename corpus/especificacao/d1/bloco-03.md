
#### 3. `_InstanceCreationEvaluator` (`evaluation.dart:2404-3195`)

Entradas: `evaluateAndFormatErrorsInConstructorCall` (`:335-374`; chamado por
`ConstantVisitor.visitInstanceCreationExpression` `:938-945`, pela anotação `:187-196` e pelo verificador
`constant_verifier.dart:265-271`) e `evaluateConstructorCall` (`:380-397`; chamado para `this(...)` `:2789-2796` e
para o super `:2975-2982` — **sem** formatação). As duas chamam `_InstanceCreationEvaluator.evaluate`.

##### 3.1 `evaluate` (`:3067-3164`) — fases, em ordem

```
1. !constructor.isConst → CONST_WITH_NON_CONST @ node.keyword (new/const escrito) senão @ node          (:3078-3088)
2. !declaration.isCycleFree → DartObjectImpl.validWithUnknownValue(lib.typeSystem, constructor.returnType) (:3090-3099)
3. para cada argumento i, na ordem escrita:                                                               (:3105-3134)
     nomeado:    tipoPadrão = argument.element?.type ?? InvalidType;  v = visitor._valueOf(argument.expression, tipoPadrão)
     posicional: tipoPadrão = i < parameters.length ? parameters[i].type : InvalidType   // i = índice na lista de argumentos
                 v = visitor._valueOf(argument, tipoPadrão)
     v inválido → devolve v (aborta; os argumentos seguintes não são avaliados)
4. invocation ??= ConstructorInvocation(constructor, posicionais, nomeados)
5. constructor = _followConstantRedirectionChain(constructor)    // factories const redirecionadas, com guarda de ciclo (:3172-3194)
6. _errorNode = configuration.errorNode(node)                    // constante de enum → a EnumConstantDeclaration
7. constructor.isFactory ? evaluateFactoryConstructorCall : evaluateGenerativeConstructorCall
```
`_valueOf(expr, tipoPadrão)` (`:2005-2026`): avalia; se o resultado é `InvalidConstant` com `isUnresolved`:
relata-o **no relator do visitante** (a menos de `avoidReporting`) e devolve
`DartObjectImpl(tipoPadrão, NullState(isInvalid: true))` (`_unresolvedObject`, `:497-506`) — a avaliação
**continua**. O relator do visitante só é o real quando quem avalia é o verificador em
`visitInstanceCreationExpression` (`constant_verifier.dart:263-264`); em `computeConstantValue`,
`_evaluateAndReportError` e nos `_initializerVisitor` ele é descartável (o `_externalErrorListener` de `:2415`
é um `BooleanErrorListener` que ninguém consulta). O código relatado é o do próprio erro, **sem** a troca por
código padrão: `const y = B(x)` com `x` indefinido → `const_with_non_constant_argument off=71 len=1` (genérico
de argumento posicional) além de `undefined_identifier` (`s09`).

##### 3.2 Factory (`evaluateFactoryConstructorCall`, `:2488-2542`)

| condição (nesta ordem) | resultado |
|---|---|
| nome `fromEnvironment` e `_checkFromEnvironmentArguments` falha | `CONST_EVAL_THROWS_EXCEPTION` @ `_errorNode`, sem contexto, sem marca de execução |
| `fromEnvironment` de `bool` | nome `'dart.library.js_util'` → bool **desconhecido**; senão variável declarada `'true'`/`'false'`; senão `defaultValue` passado; senão o padrão do parâmetro (`false`) |
| `fromEnvironment` de `int` / `String` | variável declarada (int: se `int.parse` aceita); senão `defaultValue` passado; senão o padrão do parâmetro (`0`, `''`); se esse padrão for `null` → valor desconhecido (`from_environment_evaluator.dart:55-129`) |
| `bool.hasEnvironment` | `BoolState(variável declarada != null)` (nunca valida os argumentos) |
| construtor sem nome de `Symbol` com 1 argumento | `_checkSymbolArguments` falha (não posicional, tipo ≠ `String`, valor desconhecido) → `CONST_EVAL_THROWS_EXCEPTION` @ `_errorNode`; senão `SymbolState(texto)` — **sem** validar o texto (`const Symbol('a b')` passa, `t19`) |
| qualquer outro factory (externo, redirecionamento para não-const ou não resolvido) | `validWithUnknownValue(definingType)`, sem erro |

`_checkFromEnvironmentArguments` (`:2661-2691`): 1 ou 2 argumentos; o 1º posicional e com `type == String`
(tipo do **valor**); o 2º, se houver, nomeado `defaultValue` com tipo do valor igual ao da classe
(`bool`/`int`/`String`) ou `Null`. No `dart analyze` não há `-D`: sem variáveis declaradas todo
`fromEnvironment` vale o padrão (**conhecido**: `int.fromEnvironment('x') ~/ 0` → `const_eval_throws_idbze`,
`c16`), e `bool.hasEnvironment` é `false`.

##### 3.3 Gerador (`evaluateGenerativeConstructorCall`, `:2544-2586`)

| fase | o que faz | falha → erro @ nó |
|---|---|---|
| A `_checkFields` (`:2619-2651`) | para cada campo da classe (ordem de `fields`) com `(isFinal \|\| isConst) && !isStatic` e que é `ConstFieldElementImpl` (tem inicializador): `v = field.evaluationResult`; `null` ou inválido → **pula em silêncio** (o campo fica fora do objeto); `tipo = FieldMember.from(field, returnType).type` (substituído); `!runtimeTypeMatch(v, tipo)` → | `CONST_CONSTRUCTOR_FIELD_TYPE_MISMATCH` @ `field.constantInitializer` (na declaração), args `[v.type, field.name, tipo]`, `isRuntimeException = hasTypeParameterReference(field.type)` — o tipo **declarado** |
| B `_checkTypeParameters` (`:3037-3049`) | `_typeParameterMap[Tᵢ] = _typeArguments[i]` se os comprimentos batem | — |
| C `_checkParameters` (`:2870-2952`) | para cada parâmetro `i`: nomeado → valor e nó (`NamedExpression`, **com o rótulo**) pelo nome; posicional → `_argumentValues[i]`, nó `arguments[i]`; sem argumento e opcional → `evaluationResult` do parâmetro (`null` → `Null`; inválido → parâmetro **ignorado**, fica fora do ambiente léxico); nó padrão = `_errorNode` | (1) `!v.isInvalid && !runtimeTypeMatch(v, parameter.type)` → `CONST_CONSTRUCTOR_PARAM_TYPE_MISMATCH` @ nó do argumento, args `[v.type, parameter.type]`, `isRuntimeException = nó é Expression && isAssignableTo(nó.staticType, parameter.type)`; (2) `this.x` com tipo do campo ≠ do parâmetro e valor não casa com o campo → mesmo código, args `[v.type, tipoDoCampo]`, **sem** marca de execução; (3) `this.x` cujo campo já está no mapa (tem inicializador) → `CONST_EVAL_THROWS_EXCEPTION` @ `_errorNode` |
| D `_checkInitializers` (`:2698-2865`), na ordem escrita | ver abaixo | |
| E `_checkSuperConstructorCall` (`:2962-3012`) | superclasse ≠ `Object`: `superclass.lookUpConstructor(superName, lib)`; não achou ou **não é const** → nada; senão `evaluateConstructorCall(lib, _errorNode, superclass.typeArguments, argumentos, superCtor, _initializerVisitor)`; valor → campo `(super)` | §3.4 |
| F extension type | devolve o valor do campo de representação (`:2573-2579`) | — |
| G objeto | `DartObjectImpl(definingType, GenericState(_fieldMap, invocation))` | — |

Fase D, por inicializador:
- `campo = expr`: avalia com `_initializerVisitor` (ambiente léxico = `_parameterMap`, tipos =
  `_typeParameterMap`, substituição da classe). Valor: campo já no mapa → `CONST_EVAL_THROWS_EXCEPTION` @
  `_errorNode` (`:2712-2717`); grava; `getter = definingType.getGetter(nome)`; se existe e
  `!runtimeTypeMatch(v, field.type)` → `CONST_CONSTRUCTOR_FIELD_TYPE_MISMATCH`, args `[v.type, nome, field.type]`,
  `isRuntimeException = isAssignableTo(expr.staticType, field.type)`, nó = **`expr` se de execução, senão
  `_errorNode`** (`:2731-2752`). Inválido: §3.4.
- `super(...)`/`super.n(...)`: guarda nome e argumentos, acrescenta os argumentos implícitos dos parâmetros
  `super.x` (identificadores sintéticos que o ambiente léxico resolve; posicionais inseridos **na frente**,
  nomeados no fim, `:2588-2614`).
- `this(...)`/`this.n(...)`: alvo resolvido e const → `evaluateConstructorCall(lib, _errorNode, _typeArguments,
  args, ConstructorMember.from(alvo, definingType), _initializerVisitor, invocation: _invocation)` e o resultado
  **é** o resultado final, sem contexto novo (`:2782-2799`); alvo não const/não resolvido → ignorado.
- `assert(c, m)`: avalia `c`; valor com `!isBool || toBoolValue() == false` → se `m` avalia para `String`
  conhecida: `CONST_EVAL_ASSERTION_FAILURE_WITH_MESSAGE [m]`, senão `CONST_EVAL_ASSERTION_FAILURE`; ambos @ o
  `AssertInitializer` inteiro, com `isRuntimeException: true` (`:2800-2832`). Bool **desconhecido** passa.
- Sem `super` escrito e com superclasse: argumentos = só os implícitos de `super.x` (`:2856-2859`).

Mixins não têm fase: campos declarados em mixin não são avaliados nem entram no objeto; a classe
`class C = S with M;` tem construtores sintéticos cujo `constantInitializers` é o `super(...)` de repasse
(`element.dart:670`) e é avaliada como gerador comum (`t17`).

##### 3.4 Onde o erro fica: declaração × uso, e as mensagens de contexto

Dentro do corpo do construtor todo erro nasce num nó **da declaração** (arquivo do construtor). A cada
fronteira de construtor ele é tratado assim:

| origem do erro | `isRuntimeException` | o que a fronteira faz |
|---|---|---|
| inicializador de campo (`campo = expr` inválido) | não | se não tem contexto: acrescenta `"The error is in the field initializer of '<ctor>', and occurs here."` no intervalo original; `copyWithEntity(erro, _errorNode)` → o erro passa a apontar para **o uso** com o **mesmo código e argumentos** (`:2755-2770`) |
| idem | sim | sobe intacto (`:2771-2773`) |
| condição de `assert` inválida | não | idem, com `"The error is in the assert initializer of '<ctor>', and occurs here."` (`:2833-2848`) |
| chamada ao super devolveu erro | não | sem contexto: `"The error is in the super constructor invocation of '<ctor>', and occurs here."`; **com** contexto: acrescenta `"The evaluated constructor '<super>' is called by '<ctor>' and '<ctor>' is defined here."` em `nameOffset`/`nameLength` do construtor chamador; `copyWithEntity(_errorNode)` (`:2986-3003`) |
| idem | sim | acrescenta a mensagem "The evaluated constructor…" e sobe intacto (`:3004-3007`) |
| `this(...)` devolveu erro | — | sobe intacto (o `_errorNode` é o mesmo) |
| desencontro de tipo de parâmetro/campo, `assert` falso, `~/ 0` | sim (por construção) | sobe intacto |

`<ctor>` = `_constructor.displayName`: no 3.6.2 o construtor sem nome aparece como `'A'` (o oráculo 3.13 mostra
`'A.new'`; as expectativas `[context n]` dentro dos arquivos do corpus são do main).

Na saída, `evaluateAndFormatErrorsInConstructorCall` (`:351-373`): erro com `isRuntimeException` →
**novo** `InvalidConstant.forEntity(configuration.errorNode(node), CONST_EVAL_THROWS_EXCEPTION, contextMessages:
[...os anteriores, "The exception is '<problemMessage formatada com os argumentos>' and occurs here." no
intervalo original])` — sem marca de execução. Resultado: toda falha "de execução" aparece como
`const_eval_throws_exception` no nó da criação **mais interna** que é `InstanceCreationExpression` (ou
anotação/constante de enum), e as demais como o código original reancorado no uso.

Conferido:
```dart
class A {
  final int x;
  const A(String s) : x = s.length ~/ 0;
}
class B extends A {
  const B() : super('abc');
}
class C extends B {
  const C();
}
const c = const C();
```
- `const_eval_throws_exception off=163 len=9 11:11 | Evaluation of this constant expression throws an exception.`
  - `ctx off=96 len=1 6:9 | The evaluated constructor 'A' is called by 'B' and 'B' is defined here.`
  - `ctx off=146 len=1 9:9 | The evaluated constructor 'B' is called by 'C' and 'C' is defined here.`
  - `ctx off=51 len=13 3:27 | The exception is 'Evaluation of this constant expression throws an IntegerDivisionByZeroException.' and occurs here.`

Construtor sintético de aplicação de mixin como chamador: contexto em `off=-1 len=0 1:0` (`t17`).
Criação aninhada como argumento: o erro fica na criação **interna** (`const A(const B(0))` com `assert` falso
em `B` → `const_eval_throws_exception off=93 len=10 3:19`, o `const B(0)`; `t15`).

#### 4. `DartObjectImpl` e estados (`value.dart`)

##### 4.1 Valor desconhecido e valor inválido

- **Desconhecido** (`isUnknown`): `BoolState(null)`, `IntState(null)`, `DoubleState(null)`, `StringState(null)`,
  `ListState.unknown`, `MapState.UNKNOWN`, `SetState.UNKNOWN`, `GenericState({}, isUnknown: true)`
  (`validWithUnknownValue`, `:206-233`). Fontes: `bool.fromEnvironment('dart.library.js_util')`, factory
  externo, construtor em ciclo, `identical(int, double)` (`:605-613`), ramo condicional com condição
  desconhecida, deslocamento com `bitLength > 31`, literal inteiro sem valor, interpolação/concatenação sem
  partes. Em toda operação aritmética, lógica, de comparação e de `toString` o desconhecido dá desconhecido do
  tipo do resultado, **depois** das checagens de tipo do outro operando (`IntState.add`: `assertNumOrNull(dir)`
  vem antes do teste `value == null`). `int ? ~/ 0` → int `?` (não IDBZE: o teste de zero vem depois do de
  desconhecido, `:2138-2150`). `assert(desconhecido)` passa; `if (desconhecido)` em coleção → coleção
  desconhecida; `desconhecido ? a : b` → avalia os dois ramos (erros valem) e dá desconhecido.
- **Inválido** (`isInvalid` = `state.isInvalid || hasInvalidType(type)`, `:262`): só o objeto de `_valueOf`
  (`NullState(isInvalid: true)` com o tipo do parâmetro). Efeitos: pula a checagem de tipo de parâmetro/campo
  formal (`evaluation.dart:2904`, `:2930`); `castToType` devolve o próprio objeto se ele ou o tipo é inválido
  (`:340-342`); no resto comporta-se como **`null`** (é um `NullState`): `isNull` verdadeiro, `== null`
  verdadeiro, aritmética → `CONST_EVAL_THROWS_EXCEPTION`, `.length` com tipo `String` → `CONST_EVAL_TYPE_STRING`.

##### 4.2 Igualdade

| estado | `hasPrimitiveEquality` | `isIdentical(dir)` (= `equalEqual` do estado) |
|---|---|---|
| `BoolState` | sim | bool × bool → comparação; algum `?` → `?`; outro estado → `false` |
| `IntState` | sim | int × int; int × double → `double == int.toDouble()`; `?` → `?`; outro → `false` |
| `DoubleState` | **não** (mas `==` aceita `DoubleState` à esquerda com `patterns`, `:498`) | NaN de qualquer lado → `false`; double × double → `identical(a, b)` (logo `0.0 == -0.0` avalia **falso**, sem erro); double × int → `identical(a, b.toDouble())`; `?` → `?` (`:1163-1188`) |
| `StringState`, `SymbolState` | sim | mesmo texto; `?` → `?` |
| `TypeState` | sim | `runtimeTypesEqual`; tipo `null` → `?` |
| `FunctionState` | sim | mesma `declaration`, mesmo `viaTypeAlias`, argumentos de tipo iguais par a par (`runtimeTypesEqual`); um com e outro sem argumentos → `false` (`:1392-1421`) |
| `ListState`, `SetState`, `MapState` | sim | algum desconhecido → `?`; senão igualdade estrutural (elemento a elemento; conjunto **na ordem de inserção**) |
| `RecordState` | todos os campos têm | estados diferentes → `false`; **iguais → `?`** (`:2838-2843`) |
| `GenericState` | o `==` concreto (`lookUpMethod2('==', concrete: true)`) vem de `Object` e, com `patterns`, o `hashCode` também (`:1496-1523`) | igualdade dos mapas de campos (inclui `(super)`) |
| `NullState` | sim | `dir is NullState` |

`isIdentical2` (`:603-628`): int×double ou double×int → bool `?` (contorno do `kIsWeb`); tipos de execução
diferentes → `false`; senão `state.isIdentical`. `DartObjectImpl ==` (usado em chaves de mapa/conjunto e na
igualdade estrutural) = `runtimeTypesEqual(type) && state ==` (`:278-284`).

##### 4.3 `as`, `is`, conversões

- `castToType(tipo)` (`:329-356`): o operando direito não é `TypeState` → `CONST_EVAL_TYPE_TYPE`; tipo `null`,
  objeto ou tipo inválidos, ou tipo alvo **com referência a parâmetro de tipo** → devolve o objeto;
  `!isSubtypeOf(obj.type, alvo)` → `CONST_EVAL_THROWS_EXCEPTION` (**sem** marca de execução) @ o `as` inteiro.
  O tipo alvo chega já com `_substitution` aplicada (`visitNamedType`, `evaluation.dart:1077-1079`): dentro de um
  construtor de `C<int>`, `x as List<T>` testa `List<int>`; `x as T` isolado vai por `_getConstantValue` →
  `_lexicalTypeEnvironment[T]`. Só resta parâmetro de tipo no alvo quando a criação foi feita sem argumentos
  conhecidos.
- `hasType(tipo)` (`:575-585`): `isSubtypeOf(obj.type, tipo)`; tipo `null` → `true`; `is!` nega. O tipo do
  objeto é o **de execução** (o do valor), com extension types apagados (`:189`).
- `convertToBool` (`:376-382`): bool → ele mesmo; `NullState` → `CONST_EVAL_THROWS_EXCEPTION`; qualquer outro →
  `false` (`InstanceState.convertToBool`, `:1690`). Só é chamado depois de `isBool` (condicional) — logo nunca
  lança ali.
- `performToString` (`:777-783`): bool/int/double/String/null → texto (`null` → `"null"`); tipo →
  `getDisplayString()`; função → nome; símbolo → texto; lista/mapa/conjunto/registro/objeto → `String`
  desconhecida. A interpolação só aceita bool/num/String/null (`visitInterpolationExpression`).
- `stringLength` (`:842-848`; estado `:1934-1937`, `:3040-3045`): `StringState` → `value.length` (unidades
  UTF-16) ou int `?`; outro estado → `CONST_EVAL_TYPE_STRING`.

##### 4.4 Onde a exceção vira `InvalidConstant`

`EvaluationException(errorCode, {isRuntimeException})` (`value.dart:1313-1322`) é capturada em cada método do
`DartObjectComputer` (`evaluation.dart:2031-2352`) e vira `InvalidConstant.forEntity(node, exception.errorCode)`
com `node` = o nó passado pelo visitante: a expressão binária/prefixa inteira; o `as`/`is` inteiro; a
`InterpolationExpression`; o `AdjacentStrings`/`StringInterpolation` inteiro na concatenação; o `errorNode` do
acesso a propriedade para `stringLength`; a condição em `applyBooleanConversion`. Só `integerDivide` repassa
`isRuntimeException` (`:2147-2157`); os demais a perdem — por isso `x % 0`, `1 << -1` ou um `as` que falha
dentro de construtor **não** viram "The exception is…": saem com contexto "The error is in the field
initializer…" (`s05`, `t07`).

#### 5. Relato (`ConstantVerifier`, só o que decide "nó × uso")

Três caminhos (`constant_verifier.dart`):

1. **`_reportError(erro, códigoPadrão)`** (`:634-747`), usado por `_evaluateAndReportError(expr, padrão)`
   (`:615-628`, visitante novo com relator descartável) e para os resultados guardados
   (`visitVariableDeclaration` `:494-510`, `visitEnumConstantDeclaration` `:212-216` com padrão `null`):
   ```
   erro.avoidReporting → nada
   erro.errorCode ∈ ESPECÍFICOS → relata o código do erro, com seus argumentos e contextos, em erro.offset/length
   senão, se padrão != null → relata o PADRÃO (sem argumentos, sem contextos) em erro.offset/length
   senão nada
   ```
   ESPECÍFICOS (`:645-726`): `CONST_EVAL_EXTENSION_METHOD`, `CONST_EVAL_EXTENSION_TYPE_METHOD`,
   `CONST_EVAL_FOR_ELEMENT`, `CONST_EVAL_METHOD_INVOCATION`, `CONST_EVAL_PROPERTY_ACCESS`,
   `CONST_EVAL_THROWS_EXCEPTION`, `CONST_EVAL_THROWS_IDBZE`, `CONST_EVAL_TYPE_BOOL_NUM_STRING`, `_BOOL`,
   `_BOOL_INT`, `_INT`, `_NUM`, `_NUM_STRING`, `_STRING`, `RECURSIVE_COMPILE_TIME_CONSTANT`,
   `CONST_CONSTRUCTOR_FIELD_TYPE_MISMATCH`, `CONST_CONSTRUCTOR_PARAM_TYPE_MISMATCH`, `CONST_TYPE_PARAMETER`,
   `CONST_WITH_TYPE_PARAMETERS_FUNCTION_TEAROFF`, `CONST_SPREAD_EXPECTED_LIST_OR_SET`,
   `CONST_SPREAD_EXPECTED_MAP`, `EXPRESSION_IN_MAP`, `VARIABLE_TYPE_MISMATCH`, `NON_BOOL_CONDITION`, os
   `…_FROM_DEFERRED_LIBRARY` (default value, map key, map value, set element, spread, case, annotation, if
   condition, const initialized, list element, record field, pattern constant) e os dois
   `WRONG_NUMBER_OF_TYPE_ARGUMENTS_(ANONYMOUS_)FUNCTION`.
   **Fora** da lista (logo trocados pelo padrão do ponto de uso): `INVALID_CONSTANT`,
   `CONST_WITH_NON_CONSTANT_ARGUMENT`, `CONST_WITH_NON_CONST`, `MISSING_CONST_IN_LIST/MAP/SET_LITERAL`,
   `MAP_ENTRY_NOT_IN_MAP`, `CONST_EVAL_TYPE_TYPE`, `CONST_EVAL_ASSERTION_FAILURE(_WITH_MESSAGE)`,
   `CONST_CONSTRUCTOR_CONSTANT_FROM_DEFERRED_LIBRARY`. `isUnresolved` **não** é consultado aqui: erro não
   resolvido é relatado como qualquer outro (`const k = undefinedTop` → padrão no identificador, `c30`).
   O relato usa a fonte do verificador (`_errorReporter.source`) com o offset do erro, qualquer que seja o
   arquivo em que o erro nasceu (efeito em erro de `_checkFields` vindo de outra unidade: **não verificado**).
2. **`visitInstanceCreationExpression`** (`:253-291`): para toda criação com `isConst` e construtor resolvido,
   reavalia com um visitante cujo relator é o **real** e relata o `InvalidConstant` **cru** (qualquer código,
   com contextos; só `avoidReporting` impede). Se deu erro, **não** desce nos argumentos; se deu valor, desce.
   É daqui que saem `const_with_non_constant_argument`, `invalid_constant` (argumento nomeado),
   `const_with_non_const` e os erros de construtor em criações que não são inicializador de `const`.
3. Relatos diretos do verificador (`_reportNotPotentialConstants`, `:749-762`: **todos** os nós não
   potencialmente constantes de inicializadores de construtor const, como `INVALID_CONSTANT`; `for` em literal;
   etc. — D.R4-2).

Padrões por ponto de uso: variável `const` → `CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE`; `final` de instância e
constante de enum → nenhum (só específicos); valor padrão → `NON_CONSTANT_DEFAULT_VALUE` (não avalia se o tipo
estático do padrão é `InvalidType`, `:825-826`, e **sobrescreve** `evaluationResult` do parâmetro com o resultado
sobre a AST real, `:831-833`); elemento de lista/conjunto/mapa → `NON_CONSTANT_LIST_ELEMENT` /
`NON_CONSTANT_SET_ELEMENT` / `NON_CONSTANT_MAP_KEY` / `_VALUE` (também para condição de `if` e expressão de
spread); campo de registro const → `NON_CONSTANT_RECORD_FIELD`; argumento de anotação e de constante de enum →
`CONST_WITH_NON_CONSTANT_ARGUMENT`; `case` (< 3.0) → `NON_CONSTANT_CASE_EXPRESSION`; padrão constante/relacional
→ os de padrões.

Duplicatas: os relatores guardam os erros num `Set` (`analyzer/lib/error/listener.dart:432`, `:452`); erros
iguais (mesmo código, offset, length, mensagem) vindos de dois caminhos saem uma vez. Erros **diferentes** no
mesmo intervalo saem os dois — é a cascata típica "código específico do caminho 2 + padrão do caminho 1":
```dart
var v = 1;
class A { const A(Object? x, {Object? n}); A.nc(); }
const a = const A(v);
const b = const A(1, n: v);
const c = A.nc();
const d = const A.nc();
const e = const A(const A(v));
var f = const A(v);
final g = const A(1 ~/ 0);
```
- `const_initialized_with_non_constant_value off=82 len=1 3:19` + `const_with_non_constant_argument off=82 len=1 3:19`
- `const_initialized_with_non_constant_value off=110 len=1 4:25` + `invalid_constant off=110 len=1 4:25` (nomeado: o genérico é `INVALID_CONSTANT`)
- `const_initialized_with_non_constant_value off=124 len=6 5:11` + `const_with_non_const off=124 len=6 5:11` (sem palavra-chave: nó inteiro)
- `const_initialized_with_non_constant_value off=142 len=5 6:11` + `const_with_non_const off=142 len=5 6:11` (a palavra `const`)
- `const_initialized_with_non_constant_value off=182 len=1 7:27` + `const_with_non_constant_argument off=182 len=1 7:27` (o `v` da criação interna)
- `const_with_non_constant_argument off=203 len=1 8:17` (só o caminho 2: `f` não é const)
- `const_eval_throws_exception off=217 len=15 9:11`, `ctx off=225 len=6 9:19 | The exception is 'Evaluation of this constant expression throws an IntegerDivisionByZeroException.' and occurs here.` (argumento que lança: convertido na criação)
