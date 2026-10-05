
#### E.3.4 `ModifierContext`: ordem, repetição, conflito e modificador que sobra

Três regras gerais, antes das tabelas:

- **M1 — só entra na recuperação.** Cada ponto de uso lê primeiro, sem erro, os modificadores na
  ordem canônica (o "caminho rápido"); o `ModifierContext` (`mc:63`) só é criado se **depois** disso o
  token seguinte ainda é modificador, e recebe os já lidos. Código válido nunca passa por ele; os
  `reportExtraneousModifier` do fim de cada `parse*Modifiers` só rodam nesse caso.
- **M2 — `isModifier(token)`** (`mc:12`): `const`/`final`/`var` sempre; os embutidos (`abstract`,
  `augment`, `covariant`, `external`, `late`, `required`, `static`) só se o token seguinte é palavra
  ou identificador que não `in`, ou um tipo record `(…)` seguido de identificador, `this.`/`super.`
  (com `?` opcional). Senão o embutido é **nome**: `class A { static }` é o campo `static` (`c161`).
- **M3 — o `parameterKind` trocado.** Em `parseFormalParameter` (`pi:1944-1951`) o caminho rápido
  consome `required` num parâmetro nomeado e muda `parameterKind` para `requiredNamed` **antes** de
  chamar `parseFormalParameterModifiers`, que testa `parameterKind != optionalNamed` (`mc:209`). Logo,
  sempre que o contexto é acionado num nomeado cujo `required` veio primeiro, o `required` é relatado
  como `EXTRANEOUS_MODIFIER` — além do erro que acionou o contexto. Conferido: `{required required
  int a}` (`c18`), `{required final covariant int a}` (`d10`), `{required covariant int a}` em função
  de topo (`d11`), `{required static int a}` (`d13`). `{required covariant final int a}` em método
  não aciona o contexto e não tem erro (`d12`).

`_parseModifiers` (`mc:325`) consome todo token que `isModifier` aceita (e `factory` repetido, se
`_afterFactory` → `DUPLICATED_MODIFIER`), despachando para `_parseX`. O token é sempre consumido; só é
**guardado** quando aceito. Todos os relatos vão **no token que está sendo lido**.

| lendo | aceito se | ao aceitar, relata (primeiro que casar) | se não aceito (primeiro que casar) |
|---|---|---|---|
| `abstract` (`mc:366`) | ainda não há | fora de ordem antes de `var/final/const`; antes de `covariant` | `DUPLICATED_MODIFIER` |
| `augment` (`mc:386`) | ainda não há | fora de ordem antes de `var/final/const`, `abstract`, `covariant`, `late`, `static`; com `external`: `CONFLICTING_MODIFIERS [augment, external]` | `DUPLICATED_MODIFIER` |
| `const` (`mc:418`) | sem `var/final/const` e sem `covariant` | depois de `factory`: fora de ordem antes de `'factory'`; com `late`: `CONFLICTING_MODIFIERS [const, late]` | já há `const` → `DUPLICATED_MODIFIER`; há `covariant` → `CONFLICTING_MODIFIERS [const, covariant]`; há `final` → `CONST_AND_FINAL`; há `var` → `CONFLICTING_MODIFIERS [const, var]` |
| `covariant` (`mc:448`) | sem `const`, `covariant`, `static`, fora de factory | fora de ordem antes de `var`; `final`; `late` | repetido → `DUPLICATED_MODIFIER`; depois de `factory` → `EXTRANEOUS_MODIFIER`; há `const` → `CONFLICTING_MODIFIERS [covariant, const]`; há `static` → `COVARIANT_AND_STATIC` |
| `external` (`mc:483`) | ainda não há | fora de ordem antes de `'factory'`; `const`; `static`; `late`; `var/final/const`; `covariant`; com `augment`: `CONFLICTING_MODIFIERS [external, augment]` | `DUPLICATED_MODIFIER` |
| `final` (`mc:513`) | sem `var/final/const`, fora de factory | — | repetido → `DUPLICATED_MODIFIER`; depois de `factory` → `EXTRANEOUS_MODIFIER`; há `const` → `CONST_AND_FINAL`; há `var` → `FINAL_AND_VAR` |
| `late` (`mc:539`) | ainda não há | com `const`: `CONFLICTING_MODIFIERS [late, const]`; fora de ordem antes de `var`; `final` | `DUPLICATED_MODIFIER` |
| `required` (`mc:561`) | ainda não há | fora de ordem antes de `const`; `covariant`; `final`; `var` | `DUPLICATED_MODIFIER` |
| `static` (`mc:585`) | sem `covariant`, `static`, fora de factory | fora de ordem antes de `const`; `final`; `var`; `late` | há `covariant` → `COVARIANT_AND_STATIC`; repetido → `DUPLICATED_MODIFIER`; depois de `factory` → `EXTRANEOUS_MODIFIER` |
| `var` (`mc:617`) | sem `var/final/const`, fora de factory | — | repetido → `DUPLICATED_MODIFIER`; depois de `factory` → `EXTRANEOUS_MODIFIER`; há `const` → `CONFLICTING_MODIFIERS [var, const]`; há `final` → `FINAL_AND_VAR` |

