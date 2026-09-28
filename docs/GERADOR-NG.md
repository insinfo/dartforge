# O gerador do ngdart — modelo e plano

Este documento é o que se tem de saber antes de mexer em
`crates/gerador_ng`. Ele guarda as **regras do compilador oficial** que
governam a saída, derivadas da leitura do `ngcompiler` e conferidas contra
a saída dele no corpus. Sem elas, a comparação byte a byte não fecha e o
trabalho vira tentativa e erro.

Oráculos: `corpus/ngdart/` (uma forma por arquivo, com o `.template.dart`
oficial ao lado) e os projetos reais (`new_sali/frontend`,
`references/limitless_ui/example`).

---

## 1. Arquitetura

```
pubspec.yaml ──► nome do pacote (fonte autoritativa; nome de pasta não serve)
      │
carga fase 1 ──► Program sem os gerados (tolerante) ──► Resolvedor
      │                                                     │
varredura ────► Achados por arquivo ──► Índice de componentes (todos os pacotes)
      │                                                     │
      └──────────────► emissão ◄───────────────────────────┘
                          │
              .template.dart + .css.shim.dart (em memória)
```

Duas regras de projeto:

1. **Recusar o que não se entende.** Saída errada é pior que saída
   faltando: ela compila e faz outra coisa. Cada forma não coberta vira um
   `Motivo`, o arquivo fica com o `build_runner` e o placar conta.
2. **A expressão do template é expressão Dart.** O parser da trilha nova a
   analisa; o módulo `expr` só reescreve a árvore com `_ctx.` e com os
   parênteses do emissor oficial. As 1.620 linhas do parser de expressões
   do `ngcompiler` não precisam ser portadas.

---

## 2. Numeração dos provedores (o `_5`, o `_8` e o `_9`)

Os campos de instância saem como `_<Tipo>_<nó>_<uniqueId>`. O `uniqueId` é
`_instances.length` no momento da criação
(`view_compiler/ir/provider_resolver.dart`), e o que entra antes está em
`compile_element.dart`, nesta ordem:

| # | quando | token |
|---|---|---|
| 0 | sempre | `ElementRef` |
| 1 | sempre | `Element` |
| 2 | sempre | `HtmlElement` |
| 3 | sempre | `Injector` |
| 4 | se o nó tem `ViewContainer` ou visão embutida | `ViewContainer` |
| 5 | se o nó tem `ViewContainer` | `ViewContainerRef` |
| — | sempre | `ChangeDetectorRef` |
| — | se há `appViewContainer` | `ComponentLoader` |
| — | em `<template>` | `TemplateRef` (inserido no início do array) |
| — | então | cada diretiva do nó, na ordem |

Conferido:

- elemento de componente simples: 4 embutidos + `ChangeDetectorRef` = 5 →
  o componente é **5** (`_A02TextoEstatico_0_5`);
- `<template>` com `*ngIf`: 4 + `ViewContainer` + `ViewContainerRef` +
  `ChangeDetectorRef` + `ComponentLoader` = 8 → `TemplateRef` é **8** e a
  diretiva **9** (`_TemplateRef_0_8`, `_NgIf_0_9`).

Consequência: acrescentar um `provider:` no elemento ou uma diretiva que
injeta `ViewContainerRef` **muda a numeração**. Por isso o gerador só emite
quando a conta é a conhecida, e recusa o resto.

### 2.1 Várias diretivas no nó (`diretivas.rs`)

Com diretivas de atributo — metadados lidos do programa por
`metadados.rs`, como o `_ComponentVisitor` do `find_components.dart`:
`allSupertypes` do analyzer ao contrário e a própria classe por último,
campos antes de setters nas entradas, acessores antes de campos nas saídas,
`@HostListener` num mapa por evento (o último vence na posição do
primeiro), ganchos por supertipo do `lifecycle_hooks.dart`, anotações
reconhecidas pela classe que designam —, a ordem e o `uniqueId` saem do
`provider_parser.dart`:

- `_ProviderResolver.resolve`: primeiro cada diretiva (ansiosa), na ordem
  de `directives:`; depois os `providers:` de cada uma, com o mesmo token
  multi acumulando (`NgValidators`, `NgValueAccessor`);
- `_getOrCreateLocalProvider`: busca em profundidade, as dependências antes
  de quem depende — por isso `<input required [(ngModel)]>` sai
  `_RequiredValidator_n_5`, `_NgValidators_n_6`, `_DefaultValueAccessor_n_7`,
  `_NgValueAccessor_n_8`, `_NgModel_n_9`;
- `addDirectiveProviders`: `ExistingProvider` de um provedor do próprio nó
  (não multi) é apelido — não tem campo, mas ocupa um número
  (`NgControl`, `ControlContainer`);
- as diretivas são ligadas (entradas, saídas, `registerDirective`) na ordem
  dos provedores (`transformedDirectiveAsts`); os `@HostListener` na ordem
  de `directives:` (`_collectHostListeners`), depois dos eventos do
  template;
- o `injectorGetInternal` lista os provedores `Visibility.all` e os
  apelidos, por nó, com o intervalo `[nó, nó + filhos]`
  (`ProviderForest`): `(n == nodeIndex)`, `(nodeIndex <= fim)` no topo,
  `((ini <= nodeIndex) && (nodeIndex <= fim))` no meio.

Dependência que o nó não satisfaz (`_getDependency`): `@Self` para no
nó; senão sobe pelos elementos acima — os provedores `Visibility.all` e os
apelidos —, na visão ou numa ancestral (`unsafeCast<ViewX0>((this.parentView!))._X`).
`@Optional` sem provedor é `null`; sem `@Host`, depois dos elementos viria o
injetor de fora (ainda recusado). Dois `@HostListener` do mesmo evento no
nó viram um `_handleEvent_N` que chama os dois, na ordem da primeira
aparição; o `ngOnDestroy` de diretiva entra depois dos filhos.

