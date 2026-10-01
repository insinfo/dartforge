# O compilador de visões do ngdart sem `build_runner` — especificação

Objetivo: `dartforge serve`/`dev` rodarem o `new_sali/frontend` e o
`limitless_ui/example` **sem** o `build_runner` ter rodado antes. Para isso o
`crates/gerador_ng` tem de gerar todo `.template.dart`/`.css.shim.dart` que
esses dois projetos (e as dependências deles com saída do ngdart) pedem,
**byte a byte igual** ao compilador oficial. Onde não souber, recusa (a regra
de sempre, `docs/GERADOR-NG.md` §1) — mas a meta é 0 recusas nesses projetos.

Este documento é a especificação da rodada: o placar de partida medido, cada
sub-forma pendente com a regra do compilador oficial (arquivo:linha), o
desenho no `gerador_ng` e a ordem. As regras gerais do porte continuam em
`docs/GERADOR-NG.md` e `docs/espec-ngdart/`.

Abreviações das referências:

- `NC:` — `ngcompiler` 3.0.0-dev.3 (o do `ngdart` 8.0.0-dev.4), em
  `E:\references\ngdart\ngcompiler\lib\v1\src\`.
- `NX:` — `ngx_compiler` 9.0.0-dev.2 (o do `ngx_dart` 9.0.0-dev.2), no
  pub-cache (`...\hosted\pub.dev\ngx_compiler-9.0.0-dev.2\lib\v1\src\`).

---

## 0. Oráculo e placar de partida

Os projetos em `C:\MyDartProjects` são só leitura. As cópias e o oráculo do
`build_runner` ficam em `E:\dftemp\ngdart\` (`new_sali\{frontend,core}` e
`limitless_ui\`), gerados com `dart pub get --offline` e
`dart run build_runner build --delete-conflicting-outputs` (Dart 3.6.2):
945 saídas do ngdart no new_sali (300 do pacote, 184 do `limitless_ui`
1.0.0-dev.34 hospedado, 95 `ngdart`, 124 `ngcompiler`, 28 `ngforms`, 26
`ngrouter`, 16 `ngtest`) e 628 no `limitless_ui/example` (190 do
`limitless_ui`, 68 do exemplo, 95 `ngx_dart`, 124 `ngx_compiler`, 28
`ngx_forms`, 26 `ngx_router`).

**Importante:** o `limitless_ui` de `C:\MyDartProjects` já não usa o `ngdart`
8: desde o commit `378a750` ele depende de `ngx_dart`/`ngx_forms`/`ngx_router`
9.0.0-dev.2, o fork que troca `dart:html` por `package:web`. O compilador dele
(`ngx_compiler` 9.0.0-dev.2) emite outro dialeto (§B). O `new_sali` continua
no `ngdart` 8 (e usa o `limitless_ui` 1.0.0-dev.34 hospedado, ainda no
`ngdart` 8).

Harness: `cargo run -p dartforge-gerador-ng --example oraculo --
<projeto> --todos --listar [--despejar <dir>]`. Nesta rodada ganhou:

- `--todos`: gera e compara também cada dependência com saída oficial em
  `.dart_tool/build/generated/<pacote>/lib` (menos o próprio compilador:
  `ngcompiler`/`ngx_compiler`/`ngast`/`ngx_ast`, que o DDC nunca pede), e a
  carga do programa importa todas as bibliotecas delas;
- a folha `.css` de entrada do shim é a que o `sass_builder` da mesma build
  escreveu (como o motor faz com a saída do Sass nativo). O oráculo mede o
  compilador do ngdart; o Sass tem o seu (§A0).

Placar de partida (2026-09-30, antes desta rodada):

| projeto | iguais | diferentes | pendentes |
|---|---|---|---|
| new_sali/frontend (+ dependências) | 791 | 6 | 25 (24 com oficial) |
| limitless_ui/example (+ dependências) | 368 | 23 | 114 (113 com oficial) |

O `ESTADO.md` §2.0 (114 pendentes no new_sali) estava defasado: as rodadas
de 2026-09-24…27 (`docs/GERADOR-NG.md` §9) já tinham tirado quase tudo.

---

## Parte A — new_sali/frontend (ngdart 8)

### A0. Folhas: o Sass 1.101 (fora do compilador de visões)

Com o `.css` do nosso Sass (que imita o dart-sass 1.102.0 e o 1.66.0), 41
`.css.shim.dart` saíam diferentes: o lock do new_sali tem o dart-sass
**1.101.0** e o do limitless 1.101.2, e o 1.101 serializa cor no modo
`compressed` pela forma mais curta (`hsla(0,0%,100%,.1)` onde o 1.102 escreve
`rgba(255,255,255,.1)`), além de as dependências compilarem no estilo
`expanded` (sem `outputStyle` no `build.yaml` delas). Com o `.css` oficial
como entrada, **todo** `.css.shim.dart` sai igual: o shim está certo. O modo
1.101 do Sass nativo é do `crates/sass` (`VersaoDartSass::do_lock` não conhece
`1.101.x`), fora deste documento — ver §D.

### A1. Tipo privado no local do `*ngFor` (5 diferentes)

Regra: `fromDartType` (`NC:compiler/output/convert.dart:35-37`) devolve
`dynamic` para tipo cujo elemento é privado — "private types aren't visible
to generated code" — e os argumentos de tipo passam pela mesma função
(`convert.dart:48-58`). O `getLocal` (`NC:compiler/view_compiler/
view_name_resolver.dart:55`) só põe o `unsafeCast<T>` quando
`type != dynamic`. Então `_CustomSelectItem` sai `this.locals['$implicit']`
sem cast, e `List<_CalendarCell>` sai `unsafeCast<List<dynamic>>`.

O tipo do local continua o privado para a tipagem das expressões (o
`_TypeResolver` usa o `DartType`, não o texto): `local_o.label` segue tipado
pelo membro de `_CustomSelectItem`.

Desenho: em `visao::tipo_qualificado`, nome cujo nome simples começa com `_`
vira `dynamic`, descartando os argumentos e o `?` dele; em
`declaracao_de_local`, o tipo qualificado `dynamic` sai sem cast.

### A2. `\r\n` em string de várias linhas da anotação (1 diferente)

O `li_highlight_component.dart` do `limitless_ui` 1.0.0-dev.34 hospedado tem
fim de linha CRLF, e o `styles: ['''...''']` dele sai no oficial com `\n`. Na
especificação do Dart (§17.7.1, *Strings*), numa string de várias linhas cada
fim de linha (`\r\n`, `\r`) do fonte vale `\n` no valor; o analyzer entrega o
valor assim ao ngcompiler.

Desenho: `componente::texto_do_argumento` (o valor de `template:`,
`styles:` e afins) normaliza `\r\n` e `\r` para `\n`. As posições do `REF` de
`template:` já são as do valor decodificado (casos j53, j63), e seguem sendo.
O `constant_value` do `crates/frontend` não normaliza — é um defeito dele
também para o JS gerado (o valor da string em tempo de execução); fica
registrado em §D.

### A3. Elemento de coleção genérica com argumento de fora do `dart:core` (19 pendentes)

Caso: `DataFrame<AcaoFavorita> get favoritos` com
`DataFrame<T> extends ListBase<T>` (`essential_core`). O
`getIterableElementType` (`NC:compiler/analyzed_class.dart:39-42`) é o retorno
de `lookUpGetter2('single')` no tipo da coleção: `ListMixin<E>.single`, com
`E` → `T` → `AcaoFavorita`. O resolvedor textual (`resolucao.rs`) só sabe
substituir um argumento que "se escreve igual em qualquer escopo"
(`escrito_igual_em_todo_escopo`: `dynamic` e os do `dart:core`); `AcaoFavorita`
está escrito no escopo do componente e o `E` no do `dart:collection`, e a
busca desiste.

Desenho — **nomes ancorados**: um argumento de tipo que atravessa escopos
leva a biblioteca que declara cada nome dele, escrito como `NomeªN` (`ª` é
alfabético para o `char::is_alphanumeric`, então todo leitor de palavras do
porte o trata como um nome só; `N` é o índice da biblioteca no `Program`).

- `Resolvedor::ancorar(arquivo, texto, livres)`: cada nome de `texto` que
  não é do `dart:core`, `dynamic`/`void`, parâmetro livre ou já ancorado é
  procurado no escopo de `arquivo` (com prefixo) e trocado por `NomeªN` da
  biblioteca que o declara; nome que não se acha, `None`.
- `instanciar` ancora os argumentos escritos no receptor (no escopo de quem
  pergunta) e `argumentos_do_supertipo` ancora os que o `extends`/`with`
  escreve (no escopo da unidade que declara a classe);
  `escrito_igual_em_todo_escopo` aceita nome ancorado.
- `classe`, `procurar` (`uri_do_tipo`), `tipo_inexistente` e
  `limites_de_tipo` resolvem `NomeªN` direto na biblioteca `N`, qualquer que
  seja o arquivo.
- `visao::tipo_qualificado` escreve o nome simples (sem a âncora) com o import
  da biblioteca dela; a regra de A1 vale para o nome simples.

O mesmo vale para qualquer membro lido de um receptor genérico com argumento
de fora do `dart:core` (`{{ x.item.nome }}` com `x: Grupo<Item>`), não só o
`*ngFor`.

### A4. Local de `*` com chave desconhecida (1 pendente)

`*ngFor="let s of StatusUsuario.items; let i = $index"`: o `NgFor` não
publica `$index`, e o `_typeNgForLocals` (`NC:compiler/template_optimize.dart:
76-95`) só tipa `$implicit`, `index`, `count`, `first`, `last`, `even`, `odd`.
O local sem tipo sai `final local_i = this.locals['\$index'];` (sem cast,
`view_name_resolver.dart:55`), e o valor em tempo de execução é `null` — o
que o oficial faz.

Desenho: `locais_da_micro` dá `dynamic` à chave desconhecida em vez de
recusar.

### A5. `@ViewChild` de `#ref="exportAs"` dentro de `*` (3 pendentes)

