
#### E.3.2 Cabeçalhos de classe, mixin, enum, extension e extension type

**Classe** — `parseClassOrNamedMixinApplication` (`pi:2625`): nome
(`ensureIdentifier(classOrMixinOrExtensionDeclaration)`), parâmetros de tipo (variância permitida),
conflitos de modificadores de classe, e a bifurcação `=` (aplicação de mixin) × `parseClass`.

- `abstract` + `sealed` → `ABSTRACT_SEALED_CLASS` no `sealed` (`pi:2645`, `c167`); `abstract final base`
  → `ABSTRACT_FINAL_BASE_CLASS` de `final` até `base` (`reportRecoverableErrorWithEnd`, `pi:2648`;
  `c168`: offset 9, length 10); `abstract final interface` → `ABSTRACT_FINAL_INTERFACE_CLASS` idem.
- `class C = S with M implements I;` (`parseNamedMixinApplication`, `pi:2686`): termina em
  `ensureSemicolon`. Um `{}` depois (`class C = S with M {}`) é `expected_token ';'` no último token
  do cabeçalho e `expected_executable` no `{` (o bloco inteiro é consumido calado) — `d35`.

```
parseClass(token, begin, classKeyword, nome)                   // pi:2715
  start = token
  token = parseClassHeaderOpt(token)        // extends T | with L | implements L | native 'x'  (ordem fixa)
  se token.next != '{':
     token = parseClassHeaderRecovery(start)                    // relê do começo
     ensureBlock(token, BlockKind.classDeclaration)             // EXPECTED_CLASS_BODY no último token
  parseClassOrMixinOrExtensionBody(token, DeclarationKind.Class, nome)

parseDeclarationHeaderRecoveryInternal(token, kind)            // pi:2757 (Class | ExtensionType)
  relê o cabeçalho com um listener que só anota (hasExtends, hasWith, hasImplements); nada relatado
  repete até token.next == '{' ou não haver progresso:
    skipUnexpectedTokenOpt(token, [extends, with, implements, '{']):
        se token.next NÃO é palavra-chave e token.next.next é um dos esperados:
           UNEXPECTED_TOKEN em token.next; pula-o                              // c111
    se token.next.lexeme ∈ {extend, on}: EXPECTED_INSTEAD ['extends'] nele; lê o tipo   // c110, d93
    senão parseClassExtendsOpt
    se leu extends:  Class: já havia → MULTIPLE_EXTENDS_CLAUSES ; havia with → WITH_BEFORE_EXTENDS ;
                            havia implements → IMPLEMENTS_BEFORE_EXTENDS       (no `extends`)
                     ExtensionType: EXTENSION_TYPE_EXTENDS (no `extends`)
    parseClassWithClauseOpt
    se leu with:     Class: já havia → MULTIPLE_WITH_CLAUSES ; havia implements → IMPLEMENTS_BEFORE_WITH
                     ExtensionType: EXTENSION_TYPE_WITH                        (no `with`)
    parseClassOrMixinOrEnumImplementsOpt
    se leu implements e já havia → MULTIPLE_IMPLEMENTS_CLAUSES (no `implements`)
```

`extends A, B` é `MULTIPLE_EXTENDS_CLAUSES` **na vírgula** e todos os tipos são lidos
(`parseClassExtendsSeenExtendsClause`, `pi:2880`; `c112`). Tipo que falta depois de `extends`/`with`/
`implements`/`on` é `EXPECTED_TYPE_NAME` no token seguinte, com identificador sintético (`c07`, `c25`,
`c106`): não há erro de corpo, o `{` está lá.

**Mixin** — `parseMixin` (`pi:2939`): `on L`, `implements L`; recuperação
`parseMixinHeaderRecovery` (`pi:2969`) com a mesma forma: `skipUnexpectedTokenOpt([on, implements,
'{'])`; `extend`/`extends` → `EXPECTED_INSTEAD ['on']` (`c113`); segundo `on` → `MULTIPLE_ON_CLAUSES`
(`d82`); `on` depois de `implements` → `IMPLEMENTS_BEFORE_ON` no `on` (`d81`); segundo `implements` →
`MULTIPLE_IMPLEMENTS_CLAUSES`; `with` → `MIXIN_WITH_CLAUSE` no `with`, lista lida (`d83`). Depois
`ensureBlock(BlockKind.mixinDeclaration)`.

**Enum** — `parseEnum` (`pi:2367`) e `parseEnumHeaderOpt` (`pi:2439`): parâmetros de tipo; tokens
soltos antes das cláusulas → `recoverySmallLookAheadSkipTokens` (`pi:2541`: até 3 tokens antes de
`{`/`with`/`implements`; um token → `UNEXPECTED_TOKEN`, mais → `UNEXPECTED_TOKENS` do primeiro ao
último, `d90`); `with`; outro `with` → `MULTIPLE_CLAUSES ['enum','with']` (`d88`); `implements`;
`with` depois de `implements` → `OUT_OF_ORDER_CLAUSES ['with','implements']` (`d89`); outro
`implements` → `MULTIPLE_CLAUSES ['enum','implements']`. Sem `{`: `ensureBlock(enumDeclaration)` →
`MISSING_ENUM_BODY` **no token seguinte** (EOF em `c24`). Constantes: entre duas, sem vírgula e o
seguinte é identificador → `EXPECTED_TOKEN ','` nele (`c118`); outro token → `EXPECTED_TOKEN '}'` nele
e salta para o `}` casado. Depois do `;`, o laço de membros com `DeclarationKind.Enum`.