Consulta de conteúdo do filho (`@ContentChildren`) com resultado: as
instâncias criadas no conteúdo, em pré-ordem (`addQueryResult`); sem
`descendants`, só as de distância 1, contando os elementos com diretiva
entre o nó e o filho (`_getQueriesFor`). A tag que nenhum `<ng-content>`
recebe é criada e descartada, com as diretivas dela.

Injeção num componente filho (`injectFromViewParentInjector`): o
`parentView.injectorGet(T, parentIndex)` é escrito na visão do nó e levado
(`getPropertyInView`) à visão do nó mais alto da cadeia de injetores — que
sobe pelos pais e, na raiz de uma visão embutida, continua pelo pai da
âncora dela. Âncora na raiz da visão do componente: a expressão fica na
própria visão embutida; âncora aninhada: um `parentView` a mais.

---

## 3. Ordem de alocação dos imports

Os `importN` são numerados na ordem em que o emissor oficial os aloca, que
é a ordem em que ele **escreve o arquivo**. Errar isso desloca a numeração
inteira e nada bate. A ordem é:

1. a folha compilada (`<nome>.css.shim.dart`), quando há `styleUrls`;
2. `component_view.dart`;
3. o próprio arquivo (`x.dart`);
4. **os campos da classe**, na ordem em que aparecem:
   `text_binding.dart` (ligações de texto), depois, por nó em ordem de
   documento, o `.template.dart` e o `.dart` de cada componente filho ou
   `view_container.dart` + a diretiva estrutural, depois `dart:html` se
   algum elemento virar campo;
5. `style_encapsulation.dart`, `views/view.dart`,
   `change_detection_constants.dart`, `utilities.dart`, `dart:html`;
6. o corpo do `build()`: `dom_helpers.dart`, `template_ref.dart`,
   `devtools.dart`;
7. o `detectChangesInternal`: `interpolate.dart`, `check_binding.dart`;
8. `angular.dart` (sem prefixo);
9. as classes de visão embutida: `embedded_view.dart`, `render_view.dart`;
10. `host_view.dart`, `di/errors.dart` e os tipos injetados no construtor.

**Imports sem prefixo**: há uma lista fixa em
`output/dart_emitter.dart` (`_allowListedImports`) com a API pública do
ngdart — `angular.dart`, `dart:core`, `element_ref.dart`,
`view_container.dart`, `template_ref.dart`, `change_detection.dart`,
`ng_if.dart`, `app_view.dart`, `render/api.dart`. Por isso sai `NgIf(...)`
mas `import3.NgFor(...)`.

---

## 4. Seletores e casamento de diretivas

`selector.dart` do `ngcompiler`: um seletor é uma lista separada por
vírgula de `CssSelector { element, classNames, attrs, notSelectors }`,
lida por uma expressão regular que reconhece `:not(`, `tag`, `.classe`,
`[attr]`, `[attr=valor]` (com `~= |= ^= $= *=`), `)` e `,`.

Um elemento casa quando o `element`, as classes e os atributos do seletor
estão todos presentes. `NgFor` tem seletor `[ngFor][ngForOf]`: casa um
`<template>` que tenha os dois atributos — que é o que a microssintaxe
produz.

---

## 5. A microssintaxe de `*`

`expression/micro/parser.dart` do `ngast`. Em `*dir="..."`, o valor é uma
sequência separada por `;`:

| forma | efeito |
|---|---|
| expressão solta (só na primeira) | propriedade `dir` = expressão |
| `let x` | local `x` ligado a `$implicit` |
| `let x = y` | local `x` ligado a `locals['y']` |
| `nome: expr` | propriedade `dir` + `Nome` = expr |

Então `*ngIf="cond"` vira `<template [ngIf]="cond">`, e
`*ngFor="let item of itens"` vira
`<template ngFor let-item [ngForOf]="itens">`.

Na visão embutida os locais saem no topo do `detectChangesInternal`:

```dart
final local_item = importN.unsafeCast<String>(this.locals['$implicit']);
```

com o tipo do elemento da coleção — `List<String>` dá `String`.

---

## 6. Exceções que o oficial declara

- **`NgIf` recebe a entrada sem `checkBinding`.** É o `_isDirectBinding`
  de `semantic_analysis/binding_converter.dart`: a diretiva já compara o
  valor antes de agir. `NgFor` **tem** `checkBinding`, e ainda emite
  `ngDoCheck()` sob `!debugThrowIfChanged`, por implementar `DoCheck`.
- **Expressão imutável não vira ligação.** `isImmutable`
  (`analyzed_class.dart`): literal, `StaticRead`, e campo `final`/`const`.
  O texto é calculado uma vez no `build()`, e a ligação de propriedade sai
  sob `if (firstCheck)`, sem `checkBinding` e sem campo `_expr_`.
- **Três formas de atualizar texto**, escolhidas pelo tipo estático:
  `updateTextWithPrimitive` (primitivo mutável), `interpolateString0`
  (String), `interpolate0` (o resto).

---

## 7. Folhas de estilo

Duas etapas, as duas nossas:

```
x.scss --(nosso Sass)--> x.css --(nosso shim)--> x.css.shim.dart
```

O Sass é o crate `dartforge-sass` (o `grass` 0.13.4 trazido e corrigido
até o dart-sass 1.102.0: serializador, cores, cálculos, `if()` do CSS,
`BuildImporter` e `.css.map`). No limitless_ui, os 102 `.scss` saem iguais
byte a byte nos dois estilos, com o `.css.map`; na sass-spec, 95,7% do SCSS
`expanded` (as divergências restantes, por causa, estão em
`crates/sass/README.md`). O shim aplica
`._ngcontent-%ID%` a cada composto, `._nghost-%ID%` no `:host`, duplica o
seletor no `:host-context`, deixa o que vem depois de `::ng-deep` sem
escopo e passa `@keyframes` inteiro.

---

## 8. O que falta, em ordem

O placar atual (138 pendentes no `new_sali/frontend`, por motivo e
sub-forma) está no `ESTADO.md` §2.0. A tabela abaixo é a do começo, antes
dos planos 1 e 2, e fica como registro.

Números do `new_sali/frontend` (166 pendentes) e do
`limitless_ui/example` (61):