Caso: `<form #formularioCga="ngForm">` dentro de `*ngIf`, com
`@ViewChild('formularioCga') NgForm? formularioCga`. O resultado da consulta é
o provedor do token exportado no nó (`NC:compiler/view_compiler/
compile_element.dart:254-262`: `referenceTokens[nome]` → `_providers.get(token)
.build()`), registrado na consulta com a visão embutida em que o nó está; a
consulta dinâmica o lê pelo `mapNestedViewsWithSingleResult`
(`return nestedView._NgForm_4_5;`), como a de um elemento.

Desenho: a árvore da consulta (`arvore_da_consulta`/`consulta_em_embutida`)
aceita o `#ref` com valor como terminal quando uma diretiva do nó o exporta;
o valor lido na visão aninhada é o campo da instância (o mesmo que o `#ref`
lê, `refs`), no lugar do `_el_n`. Lista, `read:` e `#ref` exportado por mais
de uma diretiva continuam recusados.

### A6. `&` em atributo interpolado (1 pendente) e entidade na interpolação de texto

O tokenizador do ngast não decodifica entidades no valor de atributo
(`docs/espec-ngdart/02-parser-de-template.md` R1.5/R1.6): o oficial passa
`placeholder="{{ a && !b ? 'x' : '' }}"` cru ao parser de expressões. A recusa
`atributo interpolado com entidade HTML` (`visao.rs`, `valor_interpolado`)
bloqueava `&&`. Desenho: tirar a recusa (o texto já é o cru).

