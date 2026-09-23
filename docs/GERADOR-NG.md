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

Com diretivas de atributo (hoje o catálogo do `ngforms`: `NgForm`,
`NgModel`, `DefaultValueAccessor`, `RequiredValidator`), a ordem e o
`uniqueId` saem do `provider_parser.dart`:

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

O Sass cobre o que os projetos usam — aninhamento com `&`, `@use ... as *`,
variáveis, comentários `//`, `@media` — e normaliza valores como o
`sass_builder` (`0.5rem`→`.5rem`, `white`→`#fff`,
`transparent`→`rgba(0,0,0,0)`, escapes de string). O shim aplica
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