| forma | new_sali | limitless | o que exige |
|---|---|---|---|
| `*ngFor` e diretivas com seletor de atributo | 139 | 28 | índice de diretivas, microssintaxe, locais tipados |
| `@Input`/`@Output` restantes em filho | 71 | 54 | `@Output` (stream), `#ref` |
| interpolação fora do subconjunto | 109 | 58 | expressões com índice, `?.`, pipes |
| `providers:` no componente | 66 | — | numeração de provedores do §2 |
| `@ViewChild` | 27 | 17 | `compile_query.dart` |
| `pipes:` | 17 | — | `compile_pipe.dart` |
| `style` em linha | 34 | 4 | `updateStyle` por propriedade |
| `<ng-content select>` | 5 | — | índice de projeção por seletor |

A ordem de ataque segue a coluna "new_sali", que é o projeto maior, com um
desvio: `@ViewChild` e `providers:` mexem na **numeração de provedores**, e
por isso devem entrar junto com o modelo do §2 inteiro, não em cima dele.

---

## 9. Tabela de suporte por forma

Levantamento de 2026-09-26: as sondas `i01`…`i59` do `corpus/ngdart`
(uma forma por arquivo, com o `.template.dart` do `build_runner` oficial
em `oraculo/`) cobrem os padrões de template e de anotação de aplicações
reais. "Gerado" quer dizer **byte a byte igual ao oficial**, conferido
por `tests/corpus.rs`; "recusado", que o arquivo fica com o
`build_runner` e o motivo aparece no placar (e em `RECUSADOS`, que
confere o motivo). No fim da rodada: 159 arquivos conferidos, 0
diferentes, 8 recusados.

Rodada de 2026-09-27 (`providers:` e consultas de conteúdo no componente,
sondas `i60`…`i73`): 178 arquivos conferidos, 0 diferentes, 5 recusados
(b06, i24, i30 e as sondas de recusa i72 e i73).

Rodada seguinte (os cinco pendentes, sondas `i74`…`i85`): 192 arquivos
conferidos, 0 diferentes, 5 recusados — todos sondas de recusa que fixam a
saída oficial de formas ainda sem tradução: i76 (provedor do filho pedido
pelo conteúdo), i77 (filho que injeta provedor do próprio nó), i78
(provedor do filho com dependência de fora do nó), i84 (`<template>` com
diretiva) e i85 (`@ViewChild` de `<template>`). b06, i24, i30, i72 e i73
passaram a ser gerados.

Rodada dos itens NG04–NG10 da auditoria (sondas `i86`…`i99`, `j01`…`j10`,
oráculo regenerado por `scripts/corpus-ngdart.sh`, que faz o roteiro do
`.ps1` sem PowerShell e copia também os `.css.dart`): 219 arquivos
conferidos, 0 diferentes, 5 recusados — i76, i77, i78 (provedores do
filho, fora destes itens) e as sondas de recusa i94 (`#f="ngForm"` com
`ngControl`: o `@SkipSelf` do `NgControlName`) e i95 (`#ref` repetido ou
sombreado por `let`). i84 e i85 passaram a ser gerados.

Escopo de `#ref` por visão (NG06): cada visão resolve o nome pelo escopo
dela, como o `ViewNameResolver` do oficial (o mais próximo vence: um `let`
ou um `#ref` da embutida esconde o de fora). O `#ref` vira local da visão
que o declara uma vez só (os das embutidas são delas), quando alguma
expressão do escopo o lê; o de uma embutida que ninguém lê é só um nome
para o nó, como na visão de topo; a marca do nó leva a visão
(`nome@ViewX1`), e o de ancestral é lido pela cadeia de `parentView`. O
campo do nó lido de uma embutida é criado quando o binder do oficial liga
a embutida — depois das entradas do molde, entre os `_expr_k` já criados e
os seguintes — e o lido só na própria visão, no fim. i95 e j11 (o mesmo
nome em três níveis, o de fora lido duas visões abaixo, um só na
embutida, `let` que esconde o de fora num evento) iguais ao oficial byte a
byte. Depois, i94 (`NgControlName` com `@SkipSelf() ControlContainer`:
a dependência começa no pai, o `NgForm` do `<form>`), j12 (`@ViewChild`
de `#f="ngForm"`: a instância exportada, na forma estática) e j13 (`#ref`
com o nome de um membro: o local é o nó, tipado pelo membro, como o
`_TypeResolver`): 224 conferidos, 0 diferentes. O comportamento é o do oficial por
construção (o código gerado é o dele); o e2e em navegador segue sendo o
do limitless_ui no Pesado.

NG05 (j14–j16): `read: ElementRef` em consulta dinâmica, filho `onPush`
em `*` (o `View.queryChangeDetectorRefs` antes do `return` do
`mapNestedViews`) e consultas estáticas e dinâmicas no mesmo componente:
227 conferidos.

Placar do `limitless_ui/example` (2026-09-27, `dartforge build --comparar`
contra o `build_runner` oficial da revisão `9e38173`): 427 `.template.dart`
iguais, 103 recusados e **2 diferentes**, mais 1 `.css.shim.dart`
diferente. Os quatro defeitos achados viraram sondas (j18–j22, oráculo
oficial) e foram corrigidos:

- o `\` e a aspa simples da folha não eram escapados na string Dart do
  `.css.shim.dart` (`content:"\e939"` perdia o ícone: **comportamento**
  errado) — agora é o `escapeSingleQuoteString` do emissor (j22);
- o `REF` de um atributo sem valor seguido de quebra de linha incluía o
  espaço até o próximo atributo (j20);
- o nó de uma visão embutida que é resultado de `@ViewChild` dinâmico vira
  campo antes dos nós que só as ligações leem: o oficial o promove
  (`promoteToClassMember`) quando a visão do componente escreve o
  `mapNestedViews` (j18);
- a chamada com argumento nomeado sai quebrada como o `DartFormatter`
  (dart_style 2.3.8, página de 1.000.000 colunas) do builder a deixa: o
  emissor põe vírgula depois de cada nomeado e a vírgula final quebra a
  lista, um argumento por linha; dentro de `interpolateString0(..)` com a
  indentação de continuação (+4). Em outros lugares (operador, acesso,
  argumento de chamada sem nomeado, entrada direta do `NgIf`) é recusada
  (j21).

Depois: 429 `.template.dart` e 13 `.css.shim.dart` iguais, **0
diferentes**; 233 conferidos no corpus.

Segunda rodada no `limitless_ui/example` (sondas j23–j31, oráculo
oficial), atacando as recusas mais frequentes:

- **folhas por arquivo**: cada `.css` dá o `.css.dart` e o
  `.css.shim.dart` à parte (`gerar_folha`, como o `StylesheetCompiler`),
  e o componente não é mais recusado pelo conteúdo da folha — o template
  só importa o módulo. O motor fica só com as saídas que o plano espera
  (`generate_for` do `build.yaml`);
- `@ContentChild(ren)` do filho que acha componente `onPush`:
  `View.queryChangeDetectorRefs[x] = compView` antes da atribuição (a
  única só do primeiro) (j23, j24);
- membros `static` do componente lidos sem qualificação:
  `import1.Classe.nome`, `const`/`final` imutáveis, tipo `dynamic`,
  atribuição em evento (j25);
- `<template [ngTemplateOutlet]="t" [ngTemplateOutletValue]="v">`: a
  diretiva casada pela própria ligação é o `*ngTemplateOutlet="t; value:
  v"` (j27);
- `<template dir let-ctx>` com diretiva de `<template>` (a que recebe o
  `TemplateRef`) projetado num filho que a acha por `@ContentChild`: a
  âncora vai para a projeção (sem `hasViewContainer`), a diretiva é o
  provedor 8, o `let-` é local `dynamic` (j26, j28);
- diretiva sem `@HostBinding` e pipe no mesmo arquivo do componente não
  geram nada (j29);
- componente com `@HostBinding` que se provê como `ngValueAccessor`: o
  campo é a instância, sem `.instance` (j30, j31);
- o `detectHostChanges(firstCheck)` de um filho com `@HostBinding` sai
  com as ligações de propriedade, na ordem de documento
  (`bindDirectiveHostProps`), não junto do `detectChanges`;
- espaços: o `MinimizeWhitespaceVisitor` do ngast de cima para baixo,
  com o `_shouldCollapseWrapperNode` (`<template>`, `<ng-container>` e o
  `*` decidem pelo filho da ponta) e o recuo para `<pre>` e
  `@preserveWhitespace`;
- visibilidade de fase: o programa que o ngdart resolve não enxerga as
  saídas das fases do `build_runner` a partir da dele (o
  `messages.i18n.dart` do `i18n`, gravado em `lib/`, não existe para o
  resolvedor do ngdart), e membro de tipo que não existe no escopo
  (`InvalidType`) dá `dynamic`. O programa é um só para todos os pacotes:
  conta a primeira fase do ngdart.

O shim de CSS passou a ser o porte do `shadow_css.dart` sobre o
`csslib` 1.0.2, e o Sass o dart-sass 1.102.0 (crate `dartforge-sass`),
sem subconjunto. Resultado: 446 `.template.dart`, 104 `.css.dart` e 104
`.css.shim.dart` iguais ao oficial, **0 diferentes**; 248 conferidos no
corpus.