Mensagens: `MODIFIER_OUT_OF_ORDER` `The modifier '{0}' should be before the modifier '{1}'.` — `{0}` o
lexema lido, `{1}` o lexema do modificador **já guardado** (ou a palavra `factory`);
`CONFLICTING_MODIFIERS` `Members can't be declared to be both '{0}' and '{1}'.` — `{0}` o lido, `{1}`
o anterior (`const var` → `'var' and 'const'`, `c63`; `var const` → `'const' and 'var'`, `d103`);
`DUPLICATED_MODIFIER` `The modifier '{0}' was already specified.`; `EXTRANEOUS_MODIFIER` `Can't have
modifier '{0}' here.` (`{0}` = lexema). Ordem canônica que resulta: `external|augment` <
`abstract` < `static|covariant` < `late` < `var|final|const`; `required` antes de tudo o que cabe num
parâmetro.

O que cada contexto relata **depois** de `_parseModifiers` (✗ = `EXTRANEOUS_MODIFIER` no token; outro
nome = código próprio; ✓ = aceito; a ordem de emissão é a das chamadas dentro de cada método de
`mc:` — nos de palavra de topo, `const`, `external` e depois os demais em ordem alfabética):

| contexto (`mc:` linha) | abstract | augment | const | covariant | external | final | late | required | static | var |
|---|---|---|---|---|---|---|---|---|---|---|
| `parseClassModifiers` (122) | ✓ | ✓ | `CONST_CLASS` | ✗ | `EXTERNAL_CLASS` | ✓ | ✗ | ✗ | ✗ | ✗ |
| `parseEnumModifiers` (139) | ✗ | ✓ | ✗ | ✗ | `EXTERNAL_ENUM` | `FINAL_ENUM` (`pi:688`) | ✗ | ✗ | ✗ | ✗ |
| `parseMixinModifiers` (167) | ✗ | ✓ | ✗ | ✗ | ✗ | `FINAL_MIXIN` (`pi:766`) | ✗ | ✗ | ✗ | ✗ |
| `parseExtensionModifiers` (153) | ✗ | ✓ | ✗ | ✗ | ✗ | (não relatado) | ✗ | ✗ | ✗ | ✗ |
| `parseTypedefModifiers` (288) | ✗ | ✓ | ✗ | ✗ | `EXTERNAL_TYPEDEF` | ✗ | ✗ | ✗ | ✗ | ✗ |
| `parseTopLevelKeywordModifiers` (182; import/export/part) | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| `parseLibraryDirectiveModifiers` (251) | ✗ | ✓ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| `parseTopLevelMemberModifiers` (279) | ✗ | ✓ | ✓ | ✗ | ✓ | ✓ | ✓ | ✗ | ✗ | ✓ |
| `parseClassMemberModifiers` (198) | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✗ | ✓ | ✓ |
| `parseModifiersAfterFactory` (266) | `ABSTRACT_CLASS_MEMBER` | ✓ | fora de ordem | ✗ (em `_parse`) | fora de ordem | ✗ (em `_parse`) | ✗ | ✗ | ✗ (em `_parse`) | ✗ (em `_parse`) |
| `parseFormalParameterModifiers` (205) | ✗ | (não relatado) | ✗ | por `MemberKind` | ✗ | ✓ (¹) | ✗ | ✗ se não é `optionalNamed` (M3) | ✗ | ✓ (¹) |
| `parseVariableDeclarationModifiers` (303) | ✗ | ✗ | ✓ | ✗ | ✗ | ✓ | ✓ | ✗ | ✗ | ✓ |

