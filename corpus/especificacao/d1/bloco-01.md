### D.R4-1 Motor de avaliação: `ConstantEvaluationEngine`, `ConstantVisitor`, grafo e exceções (rodada 4)

Fonte: SDK 3.6.2, `analyzer/lib/src/dart/constant/{evaluation,compute,value,utilities,potentially_constant,
has_type_parameter_reference,has_invalid_type,from_environment_evaluator,constant_verifier}.dart`,
`_fe_analyzer_shared/lib/src/util/dependency_walker.dart`, `analyzer/lib/src/dart/analysis/library_analyzer.dart`,
`analyzer/lib/src/dart/element/element.dart`, `analyzer/lib/src/summary2/detach_nodes.dart`; linhas conferidas
nesta sessão. Exemplos rodados no oráculo vivo (`C:\tools\dartsdk-3.6.2`), arquivos em
`E:\dftemp\analise\spec-r4\casos\d1\` (`c01`–`c38`, `s01`–`s28`, `t01`–`t20`, `u01`, `v01`–`v04`; saídas
`out1.txt`…`out4.txt`). Formato dos diagnósticos: `código off=offset len=length linha:coluna | mensagem`;
`ctx` = mensagem de contexto. Perdas de `E:\dftemp\analise\trab\familia-D.txt`, amostras de `placar-r7.txt`.

Esta parte descreve o **motor** (quem avalia, em que ordem, que valor/erro sai de cada nó, em que nó o erro
fica). O verificador (`ConstantVerifier`: o que ele manda avaliar e as regras próprias dele) é de D.R4-2; aqui
só entra o necessário para explicar "erro no nó × no uso" (§5).

#### 1. Arquitetura e ordem

##### 1.1 Quando roda

```
LibraryAnalyzer._parseAndResolve                 (library_analyzer.dart:642-668)
  resolve diretivas; _resolveFile(todas as unidades)
  _computeConstants()                            (:667, corpo :282-297)
     constants = para cada unidade: _findConstants(unit)        (:571-586)
        = ConstantFinder(unit).constantsToCompute  ++  ConstantExpressionsDependenciesFinder(unit).dependencies
     computeConstants(declaredVariables, constants, featureSet, configuration)   (compute.dart:13-28)
LibraryAnalyzer._computeDiagnostics → _computeVerifyErrors(unidade)             (:301-304, :420-457)
  _computeConstantErrors: unit.accept(ConstantVerifier)                         (:272-279)   ← 1º verificador
  InheritanceOverrideVerifier; ErrorVerifier; FfiVerifier