Depois: conteúdo passado a um filho que não projeta, ou que nenhum
`<ng-content>` recebe (`_maybeSkipNode`: o texto, ligado ou não, só consome
o índice do nó; o elemento e a âncora do `*` são criados soltos), texto e
interpolação projetados (`createText` do `dom_helpers`, `TextBinding.element`
na lista) — caso j37; provedor preguiçoso numa visão com ligação de texto (o
`ViewStorage` aloca o campo `late` ao construir a visão, antes de promover
nós e ligações de texto, então ele abre a classe e o import dele vem antes
do `text_binding.dart`) — caso j38; os campos `_viewQuery_*_isDirty` na
ordem em que o `ViewBuilder` acha o primeiro resultado dinâmico de cada
consulta (pré-ordem, `_setParentQueryAsDirty`), e `[attr.class]`/
`[className]` como o `ClassBinding` do `[class]` (`updateChildClass`) —
caso j39; dependência de diretiva que nenhum elemento da cadeia provê vem
do injetor de fora (`(v.parentView!).injectorGet(T, v.parentIndex)`,
`injectorGetOptional` com `@Optional()`, `v` pelo `nivel_do_topo`, a
criação no `debugInjectorWrap` sob `isDevMode`, dentro do `XNgCd` quando
há `@HostBinding`; dependência de `OpaqueToken`), e `Injector` é o
`this.injector(n)` do próprio elemento — caso j40; no mesmo template o
provedor local (`Visibility.local`, o padrão) é achado pelo nó de baixo, a
visibilidade só decide o `injectorGetInternal` (`Visibility.all` também na
hospedeira), e filho conhecido pelos metadados deixa de tornar a busca
incerta — caso j41; `@Attribute('x')` no construtor da diretiva (o
`RouterLink` do ngrouter) é o literal do atributo escrito no elemento, sem
decodificar entidades, ou `null` — caso j42; o índice do `<ng-content>` é o
ordinal no template inteiro, também dentro de visões embutidas — caso j43;
vários componentes com folha no mesmo arquivo e várias folhas em
`styleUrls` (cada import alocado quando a lista `styles$X` é escrita, o
repetido reaproveitado) — caso j44; diretiva com `@HostBinding` no arquivo
de componente (as classes `XNgCd` depois dos componentes, citadas sem
prefixo) e várias no arquivo só de diretivas, com `style.x` pelo tipo
escrito do campo — casos j45, j46; componente que injeta
`ViewContainerRef`: na hospedeira e no nó de quem o usa, o `ViewContainer`
nasce com o elemento (campo entre a visão e a instância, três embutidos a
mais que levam a instância ao `_8`, `detectChangesInNestedViews` depois de
`ngOnInit`/`ngDoCheck`, raiz e projeção pelo `_appEl_n`) — casos j47, j48;
diretiva de elemento comum que injeta `ViewContainerRef`: o mesmo
`ViewContainer` no nó, criado depois dos atributos e antes das diretivas
(que começam no `_8`) — caso j49. Outro embutido do elemento (`ElementRef`,
`TemplateRef` fora de `<template>`, `ComponentLoader`) e diretiva de
`<template>` com `ViewContainerRef`, entradas ou ganchos continuam
recusados, nunca do injetor de fora; parâmetro `super.x` no construtor
de diretiva (o tipo do parâmetro repassado no construtor da superclasse),
e diretiva que herda só `@HostListener` no arquivo de componente — caso
j50; nomes de `exports:` pelo import da biblioteca que os declara, tipo
`dynamic`, imutável a variável `const`/`final` e o campo estático
`const`/`final` (e o valor de enum), mutáveis getter, método, função e
cadeia além do campo — caso j51; `<template #t let-x>` lido em `*` aninhado
e em evento — caso j52. O `Gravador` do incremental passa a repassar
`tipo_inexistente`, `exportado` e `membro_estatico` (a regeração de um
arquivo sozinho dava outro texto que a do pacote). Template em aspas
triplas na anotação (a linha inicial em branco fora do valor, o `REF` com
as posições do `.dart`) — caso j53; `<template>` sem conteúdo é visão sem
raiz (`const <Object>[]`) — caso j54; referências de caractere no texto
como o `_unEscapeText` do ngast (decimal e hexadecimal de 2 a 4 dígitos,
a tabela `namedEntities` inteira, nome fora dela vira o próprio nome;
atributo fica cru) — caso j55. O programa da CLI passa a ter como raízes
todas as entradas das ações do ngdart (menos as partes): um componente que
nenhum arquivo importa também é gerado, como no `build_runner`.
`<ng-content>` na raiz de visão embutida e reprojetado no conteúdo de um
filho: a lista `this.projectedNodes[i]` entra inteira nas raízes e nas
listas do `createAndProject`, montadas pelo `createFlatArrayForProjectNodes`
(itens soltos num `<Object>[..]`, listas com `..addAll(unsafeCast(..))`) —
casos j56, j57. `class="a {{x}}"` e `value="{{i}}"` que alimentam `@Input`
de filho ou de diretiva (`interpolateString`/`interpolate`; primitiva crua
com `interpolate0` na ação; o elemento não repete o atributo consumido) —
casos j58, j59. Campo de elemento cujo `#ref` só é lido em handler de
evento entra na classe depois dos elementos ligados na detecção (o
`NodeReferenceStorageVisitor` o promove ao compilar o handler) — caso j60.
Filho que injeta um provedor que ele mesmo declara (`providers:
[ClassProvider(X)]` e `X` no construtor): o `_getOrCreateLocalProvider`
cria a dependência antes, e o filho fica com o índice seguinte — casos
i77, j61. Local de `*ngFor` sobre coleção de tipo genérico aninhado
(`List<List<X>>`, `List<Map<K, V>>`): o tipo do elemento vai inteiro e cada
nome dele é qualificado pelo import que o declara (`dart:core` sem
prefixo, mas com o import alocado) — caso j62. `template:` na anotação em
strings adjacentes, com escape ou cru: uma string só conta o `REF` do
conteúdo (`contentsOffset`), adjacentes contam do começo do nó, sempre com
as posições do valor decodificado — caso j63. `[attr.x]` vai por
`setAttribute` quando a fonte não pode ser nula (`canBeNull`: literal
primitivo, `a ?? b` com um lado assim) — caso j64. Ganchos de conteúdo e
de visão em diretiva (`ngAfterContentInit/Checked`, `ngAfterViewInit/Checked`)
e `ngOnDestroy` depois dos filhos do nó, diretiva por diretiva
(`bindDirectiveAfterChildrenCallbacks`), também no nó de um filho, depois
dos do componente — caso j65. O nó de um filho resolve as diretivas na
ordem de `directives:` (`_matchDirectives`), com os `providers:` do
componente antes dos das diretivas (`_ProviderResolver`), e cada provedor
ansioso depois das dependências: uma diretiva que vem antes do componente
em `directives:` é criada antes dele, com o `registerDirective` de todas
depois das instâncias, as entradas e as saídas dela antes das do
componente (os eventos do elemento primeiro) e os ganchos antes dos dele —
caso j67 (o `RequiredValidator` do `li-password-input`). O filho que
injeta um serviço provido por um elemento acima o lê de lá (o campo, ou
`.instance` do `XNgCd`, pela cadeia de `parentView` numa visão embutida),
sem `debugInjectorWrap` — caso j68. `@ViewChild` de `#ref="exportAs"` no
conteúdo projetado de um filho é estático como na própria visão — caso j69.
O `@HostListener` de diretiva no mesmo evento que o template escreve entra
no handler dele, depois da ação escrita (`mergeEvents` sobre as saídas do
elemento: os eventos do template primeiro, os das diretivas na ordem de
`directives:`) — caso j70. `providers:` de diretiva num nó de template de
qualquer forma (`ClassProvider`, `FactoryProvider`, `ValueProvider`, apelido
de token de fora): o que a diretiva injeta sai antes dela, o resto fica
preguiçoso (`late` com inicializador); a dependência que o nó não provê é
lida do elemento acima ou, sem ele, do injetor de fora da visão
(`(v.parentView!).injectorGet(T, v.parentIndex)`, com `debugInjectorWrap` e
campo `dynamic`) — casos j71 e i78 (este antes recusado). `@ContentChild(ren)`
de diretiva (em elemento comum ou no nó de um filho): lidos do programa com
o alvo resolvido na biblioteca da diretiva (setters, depois campos) e
escritos no `afterChildren` do nó como os de um componente
(`descendants:`, `read:`, lista vazia) — caso j72. Atributo vazio
(`x=""`) num `@Input` do filho liga `''`; sem valor (`<x ativo>`), `true` na
entrada `bool` (o tipo vem dos metadados do filho) e `''` nas outras — caso
j73. `style="..."` junto de `[style.x]` (o atributo no `build()`, a ligação na
detecção); `[class.x]`, `[style.x]`, `[attr.x]` e `[class]` no elemento de um
filho são ligações do elemento (`bindRenderInputs`, antes das das diretivas),
que vira campo `HtmlElement`; as variantes `NonHtml` fora do HTML, e o
`[class]` do elemento de um componente pela visão dele
(`this._compView_n.updateChildClassNonHtml`) — caso j74. Antes, um filho sem
`@Input` descartava essas ligações em silêncio. O `<template>` escrito com
diretiva leva a microssintaxe já decomposta (as ligações e os `let-` à
parte), sem voltar ao texto — `a == null ? null : b; value: c` nem passaria
no `isMicroExpression` —, e a microssintaxe do `*` separa as partes fora de
texto e parênteses (`;` e `:` de um literal ou de um ternário não contam) —
caso j75. `@ViewChild` de filho dentro de `*ngIf` (com `onPush`, o
`queryChangeDetectorRefs` no fecho) — caso j76 —, também quando o `*` está
no conteúdo projetado de outro filho: a âncora é desta visão, e a consulta
mapeia a embutida do mesmo jeito — caso j77. `<ng-container>` (também o
`<template [ngIf]>` reescrito) é transparente na busca dos resultados — caso
j78. `directives:` e as
listas constantes aceitam `...outraLista`. O `dirtyParentQueriesInternal`
segue o primeiro resultado de cada consulta na visão, em pré-ordem
(`_setParentQueryAsDirty` no `addQueryResult`), e os campos dos nós
consultados seguem as consultas — caso j66. Corpus: 310 conferidos.
limitless_ui/example (Linux, `build --comparar`, que agora lista cada
pendente com o motivo): **939 iguais / 11 pendentes / 0 diferentes** —
`.template.dart` 523/9/0, `.css` e `.css.map` do Sass 102/102 cada,
`.css.dart` e `.css.shim.dart` 104 cada.