Na interpolação de **texto**, ao contrário, o valor passa pelo
`_unEscapeText` (R1.5: `{{ a &amp;&amp; b }}` → `a && b`); o porte não
decodificava (`html.rs`, `No::Interpolacao`). Desenho: decodificar o valor da
interpolação de texto com a mesma `decodificar`.

### A7. `#ref` lido em evento dentro do `*ngFor` do `home_page` (1 pendente)

As recusas `#ref em visão embutida` e `evento: nome fora do componente` do
`home_page` vêm na coleta depois da recusa do `*ngFor` (A3): a visão
embutida não é montada e o resto do template é visitado sem os locais dela.
Com A3 a forma é a já coberta (`#ref` lido por handler da própria embutida,
caso j60). Se sobrar recusa depois de A3, ela entra aqui com a regra.

### A8. Folha `web/scrollbar.css` (pendente sem oficial)

O `build.yaml` do new_sali exclui `web/scrollbar.css` do ngdart
(`generate_for`); o oficial não gera nada (e o shim dele lançaria). Não é
pendência: o motor aplica o `generate_for`, o harness não.

---

## Parte B — o dialeto `ngx_dart` 9 (limitless_ui)

A diferença entre `NC` e `NX`, com os nomes de pacote normalizados, tem 737
linhas; tirando comentários e `library;`, é **esta** lista — ela é a
especificação completa do dialeto. O resto do compilador é idêntico, então
toda forma que o porte já cobre no ngdart 8 vale no 9 com estas trocas.