(¹) em `MemberKind.GeneralizedFunctionType`, `var`/`final` → `FUNCTION_TYPED_PARAMETER_VAR`.
`covariant` em parâmetro (`mc:212-234`): `StaticMethod`/`TopLevelMethod` → `EXTRANEOUS_MODIFIER`;
`Extension(Non)StaticMethod` → `INVALID_USE_OF_COVARIANT_IN_EXTENSION` (modelo
`ExtraneousModifierInExtension`); `ExtensionType(Non)StaticMethod` →
`EXTRANEOUS_MODIFIER_IN_EXTENSION_TYPE`; `PrimaryConstructor` →
`EXTRANEOUS_MODIFIER_IN_PRIMARY_CONSTRUCTOR`; nos demais é aceito. Para esses seis `MemberKind` o
caminho rápido de `parseFormalParameter` **não** consome o `covariant` (`pi:1955-1965`), justamente
para que o contexto seja acionado.

Relatos de modificador **fora** do `ModifierContext` (valem também para código que não aciona M1):

| onde (`pi:`) | condição | código, token |
|---|---|---|
| topo, método (3619-3629) | `var` | `VAR_RETURN_TYPE` no `var` (`c88`) |
| | `final`/`const` | `EXTRANEOUS_MODIFIER` (`c86`, `d45`) |
| | senão `late` | `EXTRANEOUS_MODIFIER` (`c87`) |
| `parseMethod` (4823-4877) | `abstract` | `ABSTRACT_CLASS_MEMBER` (`c68`) |
| | `late` | `EXTRANEOUS_MODIFIER` (`c49`) |
| | `static` + operador | `STATIC_OPERATOR` no `static` (`c140`) |
| | sem `static`, `covariant`, e não é setter | `COVARIANT_MEMBER` (`c141`, `d62`) |
| | `const` + getter/setter | `EXTRANEOUS_MODIFIER` (`c53`) |
| | `var` | `VAR_RETURN_TYPE` (`c51`) |
| | `final` | `EXTRANEOUS_MODIFIER` (`c50`) |
| | `const` e não é construtor (5063) | `CONST_METHOD` (`c52`) |
| `parseFactoryMethod` (5123-5131) | `static`/`covariant` antes de `factory` | `EXTRANEOUS_MODIFIER` (`c54`) |
| | `var`/`final` antes de `factory` | `EXTRANEOUS_MODIFIER` |
| campo ou método (4783, 3634) | `get`/`set` sem ser acessor | `EXTRANEOUS_MODIFIER` no `get`/`set` (código morto: com `getOrSet` o ramo de método já foi tomado) |
| variável local (8121-8133) | `late` em `for (late …;;)` | `EXTRANEOUS_MODIFIER` |
| | função local com `var/final/const`, senão `late` | `EXTRANEOUS_MODIFIER` (`c77`) |
| AstBuilder `endFormalParameter` (`ab:1930`) | `var super.x` | `EXTRANEOUS_MODIFIER` no `var` |

Caminhos rápidos, por ponto de uso:

```
topo (pi:3435):        [external | augment]            [final | var | const | late [final]]
membro (pi:4494):      [external | augment | abstract] [static | covariant] [final | var | const¹ | late [final]]
parâmetro (pi:1944):   [required²] [covariant³] [var | final]⁴
local (pi:8023):       [var | final | const] | late [var | final]          (augment super → expressão)
factory (pi:5109):     depois de `factory`, se o seguinte não é referência de tipo válida →
                       parseModifiersAfterFactory com external, static|covariant, var|final|const já lidos
¹ só sem covariant   ² só em optionalNamed (M3)   ³ só nos MemberKind que o aceitam   ⁴ não em tipo de função
```

No topo há uma saída extra (`pi:3463-3468`): com `var/final/const` já lido e outro `final`/`var`/
`const` a seguir, o contexto **não** é acionado — o segundo abre a declaração seguinte. `final var x
= 1;` é o campo `final` sem nome (`missing_identifier` 6+3 no `var`, `expected_token ';'` em `final`)
mais `var x = 1;` (`c14`). Em membro e em local não há essa saída: `var final x` → `FINAL_AND_VAR`
(`c62`), `final final int x` → `DUPLICATED_MODIFIER` (`c180`).