### Diretivas estruturais

| forma | estado | casos |
|---|---|---|
| `*ngIf`, `*ngFor` (`let x of`, `index`, `first`/`last`/`even`/`odd`) | gerado | a09, a10, a25, i01, i03 |
| `*ngFor` com `trackBy: metodo` | gerado (método lido como valor, imutável: `if (firstCheck)` com `!= null`) | i02, i37 |
| `*ngFor` sobre coleção `dynamic` (índice, `??`) | gerado (local sem cast) | i31, i52 |
| `*ngIf="a ? b : c"` (`:` fora da microssintaxe) | gerado (`isMicroExpression`) | i43 |
| `*ngIf` com `else`/`then` | não existe no ngdart 8 (`NgIf` só tem `ngIf`) | — |
| `[ngSwitch]` + `*ngSwitchCase`/`*ngSwitchWhen`/`*ngSwitchDefault` | gerado (`@Host() NgSwitch` lido do nó acima) | i04, i42 |
| `<ng-container>` com e sem `*` (raízes de texto, interpolação, várias) | gerado | i09, i21, i39, i40 |
| `<template [ngIf]>`, `<template [ngSwitchCase]>`, `<template ngSwitchDefault>` | gerado (reescrito como `<ng-container *…>`) | i10, i53 |
| `<template #t>` sem diretiva (dentro de elemento, sem `#ref`, lido de visão aninhada) | gerado (âncora, `ViewContainer` fora da detecção, `TemplateRef` 7: campo com `#ref`, local sem) | i30, i82, i83 |
| `*ngTemplateOutlet="t"`, com `context:` | gerado (`NgTemplateOutlet(this._appEl_n)`, `ngDoCheck`) | i30, i82, i83 |
| `<ng-container *x>` vazio | gerado (`initRootNodesAndSubscriptions(unsafeCast(const <Object>[]), null)`) | i82 |
| `<template dir let-x let-y="k" [dirX]="e">` escrito (diretiva estrutural conhecida) | gerado (reescrito como o `*dir` equivalente; `REF` de cada ligação escrita; entradas na ordem de declaração da diretiva, `_orderingOf`) | i84, i86 |
| `<template>` com diretiva fora dessa forma (dois atributos, evento, `#ref`, `;` na expressão, `[dir]` sem atributo), `<template>` no conteúdo projetado | recusado (`<template> escrito no template`, `<template> no conteúdo projetado`) | — |
| entrada de `*` que a diretiva não declara | recusado (erro no oficial) | — |
| `@ViewChild('t') TemplateRef` de `<template #t>` na própria visão | gerado (`_ctx.x = this._TemplateRef_n_7;` no `build()`) | i85, i87 |
| `@ViewChild` de `<template>` em lista, em `*` ou em campo que não é `TemplateRef` | recusado | — |

### Ligações e eventos

| forma | estado | casos |
|---|---|---|
| `[prop]`, `[attr.x]`, `[class.x]`, `[class]`, `[style.x]` | gerado | a14, i28 |
| `[style.x.px]`/`[style.x.%]`, `[style.x]` que não é `String` | gerado (`visitStyleBinding`) | i05, i38 |
| `[innerHtml]`, `[href]`, `[src]`, `href="/p/{{id}}"` | gerado (`sanitizeHtml`/`Url`/`ResourceUrl` pela tabela do esquema) | i14, i57 |
| `[attr.x]` com contexto de segurança, `[style]` | gerado (saneador pelo nome de propriedade mapeado: `updateAttribute(el, 'href', sanitizeUrl(v))`, `setProperty(el, 'style', sanitizeStyle(v))`) | i92 |
| `bind-x`, `on-x` | gerado (antes viravam atributo: **saída errada**) | i36 |
| `(evento)` com `$event`, `(keyup.enter)`, atribuição | gerado | c04, c12, c14, i27 |
| `[(ngModel)]` | gerado | g01, h03 |
| `[(x)]` em componente filho (`@Input x` + `@Output xChange`) | gerado (desfeito como o `DesugarVisitor`) | i59 |
| `[ngClass]`, `[ngStyle]` (diretivas com `DoCheck`) | gerado | i11, i12, i41 |
| `?.`, `??`, ternário, getters | gerado | i13, i23, i34 |
| chamada com argumento nomeado (evento, `final currVal_k`, `interpolateString0`, aninhada em outra com nomeado) | gerado (quebrada como o `DartFormatter` do builder: um argumento por linha, vírgula final) | j21 |
| chamada com argumento nomeado dentro de operador, acesso, argumento de chamada sem nomeado ou entrada direta | recusado (indentação do formatador sem caso) | — |
| atributo sem valor que casa entrada de diretiva | gerado (`REF` só do nome) | j20 |