### B1. Pacotes

`ngdart`→`ngx_dart`, `ngforms`→`ngx_forms`, `ngrouter`→`ngx_router`,
`ngtest`→`ngx_test` (e `ngcompiler`/`ngast`, que não aparecem na saída). Os
caminhos dentro dos pacotes são os mesmos (conferido: toda constante
`package:ngdart/...` do porte existe em `ngx_dart-9.0.0-dev.2/lib`).

Desenho — **dialeto por geração**: `dialeto::Dialeto { Ngdart, Ngx }`,
decidido pelo programa (há biblioteca `package:ngx_dart/…` carregada → `Ngx`)
e posto num `thread_local` pelas entradas públicas do crate (`gerar_em`,
`gerar_com_apoio`, `gerar_arquivo`, `Indice::montar`/`do_programa`). Por
dentro o porte continua falando `ngdart`: toda URI que vem do programa passa
por `dialeto::canonica` (`package:ngx_dart/…` → `package:ngdart/…`, e os
outros três pares) nos pontos em que é lida (`resolucao.rs` e `metadados.rs`, onde se
lê `library.uri`), e a busca por URI no programa desfaz a troca
(`dialeto::no_programa`). Na saída, `Importacoes::escrever` escreve a URI no
dialeto (`dialeto::escrita`). Um nome só é trocado quando é prefixo de pacote
inteiro (`package:ngdart/`), então `package:ngdart_x/` não é tocado.

### B2. `dart:html` → `package:web`

`NX:compiler/identifiers.dart:295-389`: cada identificador de DOM do
compilador tem outro nome e outro módulo (`asset:web/lib/src/dom/<m>.dart`,
escrito `package:web/src/dom/<m>.dart`):

| ngdart 8 (`dart:html`) | ngx 9 | módulo |
|---|---|---|
| `document`, `Element`, `Node`, `Text`, `Comment`, `DocumentFragment`, `Event` | igual | `dom.dart` |
| `HtmlElement` | `HTMLElement` | `html.dart` |
| `AnchorElement` `AreaElement` `AudioElement` `ButtonElement` `CanvasElement` `DivElement` `FormElement` `IFrameElement` `ImageElement` `InputElement` `TextAreaElement` `MediaElement` `MenuElement` `OptionElement` `OListElement` `SelectElement` `TableElement` `TableRowElement` `TableColElement` `UListElement` | `HTML` + nome (`HTMLDivElement`, `HTMLIFrameElement`…) | `html.dart` |
| `SvgSvgElement` / `SvgElement` (`dart:svg`) | `SVGSVGElement` / `SVGElement` | `svg.dart` |
| `MouseEvent` `KeyboardEvent` `FocusEvent` `InputEvent` `CompositionEvent` `WheelEvent` | — (novos) | `uievents.dart` |
| `DragEvent` | — | `html.dart` |
| `PointerEvent` / `TouchEvent` / `ClipboardEvent` / `AnimationEvent` / `TransitionEvent` | — | `pointerevents.dart` / `touch_events.dart` / `clipboard_apis.dart` / `css_animations.dart` / `css_transitions.dart` |