```

- A avaliação de todos os alvos da biblioteca (todas as unidades, um único `ConstantEvaluationConfiguration`)
  acontece **antes** de qualquer verificador e **não relata nada**: cada `computeConstantValue` usa um
  `RecordingErrorListener` próprio que é descartado (`evaluation.dart:94`, `:109`, `:183`; o de `generateCycleError` em `:411`); o
  resultado (valor ou `InvalidConstant`) fica guardado no elemento. Quem relata é o `ConstantVerifier`, lendo
  `evaluationResult` (§5) ou reavaliando a expressão com o relator real.
- Não há limite de profundidade nem contador de recursão no motor (`evaluation.dart` não tem nenhum; busca por
  `depth|recursion` vazia). A terminação vem do grafo: construtor em ciclo não é avaliado (`isCycleFree`,
  `evaluation.dart:3090-3099`) e variável em ciclo já tem `InvalidConstant` guardado.

##### 1.2 Alvos de constante (`ConstantEvaluationTarget`) — `ConstantFinder` (`utilities.dart:107-210`)

| nó visitado | alvo(s) acrescentado(s) | linha |
|---|---|---|
| `Annotation` com `elementAnnotation != null` | o `ElementAnnotationImpl` (anotações em `part`/`part of`/constante de enum não têm) | `:123-135` |
| `ConstructorDeclaration` com `const` | o `ConstructorElement` **e todos os parâmetros** dele (com ou sem padrão) | `:155-164` |
| `DefaultFormalParameter` com `defaultValue != null` (qualquer função, método, closure, construtor) | o `ParameterElement` | `:167-173` |
| `EnumConstantDeclaration` | o `ConstFieldElementImpl`; registra `errorNode(inicializador sintético) = a declaração da constante` | `:176-186` |
| `VariableDeclaration` com inicializador e (`const` **ou** `final` de campo de instância dentro de `ClassDeclaration` cuja classe tem **algum** construtor `const`, inclusive factory — `treatFinalInstanceVarAsConst`, `:138-152`) | o elemento da variável (topo, estática, local, campo); registra `errorNode(inicializador do elemento) = inicializador da AST` | `:189-209` |

`treatFinalInstanceVarAsConst` só é ligado por `visitClassDeclaration`: campos `final` de **enum, mixin e
extension type** não entram como alvo por aqui (entram como dependência do construtor, §1.3, pois
`computeDependencies` usa `enclosingElement3.fields` sem olhar o tipo de declaração).

`ConstantExpressionsDependenciesFinder` (`utilities.dart:20-101`) acrescenta as **dependências** (via
`ReferenceFinder`) das expressões constantes que não são alvo: `ConstantPattern.expression`, criação `const`,
lista/registro `const`, chave de `MapPatternEntry`, operando de `RelationalPattern`, `SetOrMapLiteral` `const`
(e, quando **não** é const, cada elemento de mapa/conjunto — para a checagem de chaves únicas),
`SwitchCase.expression`. Assim, tudo o que uma expressão constante solta referencia já foi avaliado quando o
verificador a reavaliar.

##### 1.3 Dependências de cada alvo — `computeDependencies` (`evaluation.dart:223-326`)

| alvo | dependências relatadas ao grafo |
|---|---|
| constante de enum cujo enum se chama `values`, ou cuja constante tem o nome do enum | nenhuma (`:225-234`; erro já relatado em outro lugar) |
| `VariableElement` (const, final de instância, parâmetro com padrão, constante de enum) | `ReferenceFinder` sobre `constantInitializer` (`:241-246`) |
| construtor `const` que é factory redirecionando para construtor const (`getConstRedirectedConstructor`, `:444-470`; `Symbol` de `dart:core` é exceção: `null`) | só o construtor alvo (`:249-253`) |
| construtor `const` factory sem alvo const (externo, alvo não-const/não resolvido) | nenhuma (`:254-264`) |
| construtor `const` gerador | (a) `ReferenceFinder` sobre cada `constantInitializers` (`:266-273`); (b) se não há `super(...)`/`this(...)` escrito: o construtor **sem nome** da superclasse, se existe, é const e a superclasse não é `Object` (`:274-285`); (c) **todo** campo da classe com `(isFinal || isConst) && !isStatic && hasInitializer` (`:286-294`); (d) **todos** os parâmetros (`:295-297`) |
| construtor não `const` | nenhuma (o `if (constant.isConst)` de `:248` não tem `else`) |
| `ElementAnnotationImpl` | a variável do getter referenciado, ou o construtor; mais `ReferenceFinder` sobre os argumentos (`:299-317`) |

`ReferenceFinder` (`utilities.dart:214-273`) — o que conta como referência dentro de uma expressão:

| nó | dependência |
|---|---|
| `InstanceCreationExpression` com `isConst` | o construtor (`declaration`), **se** resolvido e const; depois desce nos filhos (argumentos) (`:225-233`) |
| `SimpleIdentifier` cujo elemento (ou a variável do getter) é `VariableElement` com `isConst` | a variável (`:255-263`). Um `final` de instância **não** é `isConst` → nunca é dependência por nome |
| `RedirectingConstructorInvocation` (`this(...)`) | o alvo, const ou não (`:245-252`) |
| `SuperConstructorInvocation` | o alvo, const ou não (`:266-272`) |
| `Label` (nome de argumento nomeado) | ignorado (`:236-242`) |

Não são dependência: tear-off de construtor, chamada de método, tipos. Uma criação **sem** `const` em contexto
constante conta (`isConst` da AST cobre o contexto).

##### 1.4 O caminhante — `DependencyWalker` (Tarjan) (`dependency_walker.dart:7-152`, `compute.dart:31-96`)

```
computeConstants: walker = _ConstantWalker(); para cada alvo na ordem da lista: walker.walk(walker._getNode(alvo))
  _getNode: nodeMap.putIfAbsent(alvo, …)          // um nó por alvo; _index/_lowLink do nó PERSISTEM entre walks