### Referências e consultas

| forma | estado | casos |
|---|---|---|
| `#ref` lido em expressão (elemento, filho, conteúdo projetado) | gerado (`final local_x = this._el_n;`, nó promovido a campo) | a15, i06, i07, i32, i48 |
| `#ref` lido numa visão embutida (dela ou de ancestral) | gerado (`unsafeCast<_ViewX1>((this.parentView!))._el_n`) | i44, i45 |
| `#d="x"` com a diretiva do nó de `exportAs: 'x'` | gerado (o local vale a instância, `this._X_n_m`; o elemento não vira campo por isso) | i93 |
| `#f="ngForm"` num `<form>` sem controles | gerado | j03 |
| `#f="ngForm"` com `ngControl` dentro | gerado (o `@SkipSelf` começa no pai) | i94 |
| `#ref` repetido em visões diferentes ou sombreado por `let` | gerado (escopo por visão, o mais próximo vence) | i95, j11 |
| `#ref` com membro de mesmo nome | gerado (o local é o nó, tipado pelo membro) | j13 |
| `@ViewChild` de `#ref` com valor, forma estática | gerado (a instância exportada) | j12 |
| `@ViewChild` de `#ref` com valor em `*`, lista ou repetido | recusado | — |
| `@ViewChild('ref')` de elemento, de filho, no conteúdo projetado | gerado | b03, b20, i46 |
| `@ViewChild` de `#ref` repetido (o primeiro) | gerado | i49 |
| `@ViewChildren('ref')` estático, sem resultado (`[]`) | gerado | i15, i49 |
| `@ViewChild(Tipo)`/`@ViewChildren(Tipo)` de componente filho | gerado | i16, i50 |
| `@ViewChild`/`@ViewChildren` com o resultado em `*ngIf`/`*ngFor`, em qualquer profundidade (`*` na raiz de `*`), elemento ou componente filho, por `#ref` ou por tipo, junto de consultas estáticas | gerado (`_viewQuery_x_N_isDirty`, `mapNestedViews` em cada nível e `mapNestedViewsWithSingleResult` no último, `dirtyParentQueriesInternal` subindo um `parentView` por nível) | i47, i51, i96, i97, j06, j09, j10, j15 |
| consulta dinâmica com `read: ElementRef`/`Element` | gerado (`return ElementRef(nestedView._el_n);`) | j14 |
| consulta dinâmica de filho `onPush` | gerado (`View.queryChangeDetectorRefs[nestedView._X_n] = nestedView._compView_n;` antes do `return`, como `_createAddQueryChangeDetectorRefs`) | j16 |
| `@ViewChild(.., read: ElementRef)`, `read: Element`/`HtmlElement`, e campo que não é `Element` (sem `read:`) de elemento estático | gerado (`ElementRef(_el_n)` ou o nó) | i98 |
| consulta com resultados em várias visões (`#ref` repetido em `*ngIf` diferentes, estático junto de `*`) | gerado (a árvore `_NestedQueryValues`: `[this._el_0, ...this._appEl_2.mapNested…]`, `firstOrNull([...])`, `.first` com estático; a única com o primeiro estático é estática) | j17, j34 |
| dois resultados na mesma visão embutida, resultado no conteúdo projetado ou em `<template>`; `read:` de outro token | recusado | — |
| `@ContentChild`/`@ContentChildren` no próprio componente (campo ou setter, `descendants:`, `read:`, por tipo ou `'ref'`) | gerado (na hospedeira, sem conteúdo: `this.component.x = [];` para cada lista logo depois da construção, setters antes dos campos; o único não recebe nada) | d09, h01, i68, i69 |
| consulta de conteúdo do componente que acharia o próprio nó (o componente, um provedor dele, tipo do ngdart) | recusado | — |
| `@ContentChild(.., read:)` de filho usado no template, `read:` do elemento (`HtmlElement`/`Element`) ou de outra diretiva do nó achado | gerado (o nó, local ou campo; o campo da diretiva) | i73, i81 |
| `@ContentChild` único de filho com resultado no conteúdo | gerado (o primeiro, em pré-ordem) | i73, i81 |
| `read:` de tipo do ngdart (`ViewContainerRef`, `TemplateRef`) ou de token que o nó achado não tem | recusado | — |

### Pipes

| forma | estado | casos |
|---|---|---|
| `$pipe.nome(x, args)` puro | gerado | c09, c17 |
| pipe dentro de pipe | gerado (o de dentro ganha proxy primeiro) | i08, i54 |
| pipe impuro e com `OnDestroy` (`$pipe.async`) | gerado (instância por chamada, `ngOnDestroy` no `destroyInternal`) | i29, i58 |
| `x \| nome` | não existe no ngdart 8 (erro do parser oficial) | — |

### Anotações da classe

