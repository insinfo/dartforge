
#### E.3.5 Construtores: quem decide e o que cada um relata

O parser **não** decide construtor × método ao ver o nome: `parseMethod` (`pi:4807`) lê tudo (tipo,
nome com `.id` opcional, parâmetros de tipo, parâmetros, **inicializadores**, corpo) e só no fim
classifica (`pi:4990-5003`):

```
isConstructor = name.next == '.'                    // nome qualificado, qualquer que seja
             || beforeInitializers != null          // havia ':' depois dos parâmetros
             || (name.lexeme == nomeDaDeclaração && getOrSet == null)
se name.lexeme == nomeDaDeclaração e getOrSet != null:   // getter/setter com o nome da classe
     MEMBER_WITH_CLASS_NAME no nome; continua getter/setter (c47, c185, e49)
```

Antes da classificação já saíram, na ordem do código: os relatos de modificador de §E.3.4 (logo
`covariant A();` é `COVARIANT_MEMBER`, `final A();` é `EXTRANEOUS_MODIFIER` — `e41`, `e50`),
`MISSING_METHOD_PARAMETERS` (`A.foo;`), `EXTERNAL_METHOD_WITH_BODY` no começo do corpo se `external` e
o corpo não é `;` (`pi:4973`; vale para construtor: `c145`, `e16` — o 3.6.2 **não** usa
`EXTERNAL_CONSTRUCTOR_WITH_BODY`), `REDIRECTION_IN_NON_FACTORY_CONSTRUCTOR` no `=` (`pi:4978`, `c126`,
`c130`) ou o corpo por `parseFunctionBody` (`MISSING_FUNCTION_BODY`).

Se é construtor (`pi:5005-5029`), nesta ordem:

| # | condição | código | token |
|---|---|---|---|
| 1 | `name.lexeme != nomeDaDeclaração` | `INVALID_CONSTRUCTOR_NAME` | o 1º identificador (`e39`, `e38`) |
| 2 | `staticToken != null` | `STATIC_CONSTRUCTOR` | `static` |
| 3 | `getOrSet != null` | `GETTER_CONSTRUCTOR` / `SETTER_CONSTRUCTOR` | `get`/`set` (`e03`, `e04`) |
| 4 | `typeInfo != noType` | `CONSTRUCTOR_WITH_RETURN_TYPE` | `beforeType.next` — **só o 1º token** do tipo |
| 5 | havia `:` e `external` | `EXTERNAL_CONSTRUCTOR_WITH_INITIALIZER` | o `:` (`c13`, `c181`, `d63`, `e40`) |
| 6 | mixin / extension | `MIXIN_DECLARES_CONSTRUCTOR` / `EXTENSION_DECLARES_CONSTRUCTOR` | o nome (`c150`, `c151`) |

Se é método e sobrou `const`: `CONST_METHOD` (`pi:5063`). Extension e extension type, método com
corpo `;` sem `external`: `EXTENSION_DECLARES_ABSTRACT_MEMBER` no nome (extension type: modelo sem
índice, descartado pelo conversor; relata o `ErrorVerifier`, `e34`).

Eventos e AstBuilder: `endClassConstructor` (`ab:1196`) → `_buildConstructorDeclaration`
(`ab:5848`); `endClassMethod` (`ab:1302`) monta `MethodDeclaration`. O AstBuilder acrescenta, só para
construtor: parâmetros de tipo → `TYPE_PARAMETER_ON_CONSTRUCTOR` sobre `<…>` (`ab:5897`, `c149`);
`const` com corpo ≠ `;` → `CONST_CONSTRUCTOR_WITH_BODY` no 1º token do corpo (`ab:5902`: `{` em `c148`,
`=>` em `e12`); `external` → para **cada** parâmetro cujo `notDefault` é `FieldFormalParameter`,
`EXTERNAL_CONSTRUCTOR_WITH_FIELD_INITIALIZERS` no `this` (`ab:5911-5920`). Em `beginMethod`
(`ab:464-471`) o `static` é tirado dos modificadores quando o nome é o da classe e não há
`get`/`set`.

**Factory** — `parseFactoryMethod` (`pi:5104`). `factory` só abre factory se o token seguinte é
identificador ou modificador (`pi:4617-4631`); `factory() => …` é método chamado `factory`, `factory;`
é campo (`e17`). Depois:

```
se o token depois de `factory` não é referência de tipo válida → parseModifiersAfterFactory (§E.3.4)
static|covariant (antes de factory) → EXTRANEOUS_MODIFIER ; var|final → EXTRANEOUS_MODIFIER
nome: ensureIdentifier + ('.' id)? ; parâmetros de tipo ; parseFormalParametersRequiredOpt:
      sem '(' → MISSING_FUNCTION_PARAMETERS no token seguinte (c129)
async/async*/sync* → NON_SYNC_FACTORY no modificador (c189)
se next == '=':  external → EXTERNAL_FACTORY_REDIRECTION no '='           (pi:5146-5149)
                 parseRedirectingFactoryBody: referência de construtor + ensureSemicolon
senão se external: next != ';' → EXTERNAL_FACTORY_WITH_BODY em next ('{' ou '=>')   (pi:5152)
                   parseFunctionBody(allowAbstract = true)
senão: const (e next != native) → handleConstFactory → CONST_FACTORY no const (ab:3972)
       parseFunctionBody(allowAbstract = false) → `factory A();` é MISSING_FUNCTION_BODY no ';' (c128)
mixin/extension → MIXIN_/EXTENSION_DECLARES_CONSTRUCTOR no `factory` (e07)
```

A referência do redirecionamento (`parseConstructorReference`, `pi:5319`): identificador (`= ;` →
`MISSING_IDENTIFIER` no `;`, `c127`), `.id` opcional, argumentos de tipo, `.id` opcional. O que sobra
antes do `;` é `EXPECTED_TOKEN ';'` no último token da referência e vira o membro seguinte (`e09`).

**Inicializadores** — `parseInitializers` (`pi:3990`): entre dois inicializadores, sem vírgula, se o
seguinte é `assert(`, `this`/`super` + `(` ou `.`, ou identificador + `=`: `EXPECTED_TOKEN ','` **no
último token do inicializador anterior** e vírgula sintética (`c138`: no `1`). Qualquer outra coisa
encerra a lista.

**Quem relata corpo ausente.** Só o parser: `MISSING_FUNCTION_BODY` vem dos modelos `ExpectedBody` e
`ExpectedFunctionBody` (§E.3.0); o `ErrorVerifier` do 3.6.2 não tem esse código (busca em
`analyzer/lib` só o acha na tabela gerada e no conversor). Método de instância sem corpo em classe
concreta é outro código, do verificador (`concrete_class_with_abstract_member`, `c48`).

#### E.3.6 Extension types: construtor primário e representação

**3.6.2 — parser** (`parseExtensionTypeDeclaration`, `pi:3196`; não existe `parsePrimaryConstructor`
no 3.6.2, o trecho é embutido):

```
extension type [const] Nome <T>?                   // nome embutido → BUILT_IN_IDENTIFIER_IN_DECLARATION
se next ∈ { '(' , '.' }:
   '.': ensureIdentifier(primaryConstructorDeclaration)        // `E.(…)`: MISSING_IDENTIFIER no '(' (e22)
   '(' → parseFormalParameters(MemberKind.PrimaryConstructor)
   senão MISSING_PRIMARY_CONSTRUCTOR_PARAMETERS no último token (o nome depois do '.')   // c91
   endPrimaryConstructor(begin, constKeyword, hasConstructorName)
senão MISSING_PRIMARY_CONSTRUCTOR no último token (nome ou '>')  ; handleNoPrimaryConstructor   // c92, e23
implements L
se next != '{': parseExtensionTypeHeaderRecovery (§E.3.2: EXTENSION_TYPE_EXTENDS no `extends`,
                EXTENSION_TYPE_WITH no `with`, MULTIPLE_IMPLEMENTS_CLAUSES); ensureBlock → EXPECTED_EXTENSION_TYPE_BODY
corpo com DeclarationKind.ExtensionType
```

`extends`/`with` são relatados **um por cláusula, na ordem em que aparecem**, e as cláusulas são lidas
(`e29`: `with` depois `extends`; `e30`: `implements A, B extends C` → só `extends`). Os parâmetros do
construtor primário passam pelo `parseFormalParameter` comum com `MemberKind.PrimaryConstructor`:
`covariant` não é consumido no caminho rápido → `EXTRANEOUS_MODIFIER_IN_PRIMARY_CONSTRUCTOR`;
`required` posicional, `const`, `late`, `static`, `external`, `abstract` → `EXTRANEOUS_MODIFIER`
(`c104`, `c97`, `d28`, `d29`); `x = 0` posicional → `NAMED_PARAMETER_OUTSIDE_GROUP` no `=` (`d26`);
`var` com tipo → `VAR_AND_TYPE` (`c96`).

**3.6.2 — AstBuilder** (`endPrimaryConstructor`, `ab:2843`), com `first =
parameters.firstOrNull` e `abre = leftParenthesis`:

```
se first é SimpleFormalParameter:
   (a) first.type == null        → EXPECTED_REPRESENTATION_TYPE em abre.next        (ab:2871)
   (b) first.keyword ≠ null e ≠ const → REPRESENTATION_FIELD_MODIFIER no keyword     (ab:2886)
   (c) token depois de first é ',':
          1 parâmetro   → REPRESENTATION_FIELD_TRAILING_COMMA na vírgula             (ab:2898)
          mais de um    → MULTIPLE_REPRESENTATION_FIELDS na vírgula                  (ab:2904)
senão (lista vazia; 1º é this.x, super.x, parâmetro-função, opcional ou nomeado):
   EXPECTED_REPRESENTATION_FIELD em abre.next ; tipo e nome sintéticos                (ab:2911)
```