Um só `dart:html` do ngdart vira vários módulos: o import de cada um é alocado
quando o primeiro nome dele é escrito, na mesma posição em que o `dart:html`
seria (o emissor é o mesmo; só o `moduleUrl` do identificador mudou). No
cabeçalho da classe, cada campo `_el_n` aloca o módulo do tipo dele, na ordem
dos campos; o construtor aloca `dom.dart` pelo `document`.

Desenho: `dialeto::dom(nome_ngdart) -> (uri, nome)`; os pontos que hoje
escrevem `{html}.X` com o alias de `dart:html` passam a pedir o import pelo
nome (`Importacoes::dom(nome)` → `importN.Y`). A pré-alocação do `dart:html`
em `alocar_imports_dos_campos` vira a alocação, em ordem, dos módulos dos
tipos dos campos de elemento.

Os tipos do usuário (`HTMLInputElement? campo` num `@ViewChild`) já saem pelo
caminho geral (`uri_do_tipo` → biblioteca que declara →
`package:web/src/dom/html.dart`).

Reconhecimento do lado da entrada (o que hoje compara com `dart:html`):

- injeção do nó (`NX:source_gen/template_compiler/compile_metadata.dart:
  459-488`): `Element` de `dom.dart` e `HTMLElement` de `html.dart` são os
  embutidos `Element`/`HtmlElement`; um `typedef` que os apelida também
  (`_aliasedRenderNodeToken`);
- `isElementType` das consultas (`NX:source_gen/template_compiler/
  find_components.dart:533-534`): `Element` de `package:web/src/dom/dom.dart`
  e os subtipos dele. No `package:web` todos são *extension types* que
  `implements Element` (direta ou indiretamente); o porte aceita os nomes
  `Element`, `*Element` declarados em `package:web/src/dom/`.

### B3. Provedor embutido `JSObject` — a numeração sobe um

`NX:compiler/view_compiler/compile_element.dart:116`:
`_providers.add(JsInterop.jsObjectToken, renderNode)` logo depois do
`ElementRef`. Todo `uniqueId` de provedor de elemento sobe um (conferido no
oráculo: `_TemplateRef_n_9`, `_NgIf_n_10`, onde o 8 dava 8 e 9). Vale para o
nó de template, o do filho, o do `<template>` e a hospedeira.

Desenho: `dialeto::embutidos_extra()` (0 ou 1) somado a cada constante da
tabela de `docs/GERADOR-NG.md` §2 (`diretivas.rs`: 5/7/8; `visao.rs`: o
`_TemplateRef_n_8` e a diretiva `_9` do `*`, o 7/8 do `<template>`).

### B4. Eventos tipados

- Evento nativo: `NX:compiler/semantic_analysis/binding_converter.dart:75,
  228` dá o tipo `nativeHtmlEventType(nome)` (`NX:compiler/html_events.dart:
  116-177`: a tabela de 52 nomes → `MouseEvent`, `KeyboardEvent`…; o resto,
  `Event`).
- Saída de diretiva/componente: `outputTypes[membro]`
  (`binding_converter.dart:208-211`), lido por `_outputType`
  (`NX:source_gen/template_compiler/find_components.dart:412-445`): o tipo
  declarado do campo/getter como `Stream<T>` (`asInstanceOf`) dá `T` pelo
  `fromDartType`; `Stream<void>` dá `dynamic`; o que não é `Stream`, nada.
- Evento próprio (`eventManager`): sem tipo, como no 8.

Onde o tipo aparece:

1. O ouvinte nativo (`NX:compiler/view_compiler/update_statement_visitor.dart:
   254-261`): `el.addEventListener('click',
   importJ.FunctionToJSExportedDartFunction((this.eventHandler1(h) as void
   Function(importK.MouseEvent))).toJS)` — `dart:js_interop` alocado antes do
   módulo do tipo; sem tipo, `Event`.
2. O método do handler complexo (`NX:compiler/view_compiler/compile_view.dart:
   726-760`): `void _handleEvent_N(T $event)`; sem tipo (evento próprio,
   saída sem `Stream`), `$event` cru como no 8; `Stream<dynamic>`/`void`,
   `dynamic $event`.