**Extension** — `parseExtensionDeclaration` (`pi:3108`): nome opcional (identificador que não é
`on`), parâmetros de tipo, `on`. Sem `on`: `extends`/`implements`/`with` no lugar → `EXPECTED_INSTEAD
['on']` neles; senão `EXPECTED_TOKEN 'on'` **no último token lido** e `on` sintético (`c116`). Depois
do tipo, sem `{`: cada `,`, `extends`, `implements`, `on`, `with` → `UNEXPECTED_TOKEN` e pula a palavra
e um identificador depois dela (`pi:3159-3177`; `d84`, `d85`); então
`ensureBlock(extensionDeclaration)`.

**Extension type** — §E.3.6.

Modificadores de classe (`sealed`, `base`, `interface`, `macro`, `mixin`) não passam pelo
`ModifierContext`: são identificadores reconhecidos em `parseTopLevelDeclarationImpl` (`pi:576-613`),
**um só** e imediatamente antes de `class`/`mixin`/`enum`. `base abstract class`, `sealed sealed
class`, `interface base class` não casam: viram membro de topo (campo `base`/`sealed`… sem tipo).

Corpo que falta — conferido no oráculo:

| entrada | diagnóstico |
|---|---|
| `class A` (`c06`) | `expected_body` 6+1 em `A`: "A class declaration must have a body, even if it is empty." |
| `class A extends B⏎int x = 0;` (`d94`) | `expected_body` 16+1 em `B`; `int x = 0;` vira declaração de topo |
| `mixin M` (`c114`) | `expected_body` 6+1: "A mixin declaration must have a body…" |
| `extension E on int` (`c115`) | `expected_body` 15+3 em `int`: "An extension declaration must have a body…" |
| `extension type E(int i)` (`c105`) | `expected_body` 22+1 no `)`: "An extension type declaration must have a body…" |
| `enum E` (`c24`) | `missing_enum_body` 7+0 no EOF (+ `enum_without_constants` do verificador) |

#### E.3.3 Laço de membros e recuperação

`parseClassOrMixinOrExtensionBody` (`pi:4376`): enquanto `token.next` não é `}` nem EOF, chama
`parseClassOrMixinOrExtensionOrEnumMemberImpl` (`pi:4471`). Não há teste de progresso no laço: cada
ramo da recuperação garante o seu.

```
membro(token, kind, nomeDaDeclaração)
  beforeStart = token = parseMetadataStar(token)
  caminho rápido de modificadores (isModifier de mc:12):
     [external | augment | abstract]  [static | covariant]
     [final | var | const (só sem covariant) | late [final]]
     se ainda há modificador → ModifierContext.parseClassMemberModifiers (§E.3.4)
  com var/final/const e padrão externo + '=' → PATTERN_VARIABLE_DECLARATION_OUTSIDE_FUNCTION_OR_METHOD; campo
  typeInfo = computeType(...); next = token depois do tipo
  se next NÃO é IDENTIFIER puro:
    get | set:  seguido de identificador → getOrSet
                seguido de palavra reservada + (; = ( { => <) → getOrSet, nome recuperado   // pi:4609
                senão `get`/`set` é o nome
    factory:    seguido de identificador ou modificador →
                   tipo antes → TYPE_BEFORE_FACTORY no último token do tipo
                   abstract   → ABSTRACT_CLASS_MEMBER
                   parseFactoryMethod                                                      // §E.3.5
                senão `factory` é o nome
    operator:   seguido de operador declarável e sem <…>( → parseMethod (operador)
                seguido de ===, !== ou operador não declarável (≠ '=' e '<') → parseInvalidOperatorDeclaration
                seguido de `unary -` → parseMethod (UNEXPECTED_TOKEN em `unary`, c122)
                senão `operator` é o nome (c119, c120, d65)
    não identificador (ou `typedef` + identificador como 1º token):
                abstract → ABSTRACT_CLASS_MEMBER; → recoverFromInvalidMember
  senão se sem tipo e sem var/final/const:
    nome seguido de operador declarável (sem grupo) e depois ( { => → parseInvalidOperatorDeclaration
    nome seguido de palavra reservada e depois (; = ( { => <) → relê: o nome é o TIPO, a reservada o nome
  depois = token após o nome
  se getOrSet != null ou depois ∈ { ( , { , < , . , => } → parseMethod                     // pi:4760
  senão → parseFields
```

`recoverFromInvalidMember` (`pi:9325`), com `next = token.next` (o que devia ser o nome):