walk(start):                                       // dependency_walker.dart:24-122
  if start.isEvaluated: return
  index = 1                                        // o contador REINICIA a cada walk
  strongConnect(node):
     node._index = node._lowLink = index++; stack.add(node)
     para dep em getDependencies(node):            // memoizado no nó (:149-151)
        if dep.isEvaluated: continue
        if identical(node, dep): hasTrivialCycle = true
        else if dep._index == 0: strongConnect(dep); if dep._lowLink < node._lowLink: node._lowLink = dep._lowLink
        else if dep._index < node._lowLink: node._lowLink = dep._index
     if node._lowLink == node._index:
        if stack.last == node: stack.removeLast(); hasTrivialCycle ? evaluateScc([node]) : evaluate(node)
        else: scc = desempilha até node (inclusive); evaluateScc(scc)
evaluate(node)      → engine.computeConstantValue(node.constant)                       (compute.dart:60-62)
evaluateScc(scc)    → para cada nó: construtor → isCycleFree = false; generateCycleError(…, constant)  (:65-74)
isEvaluated(node)   = constant.isConstantEvaluated                                     (:37-38)
```

`isConstantEvaluated` por tipo de alvo: variável com `ConstVariableElement` → `_evaluationResult != null`
(`element.dart:1656`); construtor → campo `isConstantEvaluated`, ligado só por `computeConstantValue` de
construtor **const** (`evaluation.dart:155-162`, `element.dart:1331`); anotação → `evaluationResult != null`
(`element.dart:2085`); `VariableElementImpl` sem o mixin (variável não const, parâmetro sem padrão) → sempre
`true` (`element.dart:10241`, `:9066`).

Consequências (todas conferidas ao vivo):
1. **Um SCC = um relato por variável do SCC** (no nome) e um por construtor do SCC (em `returnType`), não um por
   aresta. Membros que só **dependem** do ciclo (fora do SCC) não recebem erro de ciclo (`c02`: `d = a` limpo).
2. Auto-referência direta (`const a = a;`, `static const s = [s];`, `const T() : this();`, parâmetro
   `[x = const S()]` do próprio `S`) é SCC trivial (`hasTrivialCycle`) → mesmo tratamento (`t01`, `s23`).
3. **Construtor de SCC nunca fica `isEvaluated`** (o `evaluateScc` não liga `isConstantEvaluated`) e guarda o
   `_index` do walk em que foi visto. Num walk posterior ele cai no ramo "já visto" (`dep._index != 0`) e
   compara o **índice velho** com o `_lowLink` novo. Se o índice velho é menor, o dependente não fecha o
   próprio SCC e é desempilhado junto com o ancestral como um SCC de vários nós → `evaluateScc` → **falso
   `RECURSIVE_COMPILE_TIME_CONSTANT`**. Reproduzido (`c27`):
   ```dart
   class C { const C() : this.a(); const C.a() : this(); }
   const y = z;
   const z = const C();
   ```
   - `recursive_constant_constructor off=16 len=1 1:17`, `off=38 len=1 1:39` (os dois `C`), mais
     `recursive_constructor_redirect` (ErrorVerifier) `off=22 len=8` e `off=46 len=6`
   - `recursive_compile_time_constant off=62 len=1 2:7` (`y`) e `off=75 len=1 3:7` (`z`) — nenhum dos dois está em
     ciclo: walk(`C`) deu `C._index=1`; walk(`y`): `y`=1, `z`=2, dep `C` não avaliado com índice velho 1 < 2 →
     `z._lowLink=1` → SCC `[z, y]`.
   Com as mesmas declarações em outra ordem (`s28`: `const w2 = w; const w = const R(); class R {…}` — a classe
   **depois**) o walk de `w2` é quem visita `R` pela primeira vez (índices 3 e 4) e `w`/`w2` saem limpos.
   Para paridade exata o DartForge precisa portar o caminhante literalmente (índices persistentes por nó,
   contador reiniciado por walk, ordem dos alvos = ordem do `ConstantFinder` por unidade, depois as dependências
   soltas, em `HashSet` — ordem deste último grupo **não verificada**).
4. A ordem dos alvos é a da AST (pré-ordem do `RecursiveAstVisitor`, com `super.visit…` **antes** do
   `add`: filhos primeiro) por unidade, unidades na ordem de `_libraryFiles`.

##### 1.5 `computeConstantValue` por tipo de alvo (`evaluation.dart:83-216`)

| alvo | o que faz | resultado guardado |
|---|---|---|
| parâmetro com padrão (`ParameterElementImpl` + `ConstVariableElement`) | `ConstantVisitor(lib, relator descartável).evaluateConstant(constantInitializer)` (`:90-105`) | `evaluationResult` (sem checagem de tipo); sem padrão: `Null` |
| variável com `constantInitializer` | avalia; se `DartObjectImpl` **e** `constant.isConst`: se `!runtimeTypeMatch(valor, constant.type)` e o tipo **estático** do inicializador é atribuível ao tipo declarado → `InvalidConstant.forEntity(inicializador, VARIABLE_TYPE_MISMATCH, [valor.type.getDisplayString(), constant.type.getDisplayString()])` (`:119-137`; se não é atribuível estaticamente, segue em silêncio — o erro estático já saiu); `DartObjectImpl.forVariable` (`:141`); constante de enum: `updateEnumConstant(index, name)` grava `index` e `_name` no mapa de campos (`:144-150`, `value.dart:987-1002`) | `evaluationResult` = valor ou o `InvalidConstant` da avaliação |
| `final` de instância (não `isConst`) | avalia, **sem** checagem de tipo (o tipo é conferido em `_checkFields` a cada criação) | idem |
| construtor const | nada; `isConstantEvaluated = true` (`:155-162`) | — |
| anotação | getter de variável: copia o `evaluationResult` da variável (`:166-179`); construtor const **com** lista de argumentos: `evaluateAndFormatErrorsInConstructorCall(lib, constNode, element.returnType.typeArguments, args, element, visitor)` e guarda também `additionalErrors` (`:180-197`); senão `null` | `evaluationResult` |

Memoização e avaliação sob demanda: `evaluationResult != null` ⇒ o nó é pulado para sempre. Fora do
`LibraryAnalyzer`, `ConstVariableElement.computeConstantValue()` (`element.dart:1662-1685`),
`ElementAnnotationImpl.computeConstantValue()` (`:2226-2240`) e `ConstructorElementImpl.computeConstantDependencies()`
(`:1521-1530`) chamam `computeConstants` só com `[this]` — é assim que constantes de **outras bibliotecas**
(lidas do resumo: `constantInitializer` vem de `linkedData.read`, `element.dart:1285-1288`, `:1622-1625`) são
avaliadas preguiçosamente; durante a análise de uma biblioteca elas entram como dependência no mesmo grafo
(o `callback` recebe o elemento, de qualquer biblioteca) e são avaliadas com o `ConstantVisitor` da biblioteca
**delas** (`constant.library`, `evaluation.dart:89`). O `FromEnvironmentEvaluator` usa
`computeConstantValue()` para o padrão de `defaultValue` (`from_environment_evaluator.dart:140-145`).

Parâmetro `super.x` sem padrão próprio: `evaluationResult` é o do parâmetro correspondente do construtor super
(`SuperFormalParameterElementImpl`, `element.dart:1750-1762`).

##### 1.6 Ciclo: o que cada membro recebe (`generateCycleError`, `evaluation.dart:406-440`)

| membro do SCC | efeito |
|---|---|
| variável | `evaluationResult = InvalidConstant.forElement(variável, RECURSIVE_COMPILE_TIME_CONSTANT)` — offset/length = `nameOffset`/`nameLength` do elemento (`value.dart:2435-2450`). O `errorReporter.atElement` de `:419-422` vai para um `RecordingErrorListener` local e é **descartado** |
| construtor | `isCycleFree = false` (`compute.dart:69-71`); nenhum resultado guardado. Toda criação por esse construtor devolve `validWithUnknownValue(tipo)` sem erro (`evaluation.dart:3090-3099`) |
| parâmetro, anotação | "should not happen" (`:432-438`): só log |

**Corrige §D `recursive_compile_time_constant` ("cada variável do SCC recebe o erro no próprio elemento
(`atElement`)"):** o relato visível não é o `atElement`; é o `ConstantVerifier` relatando o `InvalidConstant`
guardado — `visitVariableDeclaration` (`constant_verifier.dart:494-510`; para `final` de instância só se a classe
tem construtor **gerador** const, `:481-492`) e `visitEnumConstantDeclaration` (`:212-216`). Logo variável do SCC
**sem declaração na AST** (o `values` sintético do enum) não gera diagnóstico: `enum CR { e1(values); … }` dá um
único erro, em `e1` (`s19`).

Quem **lê** uma variável const cujo resultado é `InvalidConstant` (de ciclo ou qualquer outro) recebe
`InvalidConstant.forEntity(errorNode, INVALID_CONSTANT, isUnresolved: true, avoidReporting: true)`
(`evaluation.dart:1779-1784`): não relata nada no uso, e como argumento de criação vira objeto inválido do tipo
do parâmetro (`_valueOf`, §3.1).

##### 1.7 Expressões do elemento × da AST e o nó do erro

- Variáveis não locais, valores padrão, inicializadores de construtor e argumentos de anotação são avaliados
  sobre a **cópia destacada** guardada no elemento (`summary2/detach_nodes.dart:30-60`, `:89-115`, `:126-132`);
  a cópia mantém os offsets e os tipos resolvidos, mas a raiz tem como pai um
  `ConstantContextForExpressionImpl` (`:112`; é o que faz `inConstantContext` valer e o que
  `_getDeferredLibraryError` reconhece, `evaluation.dart:1886-1888`). Expressão que contém `ForElement`,
  função literal, `PatternAssignment` ou `SwitchExpression` é trocada **inteira** por um identificador marcador
  (já descrito em §D `const_initialized_with_non_constant_value`; confirmado em `c15`/`c34`: `const c = [for …]`
  → `const_initialized_with_non_constant_value off=77 len=31` no inicializador inteiro; `const f = [() {}]` →
  `off=99 len=7`; a mesma lista como **local** só dá `const_eval_for_element`).
- `ConstantEvaluationConfiguration.errorNode(node)` (`evaluation.dart:38-64`) traduz nó-do-elemento → nó-da-AST
  só para a **raiz** registrada pelo `ConstantFinder` (inicializador de variável; inicializador sintético da
  constante de enum → a `EnumConstantDeclaration`). É consultado em três lugares: `_getConstantValue`
  (`errorNode2`, `:1743`, usado só para `CONST_TYPE_PARAMETER` e para o genérico final), o nó da criação em
  `evaluateAndFormatErrorsInConstructorCall` (`:369`) e o `_errorNode` do `_InstanceCreationEvaluator` (`:3147`).