| forma | estado | casos |
|---|---|---|
| `@Input`, `@Output`, ciclo de vida, `OnPush` | gerado | a16, b01…b14, d05…d07, i25 |
| `@HostListener` em componente | gerado | b04, b17, b21, i18 |
| `@HostBinding('class.x'/'attr.x')` em componente (campo, `final`, getter) | gerado (`detectHostChanges(firstCheck)`, chamado pela hospedeira e por quem usa o filho) | b05, i17, i55, i56 |
| `@HostBinding` de propriedade, `attr.x` com contexto de segurança, `style.x`/`style.x.unidade`, sem argumento, herdado (também só da base) em componente | gerado (as formas de `createElementPropertyAst` com o elemento `div`; herdados pelos metadados do programa e `Resolucao::membro_final`) | i89, i90, i91, j02 |
| `@HostBinding` de diretiva com `attr.x`, propriedade, sem argumento | gerado (no `XNgCd`) | j05 |
| `@HostBinding('class')`, `attr.x.if`, namespace, `style.x` em campo `final` ou de tipo desconhecido, `style.x` em diretiva | recusado | — |
| `@i18n`, `@i18n:attr`, `.meaning`, `.locale`, `.skip` (texto puro) | gerado (`static final String _message_N = Intl.message(..)`; antes: **saída errada**) | i20, i35 |
| `@i18n` com HTML dentro (tags aninhadas sem atributo, elemento vazio) | gerado (método `static String _message_N(String startTag0, ..)` com `name:`, `args:`, `examples:`; `createTrustedHtml` e `append`) | i99, j04 |
| `@i18n` num elemento com `*` | gerado (a mensagem fica na visão embutida) | j01 |
| `@i18n` em filho, com tag com atributo/ligação dentro, entidade HTML, junto de handler de evento na visão, ou com filho/diretiva na visão | recusado | — |
| `providers:` no componente: `ClassProvider`, `Provider(X, useClass:)`, classe solta, `ExistingProvider`/`useExisting:` (apelido local, apelido de apelido, do próprio componente, de token de fora), `ValueProvider`/`useValue:` (texto, inteiro, booleano, objeto `const`), `FactoryProvider`/`useFactory:` com `deps:` ou pelos parâmetros, `.forToken` de `OpaqueToken` (`T` do `dart:core`) e de `MultiToken` (`T` do `dart:core` ou genérico), listas aninhadas e constantes, sobrescrita de token | gerado (na hospedeira: preguiçoso `late T _X_0_n = ..;`, ou `late final` criado no `build()` antes do componente quando ele depende; campos com inicializador primeiro; `debugInjectorWrap` com dependência do injetor; `injectorGetInternal` do nó 0; o componente montado com `this._X_0_n`) | b02, h02, i19, i60…i67, i69…i71 |
| `multi: true` | não existe no ngdart 8 (multi é o `MultiToken`) | — |
| `providers:` com `useValue:` de lista, mapa, enum, `null` ou objeto aninhado; `useFactory:` de método estático; classe genérica ou abstrata; token de subclasse de `OpaqueToken`, `OpaqueToken` sem nome ou com `T` de fora do `dart:core`, `MultiToken` com `T` de fora do `dart:core` sem argumentos; dependência `@Self`/`@Host`/`@SkipSelf` ou de embutido (`ElementRef`, `Injector`…) num serviço; `viewProviders:` | recusado | — |
| componente com `providers:` (as formas da hospedeira) usado como filho no template, também dentro de `*` e recebendo conteúdo | gerado (no nó de quem usa: preguiçosos `late T _X_n_m = ..;` antes dos outros campos, dependências dos campos do nó; o filho primeiro no `injectorGetInternal` pelos apelidos dele) | i72, i74 |
| provedor do filho pedido por um nó do conteúdo | recusado (o oficial o cria no `build()`, logo depois do filho) | i76 |
| filho que injeta um provedor do próprio nó | gerado (o provedor ansioso sai antes do filho, que fica com o índice seguinte; a dependência dele não embrulha a criação em `debugInjectorWrap`) | i77, j61 |
| filho que injeta um provedor de um elemento acima | gerado (a leitura do provedor, não o injetor de fora) | j68 |
| provedor do filho com dependência de fora do nó, apelido de token que o nó não provê | recusado | i78 |
| `encapsulation: ViewEncapsulation.emulated`/`.none` sem folha de estilo | gerado (sem folha o oficial já desliga o encapsulamento) | b06 |
| `encapsulation: ViewEncapsulation.none` com `styleUrls` (`.css` escrito) | gerado (import do `.css.dart` sem shim, também gerado; `unscoped`; sem `addShimC`) | i88 |
| `styles: [..]` na anotação, emulado ou `none` | gerado (depois das folhas de `styleUrls`, com shim ou literal) | j07, j08 |
| `none` com folha Sass, folha ou `styles:` com `@import`, item de `styleUrls`/`styles` que não é texto literal, `encapsulation:` que não é `ViewEncapsulation.x` | recusado | — |
| vários `@Component` no mesmo arquivo (o que usa antes ou depois do usado) | gerado (uma tabela de imports, trechos na ordem do fonte, filho ao lado sem import nem prefixo) | i24, i79 |
| vários `@Component` no arquivo com folha de estilo | recusado | — |
| ligação em `template:` escrito na anotação | gerado (`REF` com o `asset:` do `.dart`, em UTF-16: do conteúdo numa string só — simples, crua, com escape ou de aspas triplas —, do começo do nó em strings adjacentes; as posições são as do valor); string com interpolação não é constante e não chega aqui | i24, i79, j53, j63 |

### O que falta, pela frequência

1. Provedores do filho no nó de template além do preguiçoso local: o
   pedido pelo conteúdo (ansioso, criado depois do filho — a saída está
   no oráculo do i76), a dependência de fora do nó (elementos acima e
   `parentView!.injectorGet(.., this.parentIndex)`, i78).

O oráculo das sondas se regenera como os outros casos
(`scripts/corpus-ngdart.ps1`): criar o `.dart` e o `.html` em
`corpus/ngdart/lib/src/` e rodar o script. Sem `pwsh`,
`scripts/corpus-ngdart.sh [--limpar]` faz o mesmo roteiro — gerar o
`web/main.dart` e o `lib/corpus_ngdart.dart`, `dart pub get --offline`,
`dart run build_runner build --delete-conflicting-outputs` e copiar os
`.template.dart`, `.css.shim.dart` e `.css.dart` de
`.dart_tool/build/generated/corpus_ngdart/lib/src/` para `oraculo/` — e dá
o mesmo oráculo (conferido: a regeneração não mudou nenhum oráculo antigo).