Desenho: `ouvinte` e `handler_com` recebem o tipo do evento; o import do tipo
no método é tardio (o método sai depois do `destroyInternal`).

### B5. Consulta estática de elemento com `unsafeCast`

`NX:compiler/view_compiler/compile_query.dart:473-486`: na atualização só
estática, com `metadata.isElementType`, o valor é `unsafeCast(v)`
(`_ctx.x = importN.unsafeCast(this._el_3);`, e cada item da lista). A
aninhada não muda (`TODO` no oficial).

### B6. `[style.x]` sem nulo

`NX:compiler/view_compiler/update_statement_visitor.dart:131-160`: com unidade,
`(v == null) ? '' : (v.toString() + 'px')` (o 8 escrevia `null`); sem unidade,
o valor (ou `v?.toString()`) seguido de `?? ''` quando a fonte pode ser nula
(`isNullable` = `canBeNull`).

### B7. Nó raiz criado solto com o tipo

`NX:compiler/view_compiler/compile_view.dart:822-834`: o elemento sem pai
(`doc.createElement('x')`) sai `unsafeCast<T>(..)` com o tipo do nó
(`importN.unsafeCast<import5.HTMLElement>(doc.createElement('span'))`).

### B8. O motor

O builder é outro: `ngx_dart:ngx_dart` (mesmas fábricas, mesmas extensões).
`crates/build` passa a registrar o gerador nativo para as duas chaves
(`descritor.rs`, `nativos/ng.rs`), e quem pergunta pelas entradas/saídas
invisíveis do ngdart (`cli/motor.rs`, `dev`) pergunta pela chave que o plano
tiver.

---

## Parte C — formas que a medição mostrou

O placar de partida do limitless mistura as recusas do dialeto (metadados
lidos por URI `package:ngdart/…` que não existe no programa: "sem metadados",
`NgIf em <template>`…) com formas novas, que só se enumeram com a parte B de
pé. Estas apareceram depois de A e B, nos dois projetos:

### C1. `exports:` de `enum`/`mixin` não tem `AnalyzedClass`

`*ngFor="let s of StatusUsuario.items"` com `StatusUsuario` um `enum` em
`exports:`: o oficial escreve o `checkBinding` (mutável). No analyzer 6, `enum`
e `mixin` não são `ClassElement`, e o `_extractExports`
(`NC:source_gen/template_compiler/find_components.dart:891-893`) só dá
`AnalyzedClass` a `ClassElement`; sem ela o `isImmutable` de `X.y`
(`NC:compiler/analyzed_class.dart:134-137`, que exige
`receiver.id.analyzedClass != null`) é `false`, e o tipo, `dynamic`.

Desenho: `Exportado::TipoSemClasse` (enum, mixin, *extension type*); o membro
lido dele é mutável e `dynamic`.

### C2. `@ViewChild` sem `#ref` no template não lê o `_ctx`

A única sem resultado não recebe nada (caso j82), e o `build()` não declara
`final _ctx = this.ctx` por causa dela. O porte marcava o `_ctx` mesmo sem
atribuição; aparecia no `incluir_cga`/`incluir_cgm`, que têm um
`@ViewChild('selectTipoPessoa')` sem `#ref`.

### C3. `[style.x]` do `@HostBinding`: `isNullable` é o `canBeNull`

`BoundExpression.isNullable` é `canBeNull(expression)`
(`NC:compiler/ir/model.dart:609`), e `canBeNull` só é falso para literal,
`EmptyExpr` e interpolação (`analyzed_class.dart:210-221`). Para o
`_ctx.membro` de um `@HostBinding`, então, sempre verdadeiro — pouco importa
o tipo escrito: `?.toString()` no 8 e `(v ?? '')` no 9 (`String hostDisplay`
sai `(currVal_2 ?? '')` no oráculo do limitless). O porte decidia pelo `?` do
tipo escrito.

### C4. Saídas de fases posteriores no programa do harness