`abre.next` é o **primeiro token depois do `(`**, não o parâmetro: `@` da metadata (`c103`: 17+1),
`covariant` (`e28`: 17+9), `final` (`c102`), `[`/`{` (`c98`, `c99`), `int` de `int this.x` (`d22`), `)`
de `()` (`c19`). (a), (b) e (c) são independentes e acumulam (`e27`: os três). No ramo "senão" não
há relato de vírgula. Sem `(` depois de `.nome`, a lista é sintética (`_syntheticFormalParameterList`,
`ab:6105`, parênteses de largura 0 depois do nome do tipo) e o relato sai em `abre.next` = `)`
sintético: **length 0**, offset do token seguinte ao nome (`c91`: 16+0). Sem construtor primário
(`handleNoPrimaryConstructor`, `ab:5094`) não há `EXPECTED_REPRESENTATION_FIELD`: a representação
sintética é montada em `endExtensionTypeDeclaration` (`ab:1702-1718`) sem erro. Nome do campo igual ao
do tipo → `MEMBER_WITH_CLASS_NAME` no nome (`ab:1722`, `e25`).

**3.13.4** (fonte **[main]** `analyzer/lib/src/generated/error_verifier.dart:5597`,
`_checkForExtensionTypeRepresentationErrorCodes`, conferida contra o oráculo gravado de
`linguagem-3.13/extension_type/regress_53625_error_test.dart` e
`linguagem/primary_constructors/header/extension_type_error_test.dart`; não verificado em binário):
os mesmos códigos saíram do AstBuilder para o `ErrorVerifier`, com **retorno cedo** e posições novas.

| ordem | condição (sem `primary-constructors` ligado) | código | posição |
|---|---|---|---|
| 1 | lista vazia | `expected_representation_field` | `abre.next` (o `)`; length 0 se sintético) → **fim** |
| 2 | mais de um parâmetro | `multiple_representation_fields` | a vírgula depois do 1º → **fim** |
| 3 | 1º é `this.x` | `expected_representation_field` | o `this` → **fim** |
| 4 | 1º é `super.x` | `expected_representation_field` | o `super` → **fim** |
| 5 | 1º sem nome | `expected_representation_field` | o parâmetro → **fim** |
| 6 | (sempre) nome = nome do tipo / membro de `Object` | `member_with_class_name` / `extension_type_declares_member_of_object` | o nome |
| 7 | grupo `[…]`/`{…}` | `expected_representation_field` | o delimitador → **fim** |
| 8 | parâmetro-função | `expected_representation_field` | 1º token do parâmetro → **fim** |
| 9 | `final` ou `var` | `representation_field_modifier` | a palavra |
| 10 | sem tipo | `expected_representation_type` | **o nome** |
| 11 | vírgula depois do 1º | `representation_field_trailing_comma` | a vírgula |

Diferenças práticas contra o 3.6.2: (i) com dois ou mais parâmetros só sai
`multiple_representation_fields` (`ET3(final i, final x)`: 3.6.2 dá três relatos, `d38`; 3.13.4 um);
(ii) `expected_representation_type` vai no nome (`E03(var x)`: 3.13.4 em `x`, 38:24; 3.6.2 em `var`,
`d21`), inclusive quando o "nome" é `int` em `E00(int)` e `E26(@anno int)`; (iii)
`expected_representation_field` vai no `this`/`super`/delimitador; (iv) a mensagem de
`representation_field_modifier` é fixa `Representation fields can't have the modifier 'var'.`
([main] `am:23931`, `parameters: none`), até para `final` (oráculo gravado, `E02(final int x)` 32:20);
(v) sem construtor primário (`extension type E {`) há `missing_primary_constructor` no nome **e**
`expected_representation_field` length 0 no offset do token seguinte (oráculo gravado de
`AugmentationModifierExtra__extensionTyp_72995408.dart`: 3:24 e 3:26+0) — a lista sintética agora
existe e cai na linha 1; (vi) `E13(int x = 0)`: o 3.6.2 dá `expected_representation_field` em `int`
além de `named_parameter_outside_group` (`d26`); o oráculo gravado do 3.13.4 só o segundo (105:26).
Com `primary-constructors` ligado só restam as linhas 1–6 e `var` na 9
([main] `:5657-5667`).

`const_primary_constructor_with_body` **não existe no 3.6.2** (nem em `fm:` nem em `am:` nem no
gerado; `this` não abre membro: `d31`). É do `ErrorVerifier` posterior ([main]
`error_verifier.dart:8941`, `_validateConstructorBodyAllowed`), ver o código em §E.3.7.