| # | condição | efeito | tokens consumidos |
|---|---|---|---|
| 1 | `class` | `CLASS_IN_CLASS` no `class`; se vem identificador e `{…}`, pula até o `}` | a declaração inteira (`c123`) |
| 2 | `enum` | `ENUM_IN_CLASS`; idem | idem (`c124`) |
| 3 | `typedef` | `TYPEDEF_IN_CLASS` | só o `typedef`; o resto é lido como membro (`c125`) |
| 4 | operador (sem grupo) | `parseInvalidOperatorDeclaration`: `MISSING_KEYWORD_OPERATOR` no operador, insere `operator` sintético, `parseMethod` | o membro (`c05`; `c121` chega pelo ramo "nome seguido de operador") |
| 5 | `getOrSet != null`, ou `next` ∈ `(`, `=>`, `{` | `parseMethod` com o token como "nome": `ensureIdentifier` → `MISSING_IDENTIFIER` nele; depois `MISSING_METHOD_PARAMETERS` se não é `(` | o membro (`d56`) |
| 6 | `token == beforeStart` (nada lido antes) | `EXPECTED_CLASS_MEMBER` em `next`; `handleInvalidMember`; se `next` ≠ `}`, pula-o | **1** (`c37`, `c38`, `c39`), **0** se é `}` (`c163`) |
| 7 | senão (havia modificador ou tipo) | `parseFields` com `next` como nome → `MISSING_IDENTIFIER`/`EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD`, depois `EXPECTED_TOKEN ';'` | o campo (`c162`, `d33` linha 6) |

`handleInvalidMember` (`ab:4602`) só descarta a metadata; nenhum nó entra na árvore.
`parseInvalidOperatorDeclaration` (`pi:9253`): o relato vai no operador (ou no token seguinte ao
nome, se o nome não é operador e o seguinte é), e o tipo é recalculado depois de inserir a palavra.

Decisão campo × método dentro de classe — casos mínimos conferidos:

| entrada | leitura | diagnósticos |
|---|---|---|
| `class A { int }` (`c03`) | campo `int` sem tipo | `missing_const_final_var_or_type` 10+3, `expected_token ';'` 10+3 |
| `class A { foo }` (`c04`) | idem | idem em `foo` |
| `class A { m; }` (`c36`) | campo | `missing_const_final_var_or_type` em `m` |
| `class A { int x }` (`c142`) | campo, `;` falta | `expected_token ';'` em `x` (token anterior) |
| `class A { f g h; }` (`c165`) | campo `g` de tipo `f`, depois campo `h` | `expected_token ';'` em `g`; `missing_const_final_var_or_type` em `h` |
| `class A { void m {} }` (`c34`) | método sem `(` | `missing_method_parameters` em `m` |
| `class A { static m => 0; }` (`c35`) | idem | `missing_method_parameters` em `m` |
| `class A { A.foo; }` (`d58`) | `.` → método → construtor | `missing_method_parameters` em `A` (nome = 1º identificador) |
| `class A { int A.foo; }` (`d59`) | idem com tipo | `constructor_with_return_type` em `int`; `missing_method_parameters` em `A` |
| `class A { => 0; }` (`d56`) | linha 5 da tabela | `missing_identifier` e `missing_method_parameters` no `=>` |
| `class A { + }` (`c05`) | linha 4 | `missing_keyword_operator`, `missing_method_parameters` no `+`; `missing_function_body` no `}` |
| `class A { ; }` (`c37`) | linha 6 | `expected_class_member` no `;` |
| `class A { int x = 0;; }` (`d114`) | linha 6 | `expected_class_member` no 2º `;` |
| `class A { @deprecated }` (`c163`) | linha 6, `}` não consumido | `expected_class_member` no `}` |
| `class A { final }` (`c162`) | linha 7 | `missing_identifier` no `}`, `expected_token ';'` em `final` |
| `class A { operator }` (`c119`) | campo `operator` | `missing_const_final_var_or_type`, `expected_token ';'` |
| `class A { get }` (`c160`), `{ static }` (`c161`) | campo `get`/`static` (não são modificador: `isModifier` exige palavra depois) | idem |
| `class A { get int x; }` (`c90`) | getter `int` sem corpo, depois campo `x` | ver abaixo |

`c90` (`get int x;`): `get` + identificador → getter chamado `int`; `parseFunctionBody` encontra `x`
(nem `;` nem `{`) → `ensureBlock` → `missing_function_body` em `x` (18+1); o membro seguinte é `x;` →
`missing_const_final_var_or_type` em `x`. No topo (`get int x => 0;`, `c89`) o caminho é outro:
`x` é palavra seguida de `=>` → `unexpected_token` em `x` e o corpo é lido.

`recoverFromStackOverflow` (`pi:9416`): chamado só por `parseStatement` quando `statementDepth`
passa de 500 (`pi:5578-5582`) — não por declarações nem membros; `STACK_OVERFLOW` no token
seguinte, `;` sintético (comando vazio), e anda até o próximo `}` ou EOF. Não verificado no oráculo
vivo (não há amostra).