O `messages.i18n.dart` que o `i18n` (`build_to: source`) escreve em `lib/`
não existe para o resolvedor do ngdart (a fase dele vem depois): `t.app.brand`
é `InvalidType` e a interpolação é `interpolate0`. O motor já oculta essas
saídas (`Motor::saidas_invisiveis_a`); o harness passou a ocultar os
`x.i18n.dart` com `x.i18n.yaml` ao lado e a carregar todo arquivo como raiz
(sem a biblioteca sintética).

---

## Ordem

1. A1–A6 (formas do ngdart 8, independentes do dialeto), com o oráculo do
   new_sali e o corpus (`tests/corpus.rs`) sem regressão.
2. B1–B7 no gerador (dialeto), B8 no motor.
3. Medir o limitless; enumerar e implementar a parte C.
4. Verificação: oráculo dos dois projetos, testes do crate, `dartforge serve`
   dos dois no navegador sem `build_runner`.

## D. Fora do compilador de visões (registrado)

- Sass: o modo dart-sass 1.101.x (`crates/sass`), para o `.css` servido sair
  do nativo igual ao do `sass_builder` desses locks.
- `crates/frontend`: o valor de string de várias linhas com CRLF no fonte deve
  ser com `\n` (§A2) — afeta também o JS gerado.
- `crates/emit_js`: construtor não-`external` de *extension type* de interop
  (`package:web`) — `finish_ctor_call` cai no `new X.new()`, e o objeto de
  apoio dos tipos de extensão só sai para os apagados. Bloqueia o limitless
  no navegador.
- Programa do `dev`/`serve` enxerga as saídas `build_to: source` de fases
  posteriores ao ngdart (o `messages.i18n.dart`): o template sai com
  `interpolateString0` onde o oficial tem `interpolate0` — mesmo
  comportamento, texto diferente.

## E. Resultado

Oráculo (`oraculo --todos`, 2026-09-30):

| projeto | antes (iguais / diferentes / pendentes) | depois |
|---|---|---|
| new_sali/frontend + dependências | 791 / 6 / 25 (24 com oficial) | **821 / 0 / 1 (0 com oficial)** |
| limitless_ui/example + dependências | 368 / 23 / 114 (113 com oficial) | **504 / 0 / 1 (0 com oficial)** |

821 e 504 são todas as saídas oficiais de cada projeto menos as 124 do
próprio `ngcompiler`/`ngx_compiler` (que o harness não examina: o DDC nunca
as pede). O pendente de cada um é o `web/scrollbar.css`, excluído pelo
`generate_for` do `build.yaml` (o oficial não o gera).

Testes do crate: 62 unitários, corpus `corpus/ngdart` **372 conferidos, 0
pendentes**, incremental, Sass e shim — todos verdes.

`dartforge serve`/`dev` em cópias limpas (sem `.dart_tool/build`, sem nenhum
`.template.dart`), aberto no Edge headless:

- a sessão do `dev` passou a carregar, além do `main`, as entradas do ngdart
  do `lib/` e `web/` do pacote (`EtapaDeGeracao::raizes`,
  `crates/dev/src/{geracao,lib}.rs`), como a CLI `build` já fazia: antes, o
  `app_component.dart` só era alcançável pelo próprio `.template.dart` (que
  ainda não existia), ficava fora do programa e o gerador o recusava — a
  carga falhava com 83 templates ausentes;
- **new_sali/frontend**: pronto em 12 s, 592 módulos, 968 ações nativas do
  motor; a tela de login monta ("Bem-vindo(a) … ENTRAR"), 0 exceções; único
  erro: `style.css` 404 (o `web/style.scss` é do `sass_builder`, e o lock tem
  o dart-sass 1.101.0, que o Sass nativo não imita — §D);
- **limitless_ui/example**: pronto em 10 s, 477 módulos, 636 ações nativas do
  `ngx_dart`; quebra em tempo de execução no `ngx_dart`
  (`HTMLStyleElement.new is not a constructor`): o construtor gerativo
  não-`external` de *extension type* de interop do `package:web`
  (`HTMLStyleElement() : _ = document.createElement('style')`) sai do
  `crates/emit_js` como `new X.new()` de classe — defeito do emissor JS,
  encaminhado (§D).
