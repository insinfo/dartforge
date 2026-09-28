# 04 — Compilação de visões (ngcompiler 3.0.0-dev.3)

Esta seção descreve, a partir da **leitura do código-fonte oficial**, como o
`ngcompiler` transforma um `@Component` nas classes de visão do
`.template.dart`: a estrutura do arquivo, a numeração de nós, a ordem das
instruções no `build()`, a detecção de mudanças, a destruição, as fábricas e a
alocação dos imports. Serve de guia para o porte byte a byte em Rust
(`crates/gerador_ng/src/visao.rs`).

Convenções:

- `C/` = `/root/.pub-cache/hosted/pub.dev/ngcompiler-3.0.0-dev.3/lib/v1/src/compiler/`;
  `VC/` = `C/view_compiler/`.
- `visao.rs:N` = `/home/user/dartforge/crates/gerador_ng/src/visao.rs`, linha da
  função citada; `dom.rs:N` idem. As linhas do Rust são um retrato do arquivo
  no momento da leitura (ele muda com frequência): na dúvida, procure pelo
  nome da função.
- Status do porte: **Coberto** (o Rust emite a forma), **Parcial** (emite
  parte ou só em casos restritos) ou **Falta** (recusa ou não trata).
- Nos exemplos, `importK` é o número que o emissor aloca (regra §2.2): o
  número exato depende do que já foi escrito antes no arquivo. `⟨…⟩` marca um
  trecho que outra seção da especificação define (expressões, interpolação,
  injeção, consultas).
- A saída mostrada já passou pelo `DartFormatter(pageWidth: 1000000)`
  (`lib/v1/src/angular_compiler/cli/builder.dart:15`), que é o que o `build_runner`
  grava.

---

## 1. Pipeline por componente e ordem das declarações de topo

### 1.1 Duas visões IR por componente

**Regra.** Cada `@Component` vira um `ir.Component` com exatamente duas visões
IR, nesta ordem: a visão do componente (`ir.ComponentView`) e a visão
hospedeira (`ir.HostView`) (`C/../angular_compiler.dart:100-113`). A hospedeira
é o template sintético do seletor, com o próprio componente como única
diretiva (`angular_compiler.dart:149-169`).

**Regra.** `TemplateCompiler.compile` percorre os componentes na ordem em que
foram achados na biblioteca e, para cada visão, emite **primeiro** a declaração
de estilos e **depois** as instruções da visão
(`C/template_compiler.dart:66-105`). Depois de todos os componentes vêm as
classes `NgCd` das diretivas que precisam de detector
(`template_compiler.dart:71-76`).

Ordem resultante das declarações de topo, por componente `X`:

1. `final List<Object> styles$X = …;` (§3)
2. `class ViewX0 extends ComponentView<X>` (§4)
3. `const _XNgFactory`, getter `XNgFactory`, função `createXFactory` (§5)
4. Para cada visão embutida, em pré-ordem (§7): `class _ViewXk` e
   `viewFactory_Xk`
5. `final List<Object> styles$XHost = const [];`
6. `class _ViewXHost0 extends HostView<X>` e `viewFactory_XHost0` (§6)

A pré-ordem vem de `_finishView` (`VC/view_compiler.dart:76-93`): escreve a
classe da visão (`_createViewTopLevelStmts`, `:95-109`) e depois, para cada nó
da visão com `embeddedView`, recursivamente a embutida. A visão do componente
recebe `registerComponentFactory: true` (só ela, `template_compiler.dart:101`),
por isso as fábricas saem logo após `ViewX0` e **antes** das embutidas.

**Rust.** Coberto — `gerar_componente_com` (`visao.rs:10113`) escreve nessa
ordem (o `format!` final: estilos, `ViewX0`, fábricas, `{embutidas}`,
`styles$XHost`, hospedeira). Vários componentes num arquivo:
`trecho_de_componente` (`visao.rs:9749`) + `montar_arquivo` (`visao.rs:9806`),
com a tabela de imports compartilhada. As classes `NgCd`: `classe_ngcd`
(`visao.rs:9686`).

### 1.2 Otimização de encapsulamento

**Regra.** Encapsulamento `emulated` sem `styles` e sem folhas externas vira
`none` antes da compilação (`C/ast_directive_normalizer.dart:154-160`).
Consequência: sem estilo não há `addShimC/E` (§11.4) e o
`ComponentStyles` é `unscoped` (§4.6).

**Rust.** Coberto — `com_estilo` exige estilo e encapsulamento
(`visao.rs:10113`, campo `com_estilo` do `Contexto`), e a escolha
`scoped/unscoped` no fim de `gerar_componente_com`.

---

## 2. Emissão Dart e alocação de imports

### 2.1 Cabeçalho do arquivo

**Regra.** O arquivo é montado por `buildGeneratedCode`
(`C/../source_gen/template_compiler/code_builder.dart:8-53`): a primeira linha
é `import '<arquivo-fonte>';` (sem prefixo); se a flag
`exportUserCodeFromTemplate` estiver ligada, `export '<arquivo-fonte>';`; depois
(sem injetores gerados) o texto do emissor, que começa pelos seus próprios
`import`. O emissor lista os imports na ordem de alocação e só então o código
(`C/output/dart_emitter.dart:51-70`).

**Rust.** Coberto sem o `export` — `montar_arquivo` (`visao.rs:9806`) escreve
o cabeçalho, `import '{arquivo}';` e a tabela. A flag
`exportUserCodeFromTemplate` não é tratada nesta função.

### 2.2 Numeração `importN`

**Regra.** O prefixo é alocado **na primeira vez que o emissor escreve** um
identificador de outro módulo, na ordem textual de escrita
(`dart_emitter.dart:727-754`):

- `moduleUrl == null` (ex.: `override`, `Identifiers.dartCoreOverride`,
  `identifiers.dart:352`) ou igual ao módulo do próprio `.template.dart` → sem
  prefixo e **não** entra na tabela (é por isso que `ViewY0` de um filho no
  mesmo arquivo sai sem import);
- URL na lista `_allowListedImports` (`dart_emitter.dart:76-100`:
  `package:ngdart/angular.dart`, `dart:core`, `element_ref.dart`,
  `view_container.dart`, `template_ref.dart`, `change_detection.dart`,
  `ng_if.dart`, `app_view.dart`, `render/api.dart`, em `asset:` e `package:`)
  → prefixo vazio, **mas ocupa um número** (`'import${importsWithPrefixes.length}'`
  conta a entrada vazia);
- qualquer outra → `importK` com `K = importsWithPrefixes.length`.

A linha emitida é `import '<caminho>' as importK;` ou `import '<caminho>';`
(`dart_emitter.dart:62-67`).

Como a classe é escrita campo a campo (§4.2), construtor, getters e métodos, a
numeração segue exatamente essa ordem. Exemplo, componente sem estilo
`MeuApp` em `lib/meu_app.dart` com um texto interpolado:

```
import 'meu_app.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'meu_app.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'dart:html' as import7;
...
```

(`ComponentView` no `extends` → import0; o tipo `MeuApp` → import1; o campo
com inicializador `_textBinding_N` → import2; o campo estático
`_componentStyles` → import3; construtor: `View`, `ChangeDetectionCheckedState`,
`unsafeCast`, `document` → import4..7; o resto no `build()` e adiante.)

**Rust.** Coberto — `Importacoes` (`visao.rs:25-90`, `SEM_PREFIXO` replica a
allowlist) com pré-alocação na ordem do oficial em `gerar_componente_com`
(`visao.rs:10113`: estilos → limites genéricos → `COMPONENT_VIEW` → arquivo
próprio → campos via `alocar_imports_preguicosos`/`alocar_imports_dos_campos`
(`visao.rs:9002`, `visao.rs:9018`) → `STYLE_ENCAPSULATION`, `VIEW`,
`CHANGE_DETECTION`, `UTILITIES`, `dart:html`), `sem_alias(ANGULAR)` depois da
classe e imports "tardios" (`tardio`/`resolver_tardios`, `visao.rs:9347-9359`)
resolvidos no ponto de escrita. `q_chave` cobre o caso de a mesma URI receber
dois números.

### 2.3 Campos: agrupamento pelo emissor

**Regra.** O emissor escreve **primeiro todos os campos com inicializador e
depois os sem inicializador**, cada grupo na ordem de alocação no
`CompileViewStorage` (`dart_emitter.dart:198-205`;
`VC/compile_view.dart:1600-1639`). Depois: construtor, getters, métodos
(`dart_emitter.dart:206-214`).

Modificadores: `late`/`late final` (com `emitNullSafeSyntax`) ou
`/*late final*/ ` sem ele; `final`; `static`; `var` só se sem tipo
(`dart_emitter.dart:234-266`).

**Rust.** Coberto por construção — `campos_em_ordem` (`visao.rs:8105`) e a
junção de listas no fim de `gerar_componente_com` / `corpo_da_embutida`
(`campos_preguicosos` → `campos` → `campos_filho` → `campos_da_deteccao` →
pipes → `_el_` consultados → `_el_` …).

---

## 3. `styles$X`

**Regra.** `StyleCompiler._compileStyles` (`C/stylesheet_compiler/style_compiler.dart:56-95`)
declara `final List<Object> <var> = <lista>;` onde:

- `<var>` = `styles$X` para a visão do componente e `styles$XHost` para a
  hospedeira (`:108`, `:38-47`);
- a lista tem primeiro uma referência `importK.styles` por `styleUrls` (URL
  `stylesModuleUrl(url, shim)`), depois cada texto de `styles:` (passado por
  `shimShadowCss` com `_ngcontent-%ID%` / `_nghost-%ID%` se o encapsulamento é
  emulado, `:97-100`);
- lista vazia → `const []`; não vazia → `[…]` (sem `const`);
- a hospedeira é sempre `const []` (`compileHostComponent` com listas vazias).

Exemplo: `styleUrls: ['a.css']`, emulado →

```dart
final List<Object> styles$X = [import0.styles];
```

**Rust.** Coberto — `gerar_componente_com` (`visao.rs:10113`: `estilo`,
`em_linha`, `lista_de_estilos`); o shim é `crate::css::shim`
(`shadow_css.rs`). `styles:` com `@import` é recusado.

---

## 4. A classe `ViewX0`

### 4.1 Cabeçalho

**Regra.** `class ViewX0<T…> extends importA.ComponentView<importB.X<T…>> {`
(`VC/view_builder.dart:580-588`, `_createParentClassExpr :681-693`,
`contextType :966-972`). Nome: `'${viewIndex == 0 && viewType != host ? '' : '_'}View${X}$viewIndex'`
(`VC/compile_view.dart:449-450`) → `ViewX0`, `_ViewX1…`, `_ViewXHost0`.

**Rust.** Coberto (`visao.rs:10113`, bloco `write!`).

### 4.2 Ordem de alocação dos campos

**Regra.** O `CompileViewStorage` guarda os campos na ordem em que
`allocate` é chamado, e as fases do compilador acontecem nesta ordem:

1. **Construção** (`ViewBuilderVisitor`, ordem de documento):
   `_compView_N` (`compile_view.dart:872-919`), `_appEl_N`
   (`CompileElement` → `createViewContainer`, `:942-978`), provedores ansiosos
   referenciados fora do `build()` e provedores preguiçosos (`late … = …`,
   com inicializador) (`createProvider`, `:1134-1283`), mensagens
   `static final String _message_N` (`:580-590`).
2. **Ligação** (`bindView`): `Object? _expr_N` (`property_binder.dart`
   `_bind`, VC `property_binder.dart:301-348`), e nós de outra visão lidos
   por uma embutida (`getPropertyInView` → `replaceReadClassMemberInExpression`
   promove o `NodeReference` na hora, `view_compiler_utils.dart:86-110`).
3. **`afterNodes`** (`compile_view.dart:502-512`): `_pipe_x_N`, proxies
   `_pipe_x_N_M`, campos de consultas.
4. **`createViewClass`** (`view_builder.dart:572-579`): o
   `NodeReferenceStorageVisitor` percorre os métodos na ordem
   `build`, `injectorGetInternal`, `detectChangesInCheckAlwaysViews`,
   `detectChangesInternal`, `dirtyParentQueriesInternal`, `destroyInternal`,
   `view.methods`, `detectHostChanges`, e depois os getters; cada
   `NodeReference` lido fora do escopo em que foi declarado vira campo
   (`compile_view.dart:160-179`, `:255-316`): `late final Tipo _el_N;`, ou
   `final TextBinding _textBinding_N = TextBinding();` (tem inicializador).
5. **Estilos**: `static ComponentStyles? _componentStyles;` por último
   (`VC/view_style_linker.dart:72-84`).

Combinada com §2.3: `_textBinding_N`, provedores preguiçosos e `_message_N`
sobem para o topo (têm inicializador); depois `_compView_N`/`_appEl_N`/provedores
ansiosos, `_expr_N` (intercalados com `_el_` promovidos por embutidas), pipes,
`_el_N` promovidos na classe, e `_componentStyles`.

**Rust.** Parcial — reproduz a ordem por listas separadas
(`gerar_componente_com`, `corpo_da_embutida` em `visao.rs:8518`,
`campos_da_deteccao` em `visao.rs:733`); a ordem entre campos de i18n e
filhos/diretivas/`*` na mesma visão ainda é recusada
(`"@i18n com filho, diretiva ou * na visão"`, em `gerar_componente_com`).

### 4.3 Construtor

**Regra** (`view_builder.dart:609-657`, `view_style_linker.dart:35-48`):

```dart
ViewX0(importV.View parentView, int parentIndex) : super(parentView, parentIndex, importC.ChangeDetectionCheckedState.checkAlways) {
  this.initComponentStyles();
  this.rootElement = importU.unsafeCast(importH.document.createElement('tag'));
  ⟨hostAttributes do componente⟩
}
```

- `tag` = `_tagNameFromComponentSelector` (`view_builder.dart:695-707`): o
  primeiro seletor CSS com elemento; senão o seletor inteiro com `trim()`;
  vazio → `BuildError`.
- Estado: `checkOnce` se `changeDetection != checkAlways`, senão
  `checkAlways` (`_getChangeDetectionCheckMode`, `:974-979`), escrito como
  identificador `ChangeDetectionCheckedState.<nome>` do módulo
  `change_detection_constants.dart` (`VC/constants.dart:18-23`).
- `initComponentStyles()` é inserido na posição 0 do corpo
  (`view_style_linker.dart:40-43`).
- `hostAttributes` do componente (`@HostBinding` estáticos): cada um vira
  `createAttributeStatements` sobre `rootElement` (`:626-636`), com
  `isHtmlElement = detectHtmlElementFromTagName(tag)`.

**Rust.** Coberto — bloco `write!` de `gerar_componente_com` (estado, tag via
`crate::seletor`, `estaticos_no_construtor`).

### 4.4 `_debugComponentUrl`

**Regra** (`view_style_linker.dart:50-70`):

```dart
static String? get _debugComponentUrl {
  return (importU.isDevMode ? '<moduleUrl do tipo>' : null);
}
```

**Rust.** Coberto (`'asset:{pacote}/{relativo}'`).

### 4.5 Ordem dos métodos

**Regra** (`view_builder.dart:512-588`): `build` (`@override`),
`injectorGetInternal` (`@override`), `detectChangesInCheckAlwaysViews`
(`@override`, só se `component.isChangeDetectionLink`), `detectChangesInternal`
(`@override`), `dirtyParentQueriesInternal` (`@override`, só embutida),
`destroyInternal` (`@override`), `view.methods` (handlers `_handleEvent_N` e
métodos estáticos de i18n, na ordem de criação), `detectHostChanges(bool firstCheck)`
(sem `@override`, só na visão de componente com `@HostBinding` dinâmico);
**métodos de corpo vazio são descartados** (`:586`). O linker de estilos
acrescenta depois `static void _debugClearComponentStyles()` e
`void initComponentStyles()` (`view_style_linker.dart:86-163`).

**Rust.** Coberto, exceto `detectChangesInCheckAlwaysViews` (Falta: não há
`isChangeDetectionLink` no porte).

### 4.6 `initComponentStyles` e `_debugClearComponentStyles`

**Regra** (`view_style_linker.dart:86-163`):

```dart
static void _debugClearComponentStyles() {
  _componentStyles = null;
}

void initComponentStyles() {
  var styles = _componentStyles;
  if ((styles == null)) {
    _componentStyles = (styles = importS.ComponentStyles.scoped(styles$X, _debugComponentUrl));
    if (importU.isDevMode) {
      importS.ComponentStyles.debugOnClear(_debugClearComponentStyles);
    }
  }
  this.componentStyles = styles;
}
```

`scoped` se encapsulamento emulado, `unscoped` senão (`:28-33`).

**Rust.** Coberto (`visao.rs:10113`).

---

## 5. Fábricas do componente

**Regra** (`VC/view_compiler.dart:112-177`), só para a visão do componente:

```dart
const _XNgFactory = ComponentFactory<importB.X>('<seletor>', viewFactory_XHost0);
ComponentFactory<importB.X> get XNgFactory {
  return _XNgFactory;
}

ComponentFactory<importB.X<T>> createXFactory<T>() {
  return ComponentFactory('<seletor>', viewFactory_XHost0);
}
```

- `ComponentFactory` vem de `package:ngdart/angular.dart` (allowlist: sem
  prefixo, ocupa um número no ponto em que é escrito pela primeira vez).
- O `const` usa o tipo **cru** (`importType(componentTypeMetadata)`), o
  `create…Factory` usa `contextType` (com os parâmetros) e declara os
  parâmetros de tipo da classe.
- `<seletor>` é o seletor literal do componente, sem `trim`.

**Rust.** Coberto (`visao.rs:10113`, com `imp.sem_alias(ANGULAR)` logo depois
da classe).

---

## 6. A visão hospedeira `_ViewXHost0`

**Regra.** `_ViewXHost0<T…> extends importH.HostView<importB.X<T…>>`, **sem
construtor** (`view_builder.dart:600-603`), com `build()`:

```dart
@override
void build() {
  this.componentView = ViewX0(this, 0);
  final _el_0 = this.componentView.rootElement;
  ⟨ViewContainer do nó, se houver⟩
  ⟨provedores do nó⟩
  this.component = ⟨construção do componente⟩;
  ⟨consultas⟩
  this.initRootNode(_el_0);   // ou this._appEl_0
}
```

- `componentView` e `component` são campos da própria `HostView`
  (`compile_view.dart:884-888`, `:1232-1241`; `constants.dart:9`).
- A hospedeira não escreve atributos literais nem `addShim*`
  (`view_builder.dart:294-307`, `compile_view.dart:1069`) e não chama
  `create`/`createAndProject` (`view_builder.dart:339-354`).
- Fábrica (`view_builder.dart:787-814`):
  `importH.HostView<importB.X<T>> viewFactory_XHost0<T>() { return _ViewXHost0(); }`
  (sem argumentos de tipo no construtor: inferidos do retorno).
- `detectChangesInternal` só existe se algo além do filho único precisa ser
  detectado (`compile_view.dart:1398-1411`): ganchos do componente,
  `if (changed) markAsCheckOnce` de componente `onPush` com `@Input`,
  `detectHostChanges(firstCheck)`, `ViewContainer`. Quando existe, termina com
  `this.componentView.detectChanges();` (§14).
- `destroyInternal`: `_appEl_0.destroyNestedViews()` e `ngOnDestroy`, mas
  **não** `destroyInternalState` do filho (`view_builder.dart:715-718`).

**Rust.** Coberto — fim de `gerar_componente_com` (`visao.rs:10113`),
`construcao_do_componente` (`visao.rs:11593`), `ciclo_de_vida`
(`visao.rs:7500`), provedores `escrever_provedores_da_hospedeira`
(`visao.rs:11481`).

---

## 7. Visões embutidas

### 7.1 Numeração e nomes

**Regra.** Cada `EmbeddedTemplateAst` (`<template>` ou `*`) cria uma
`CompileView` com `viewIndex = pai.viewIndex + _nestedViewCount`, onde o
contador é incrementado **antes** de visitar os filhos e acumula os aninhados
(`view_builder.dart:416-469`) → numeração global em **pré-ordem** a partir de 1.
Nome `_ViewXk`, fábrica `viewFactory_Xk` (`view_compiler_utils.dart:151-158`).

Exemplo: `<template A><template B></template></template><template C>` → A=1,
B=2, C=3; emissão `_ViewX1`, `viewFactory_X1`, `_ViewX2`, `viewFactory_X2`,
`_ViewX3`, `viewFactory_X3`.

**Rust.** Coberto — `proxima_embutida` + `contar_estruturais`
(`visao.rs:9103`) em `estrutural`/`molde`; emissão recursiva em
`emitir_embutida` (`visao.rs:8491`).

### 7.2 Classe e fábrica

**Regra** (`view_builder.dart:659-678`, `:742-785`):

```dart
class _ViewX1 extends importE.EmbeddedView<importB.X> {
  ⟨campos⟩
  _ViewX1(importR.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() { … }
  …
}

importE.EmbeddedView<void> viewFactory_X1(importR.RenderView parentView, int parentIndex) {
  return _ViewX1(parentView, parentIndex);
}
```

Com componente genérico: `class _ViewX1<T>`, `viewFactory_X1<T>(…)` e
`return _ViewX1<T>(parentView, parentIndex);`. A embutida não tem
`initComponentStyles` (`view_style_linker.dart:8-14`).

**Rust.** Coberto — `corpo_da_embutida` (`visao.rs:8518`, `format!` final).

### 7.3 Locais do template

**Regra.** Cada `let-x="v"` vira local `x` → `this.locals['v']`
(`compile_view.dart:482-488`); ao ser usado, `ViewNameResolver.getLocal`
declara `final local_x = ⟨unsafeCast<T>(…)⟩ ;` (com `unsafeCast` só se o tipo é
conhecido e não `dynamic`) e, de visão ancestral, via `parentView`
(`VC/view_name_resolver.dart:40-71`). As declarações entram no
`detectChangesInternal` depois de `_ctx/changed/firstCheck`
(`compile_view.dart:1413-1463`). Locais do `NgFor` ganham tipo
(`$implicit` = elemento do iterável, `index/count` = `int`,
`first/last/even/odd` = `bool`) (`C/template_optimize.dart:50-97`).

**Rust.** Coberto — `locais_da_micro` (`visao.rs:6026`),
`declaracao_de_local` (`visao.rs:8908`).

### 7.4 Instrução final do `build()`

**Regra** (`view_builder.dart:871-913`):

- embutida: raízes = `createFlatArrayForProjectNodes(rootNodesOrViewContainers, constForEmpty: true)`;
  se for literal com **uma** entrada e sem subscrições →
  `this.initRootNode(<raiz>);`; senão
  `this.initRootNodesAndSubscriptions(importU.unsafeCast(<lista>), <subs|null>);`;
- componente: `this.initSubscriptions([subscription_0, …]);` só se houver
  subscrições;
- hospedeira: `this.initRootNode(<única raiz>);`.

Raiz de um nó com `ViewContainer` é o `_appEl_N` (`view_builder.dart:75-90`).

**Rust.** Coberto — `corpo_da_embutida` (variável `inicio`) e
`lista_plana` (`visao.rs:1928`).

---

## 8. Numeração de nós e nomes

### 8.1 Índice de nó

**Regra.** O índice é `_view.nodes.length` no momento da visita; consomem
índice: texto, texto ligado, `I18nTextAst`, elemento, componente, `<template>`,
`<ng-content>` **só se tiver `#ref`** (`view_builder.dart:92-206`).
`<ng-container>` **não** consome índice (`:170-172`). Texto filho de componente
que não projeta (`ngContentIndex == null`) é pulado mas **consome índice**
(`nodes.add(null)`, `_maybeSkipNode :129-138`), com aviso "Dead code in template".

Exemplo: `<p>a</p><ng-container><b></b></ng-container>{{x}}` → `_el_0`,
`_text_1`, `_el_2`, `_textBinding_3`.

**Rust.** Coberto — `proximo` em `Corpo::no` (`visao.rs:6516`), `container`
(`visao.rs:6642`), pulo do texto morto em `componente_filho`
(`visao.rs:3814`, trecho do conteúdo projetado). `<ng-content #ref>`: Falta.

### 8.2 Nomes

| Nome | Origem |
|---|---|
| `_el_N` | elemento/componente (`compile_view.dart:82-87`) |
| `_text_N` | texto imutável (`:98-104`) |
| `_textBinding_N` | texto ligado, `TextBinding()` (`:115-118`) |
| `_html_N` | fragmento i18n com HTML (`:90-95`) |
| `_anchor_N` | comentário-âncora de `<template>` (`:121-127`) |
| `_ngContent_N` | `<ng-content #ref>` (`:107-112`) |
| `_compView_N` | visão do filho (`:319-348`) |
| `_appEl_N` | `ViewContainer` (`:942-978`) |
| `_<Token>_N_M` | provedor/diretiva (`compile_element.dart:385`) |
| `_expr_N`, `currVal_N` | ligação checada (`property_binder.dart:290-294` do arquivo) |
| `subscription_N` | `@Output` de diretiva (`compile_view.dart:1080-1085`) |
| `_handleEvent_N` | handler de evento (`:726-735`) |
| `_message_N` | i18n (`:533`) |
| `_pipe_x_N`, `_pipe_x_N_M` | pipe, proxy puro (`compile_pipe.dart:45-47`, `:80`) |

**Regra (M de `_Token_N_M`).** `M = _instances.length` do `ProviderResolver`
no momento da criação (`ir/provider_resolver.dart:131-134`). Os embutidos
entram antes, nesta ordem: `ElementRef`, `Element`, `HtmlElement`, `Injector`
(`compile_element.dart:111-120`), `ViewContainer` se houver
(`:122-130`); em `beforeChildren`: `ViewContainerRef` se `hasViewContainer`,
`ChangeDetectorRef`, `ComponentLoader` se houver `ViewContainer`
(`:191-206`). Logo:

- elemento comum: primeira diretiva/componente = 5 (`_Foo_0_5`);
- `<template>` cuja diretiva pede `ViewContainerRef`: `TemplateRef` = 8,
  primeira diretiva = 9 (`_TemplateRef_0_8`, `_NgIf_0_9`);
- `<template>` sem `ViewContainerRef` (VC privado): `TemplateRef` = 7;
- componente filho que injeta `ViewContainerRef`: instância = 8.

O `TemplateRef` é inserido como **primeiro** provedor (`setEmbeddedView`,
`:160-189`).

**Rust.** Coberto nos casos listados — `visao.rs:5426-5464` (`_TemplateRef_n_8`,
`_…_n_9`), `molde` (`visao.rs:5593`, `k_tr` 7), `componente_filho`
(`campo_inst` 5/8) e `crate::diretivas::resolver_*`.

### 8.3 Local ou campo

**Regra.** Todo `NodeReference` nasce como local `final _el_N = …;`
(`WriteNodeReferenceStmt`, `compile_view.dart:233-252`) e só vira campo se lido
fora do escopo onde foi escrito (§4.2 item 4); então a escrita passa a
`this._el_N = …;` e as leituras a `this._el_N`
(`ReadNodeReferenceExpr :216-227`). Campo: `late final <Tipo> _el_N;`, com o
tipo de `identifierFromTagName` (elemento HTML) ou `HtmlElement` (componente)
(`view_builder.dart:240-253`).

**Rust.** Parcial — heurística equivalente (`liga_no_elemento`, `refs_locais`,
`campos_el_*` em `elemento_html`, `visao.rs:6666`); casos fora dela são
recusados.

---

## 9. Ordem das instruções no `build()`

**Regra** (`view_builder.dart:816-866`, `compile_view.dart:1378-1380`). O
`_createMethod` é uma lista única preenchida em três fases, e o `build()` é:

1. `final _ctx = this.ctx;` — só se `_ctx` é lido em algum ponto do corpo
   (`maybeCachedCtxDeclarationStatement`, `view_compiler_utils.dart:607-620`);
2. `final parentRenderNode = this.initViewRoot();` — só visão de componente;
3. **fase de construção**, ordem de documento: `final doc = importH.document;`
   (no primeiro elemento criado), criação de nós, atributos literais,
   `addShim*`, `ViewContainer`, `TemplateRef`, provedores/diretivas,
   `registerDirective` (devtools), `create`/`createAndProject` do filho;
4. **fase de ligação** (`bindView`): ouvintes nativos (`addEventListener`),
   `appViewUtils.eventManager.addEventListener`, `final subscription_N = ….listen(…)`,
   por elemento em ordem de documento (`view_binder.dart:89-115`,
   `event_binder.dart:8-35`);
5. **`afterNodes`** (`compile_view.dart:502-512`): criação dos pipes
   (e proxies), depois `updateQueryAtStartup` das consultas;
6. instrução final (§7.4): `initSubscriptions` / `initRootNode` /
   `initRootNodesAndSubscriptions`;
7. **só componente**: `@HostListener` do componente, `parentRenderNode.addEventListener(…)`
   (`_writeComponentHostEventListeners`, `view_builder.dart:845-852`, `:934-956`)
   — **depois** do `initSubscriptions`.

**Rust.** Parcial — `gerar_componente_com` (`visao.rs:10113`, variável
`linhas`) encadeia `linhas → ouvintes → pipes → consultas → hospedeiro →
initSubscriptions`: com `@HostListener` de componente **e** subscrições na
mesma visão, o Rust escreve os ouvintes do hospedeiro **antes** do
`initSubscriptions` e o oficial **depois** (divergência).

---

## 10. Criação de nós

### 10.1 Pai de renderização

**Regra** (`_getParentRenderNode`, `compile_view.dart:1484-1501`):

- nó raiz da visão de componente → `parentRenderNode`;
- raiz de embutida ou hospedeira → nenhum pai;
- filho de elemento que é componente (conteúdo projetado) → nenhum pai;
- senão → o nó do elemento pai (`_el_N` ou `this._el_N`).

### 10.2 Texto

**Regra** (`compile_view.dart:606-680`):

| Imutável | Com pai | Saída |
|---|---|---|
| sim | sim | `final _text_N = importD.appendText(pai, 'txt');` |
| sim | não | `final _text_N = importD.createText('txt');` |
| não | sim | `pai.append(this._textBinding_N.element);` + campo `final importT.TextBinding _textBinding_N = importT.TextBinding();` |
| não | não | nada no `build()` (a raiz é `this._textBinding_N.element`) |

A atualização (§14.3): `this._textBinding_N.updateText(⟨v⟩)` ou
`updateTextWithPrimitive(⟨v⟩)` se o valor é `bool/num/double/int`
(`update_statement_visitor.dart:193-205`).

**Rust.** Coberto — `Corpo::no` (`visao.rs:6516`) e `interpolacao`
(`visao.rs:6161`).

### 10.3 Elemento HTML

**Regra** (`_createElementAndAppend`, `compile_view.dart:774-827`):

- primeira criação da visão declara `final doc = importH.document;`
  (`:845-853`);
- com pai: `div` → `importD.appendDiv(doc, pai)`; `span` →
  `importD.appendSpan(doc, pai)`; outra tag →
  `importD.appendElement<importH.<Tipo>>(doc, pai, 'tag')` com o tipo de
  `identifierFromTagName` (`view_compiler_utils.dart:430-459`: tabela
  `a→AnchorElement`, …, `svg→SvgSvgElement` de `dart:svg`; senão
  `HtmlElement` se a tag está em `_htmlTagNames` (`:461-594`,
  **comparação sensível a maiúsculas**), senão `Element`);
- sem pai: `final _el_N = importU.unsafeCast(doc.createElement('tag'));`
  (sem argumento de tipo);
- `enableDataDebugSource`: `_el_N.setAttribute('data-debug-source', '<url>:<offset>')`
  (`:829-843`).

Exemplo: `<div><input></div>` na raiz do componente →

```dart
final doc = import7.document;
final _el_0 = import8.appendDiv(doc, parentRenderNode);
final _el_1 = import8.appendElement<import7.InputElement>(doc, _el_0, 'input');
```

**Rust.** Coberto — `elemento_html` (`visao.rs:6666`), `dom::tipo_da_tag`
(`dom.rs:169`). Diferenças: o Rust baixa a caixa da tag (`to_ascii_lowercase`)
e `tag_html` (`dom.rs:164`) também ignora caixa, enquanto
`detectHtmlElementFromTagName` do oficial não; `SvgSvgElement` sairia com o
prefixo de `dart:html` (nunca alcançado hoje, pois `svg` é recusado).
`data-debug-source`: Falta.

### 10.4 Elemento com namespace

**Regra.** Tag `@ns:nome` (ex.: `@svg:path`) → `doc.createElementNS('<uri>', 'nome')`
com `namespaceUris` (`xlink`, `svg`, `xhtml`) e `pai.append(_el_N)`
(`view_builder.dart:366-374`, `compile_view.dart:856-865`,
`view_compiler_utils.dart:21-25`).

**Rust.** Falta — tag que não é HTML nem casa diretiva é recusada
(`Corpo::no`, `visao.rs:6516`).

### 10.5 `<ng-container>`

**Regra.** Só visita os filhos com o mesmo pai; não cria nó nem índice
(`view_builder.dart:170-172`, `view_binder.dart:84-86`).

**Rust.** Coberto — `container` (`visao.rs:6642`); com atributo/`#ref`/`@i18n`
ou no conteúdo projetado: recusado.

### 10.6 `<ng-content>`

**Regra** (`projectNodesIntoElement`, `compile_view.dart:1041-1065`):

- com pai: `this.project(pai, <índice>);`
- raiz de embutida/hospedeira: a lista `this.projectedNodes[<índice>]` entra
  nas raízes (tipo lista: `createFlatArrayForProjectNodes` a concatena);
- dentro de conteúdo projetado para outro filho: entra na lista daquela
  projeção.

Com `#ref`: cria nó `_ngContent_N` e o provedor `NgContentRef(this, N)`
(`view_builder.dart:177-205`).

**Rust.** Coberto sem `#ref` (`Corpo::no`, braço `No::Conteudo`). `#ref`: Falta.

---

## 11. Atributos estáticos

### 11.1 Mescla e ordenação

**Regra** (`mergeHtmlAndDirectiveAttributes`, `view_compiler_utils.dart:291-355`):

1. atributos HTML escritos entram num mapa por nome (`class`, `class.x`,
   `style`, `tabIndex`, `@ns:nome` ou nome; `_nameOf :357-374`);
2. `hostAttributes` de cada diretiva do nó contam para `mergeCount`;
3. para cada diretiva, cada host attribute: se a diretiva é **componente** e
   não há mescla (`class`/`style` com mais de uma fonte), é pulado (o
   construtor da visão do filho o escreve); senão substitui o anterior, ou,
   para `class`/`style`, mescla numa interpolação `interpolate2('', a, ' ', b, '')`
   (`_mergeAttributeValue :381-410`);
4. o resultado sai **ordenado por nome** (`SplayTreeMap`, `:425-428`).

Precedência (baixa→alta): host attribute de componente, atributo HTML, host
attribute de diretiva.

**Rust.** Parcial — ordenação alfabética em `elemento_html`
(`visao.rs:6666`) e `componente_filho` (`visao.rs:3814`); mescla de `class`
com o host attribute do **filho** coberta (`interpolate2`); `style` mesclado
recusado; host attributes de **diretivas** (`@HostBinding` estático em
diretiva) recusados (`hospedeiro_estatico`, `diretivas.rs:248-296`).

### 11.2 Forma de escrita

**Regra** (`createAttributeStatements`, `compile_view.dart:1362-1376`, com
`appViewInstance = this`; `update_statement_visitor.dart`):

| Alvo | Saída |
|---|---|
| `class` | `this.updateChildClass(el, 'v')` (HTML) / `this.updateChildClassNonHtml(el, 'v')` (`:98-108`) |
| `class.x` | `importD.updateClassBinding(el, 'x', v)` / `…NonHtml` (`:109-118`) |
| `tabindex` | `el.tabIndex = N;` (não inteiro → `BuildError`) (`:165-191`) |
| atributo com namespace | `importD.updateAttributeNS(el, 'ns', 'n', v)` (`:75-82`) |
| outro atributo | `importD.setAttribute(el, 'n', 'v')`; se a fonte é anulável ou condicional → `updateAttribute` (`:84-94`) |

`isHtmlElement` = `detectHtmlElementFromTagName(tag)` — elemento de
componente com tag customizada → `NonHtml`.

Exemplo: `<div id="a" class="b">` → `this.updateChildClass(_el_0, 'b');`
depois `import8.setAttribute(_el_0, 'id', 'a');` (ordem alfabética).

**Rust.** Coberto — `elemento_html` e `componente_filho`.

### 11.3 Ordem na criação do elemento

**Regra.** HTML: criação → atributos literais → `addShim*` → (`CompileElement`)
`ViewContainer` → diretivas (`view_builder.dart:357-413`). Componente: criação
da visão e `rootElement` (+ `append`) → atributos (não na hospedeira) →
`addShimC` → `ViewContainer` → filhos/provedores → `create`
(`:275-355`).

**Rust.** Coberto.

### 11.4 Shim de CSS

**Regra** (`shimCssForNode`, `compile_view.dart:1067-1078`): com
encapsulamento emulado, e fora do nó raiz da hospedeira,
`this.addShimE(el)` se o tipo do nó é `Element`, senão `this.addShimC(el)`;
elemento de componente usa `HtmlElement` → sempre `addShimC`.

**Rust.** Coberto — `elemento_html` e `componente_filho` (`com_estilo`).

---

## 12. Componentes filhos e projeção

### 12.1 Criação

**Regra** (`compile_view.dart:872-919`, `:980-1011`):

```dart
// campo: late final importT.ViewY0 _compView_N;   (sem import se no mesmo arquivo)
this._compView_N = importT.ViewY0(this, N);
final _el_N = this._compView_N.rootElement;
pai.append(_el_N);          // se houver pai de renderização
```

Tipo do campo com argumentos de tipo de `Typed` (`lookupTypeArgumentsOf`,
`:1523-1569`).

**Rust.** Coberto sem `Typed` — `componente_filho` (`visao.rs:3814`).

### 12.2 `create` / `createAndProject`

**Regra** (`view_builder.dart:342-354`, `compile_view.dart:1013-1026`):
`contentNodesByNgContentIndex` tem uma lista por `ngContentSelectors` do filho
(`compile_element.dart:135-140`); cada lista passa por
`createFlatArrayForProjectNodes` (`view_compiler_utils.dart:209-264`):

- vazia → `const <Object>[]`;
- um item que já é lista (`projectedNodes[i]`) → ele mesmo;
- um item → `<Object>[x]`;
- misto → `<Object>[a, b]..addAll(importU.unsafeCast(lista))…`.

Lista externa vazia (filho sem `<ng-content>`) →
`this._compView_N.create(this._Y_N_5);`; senão
`this._compView_N.createAndProject(this._Y_N_5, [<listas>]);`.

O nó projetado vai para o **menor** índice cujo seletor casa, senão o `*`
(`findNgContentIndex`, no parser); o nó é criado sem pai
(`doc.createElement`, `createText`, `createAnchor`).

**Rust.** Coberto — `componente_filho` (`visao.rs:3814`, trecho após
`// Conteúdo projetado`), `lista_plana` (`visao.rs:1928`).

### 12.3 Detecção e destruição do filho

**Regra.** Na detecção: `this._compView_N.detectChanges();` (bloco de
visões-filhas, §14.1); com `@HostBinding` no filho:
`this._compView_N.detectHostChanges(firstCheck);` no bloco de propriedades
(`property_binder.dart:389-435` do arquivo, `bindDirectiveHostProps`). Na
destruição: `this._compView_N.destroyInternalState();`.

**Rust.** Coberto.

---

## 13. `<template>`, `*`, `ViewContainer`, `TemplateRef`

### 13.1 Âncora e `ViewContainer`

**Regra** (`compile_view.dart:923-978`, `compile_element.dart:122-130`):

```dart
final _anchor_N = importD.appendAnchor(pai);        // sem pai: importD.createAnchor()
this._appEl_N = ViewContainer(N, <índice do pai|null>, this, _anchor_N);
```

- `ViewContainer` sem prefixo (allowlist); campo
  `late final ViewContainer _appEl_N;`.
- 2º argumento: `null` se o elemento é raiz da visão, senão o índice do
  `CompileElement` pai (o componente, no conteúdo projetado).
- Só entra em `viewContainers` (detecção/destruição) se **não privado**,
  i.e. `hasViewContainer` (alguma diretiva pede `ViewContainerRef`).
- Elemento HTML ou componente com diretiva que pede `ViewContainerRef` também
  ganha `_appEl_N` sobre o próprio `_el_N` (raiz: o `_appEl_N`).

### 13.2 `TemplateRef`

**Regra** (`compile_element.dart:160-189`, `compile_view.dart:1134-1283`):
provedor ansioso; se referenciado fora do `build()` (`#ref` com
`TemplateRef` ou consulta) → campo `late final TemplateRef _TemplateRef_N_M;`
e `this._TemplateRef_N_M = TemplateRef(this._appEl_N, viewFactory_Xk);`;
senão local `var _TemplateRef_N_M = TemplateRef(this._appEl_N, viewFactory_Xk);`
(`toDeclStmt()` sem tipo → `var`). Componente genérico: a fábrica vira
closure `(parentView, parentIndex) { return viewFactory_Xk<T>(parentView, parentIndex); }`
(`getViewFactory`, `view_compiler_utils.dart:167-184`).

### 13.3 Diretivas no `<template>`

**Regra** (`view_binder.dart:118-131`): entradas, ganchos de detecção,
saídas e **ganchos pós-filhos** (AfterContent/AfterView/OnDestroy) são ligados
logo, sem esperar filhos; não há ligação de propriedades do elemento nem
`detectHostChanges`. Depois, `bindView` da embutida.

### 13.4 `NgIf` / `NgFor`

**Regra.** Não há ramo especial no compilador de visões; os casos especiais
são:

- `NgIf.ngIf`: tipo `bool` e ligação **direta** (`isDirect`) — sem
  `_expr_N`/`checkBinding`: `this._NgIf_N_9.ngIf = ⟨v⟩;` a cada detecção
  (`C/semantic_analysis/binding_converter.dart:164-184`; `_directBinding`,
  `property_binder.dart:148-166` do arquivo); imutável → no `if (firstCheck)`;
- `NgFor`: tipagem dos locais (§7.3);
- `ng_if.dart` na allowlist (sem prefixo); `ng_for.dart` com prefixo.

Exemplo `*ngIf="ok"` na raiz:

```dart
final _anchor_0 = import9.appendAnchor(parentRenderNode);
this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_X1);
this._NgIf_0_9 = NgIf(this._appEl_0, _TemplateRef_0_8);
if (import10.isDevToolsEnabled) {
  import10.Inspector.instance.registerDirective(_anchor_0, this._NgIf_0_9);
}
```

**Rust.** Parcial — `estrutural` (`visao.rs:5348`) reconhece só
`ngIf`/`ngFor`/`ngSwitch*`/`ngTemplateOutlet` (`Estrutural::conhecida`,
`visao.rs:9202`); `*` de outra diretiva é recusado. `<template>` escrito:
`molde` (`visao.rs:5593`), com diretivas por `molde_com_diretiva`
(`visao.rs:1615`).

---

## 14. `detectChangesInternal`

### 14.1 Ordem

**Regra** (`writeChangeDetectionStatements`, `compile_view.dart:1396-1464`).
Método omitido se todos os blocos estão vazios, sem visões-filhas (exceto
hospedeira) e sem `viewContainers`. Senão:

1. `final _ctx = this.ctx;` (se lido), `bool changed = false;` (se lido ou
   escrito), `bool firstCheck = this.firstCheck;` (se lido);
2. declarações de locais (`final local_x = …;`);
3. `detectChangesInInputsMethod`: por elemento em ordem de documento, por
   diretiva: entradas → `ngAfterChanges`/`ngOnInit`/`ngDoCheck`;
4. `this._appEl_N.detectChangesInNestedViews();` para cada `viewContainers`;
5. `if ((!importK.debugThrowIfChanged)) { ⟨atualização de consultas de conteúdo⟩ ⟨AfterContent*⟩ }`;
6. `detectChangesRenderPropertiesMethod`: textos ligados, propriedades do
   elemento, `detectHostChanges` de diretivas/filhos;
7. `this._compView_N.detectChanges();` para cada visão-filha;
8. `if ((!importK.debugThrowIfChanged)) { ⟨consultas de visão⟩ ⟨AfterView*⟩ }`.

**Rust.** Coberto — fim de `gerar_componente_com` (`linhas_deteccao`) e
`corpo_da_embutida` (`linhas_det`), com `sem_lancar` (`visao.rs:8953`).

### 14.2 Ciclo de vida

**Regra** (`VC/lifecycle_binder.dart`):

- `ngAfterChanges`: `if (changed) { d.ngAfterChanges(); }` (`:23-30`);
- `ngOnInit`: `if (((!importK.debugThrowIfChanged) && firstCheck)) { d.ngOnInit(); }` (`:32-43`);
- `ngDoCheck`: `if ((!importK.debugThrowIfChanged)) { d.ngDoCheck(); }` (`:45-51`);
- `ngAfterContentInit`/`ngAfterViewInit`: dentro de `if (firstCheck) {…}`,
  **reaproveitando** o bloco anterior se a última instrução do método já é um
  `if (firstCheck)` (`addStmtsIfFirstCheck`, `compile_method.dart:23-36`);
  `…Checked` fora dele (`lifecycle_binder.dart:62-96`);
- ganchos pós-filhos são ligados **depois** dos filhos do elemento (ordem
  de baixo para cima, `view_binder.dart:109-114`);
- `ngOnDestroy` no `destroyInternal` (`lifecycle_binder.dart:99-107`); pipes
  com `OnDestroy` depois das diretivas (`view_binder.dart:44-46`).

`importK` = `check_binding.dart` (`debugThrowIfChanged`, `identifiers.dart:143`).

**Rust.** Coberto — `ciclo_de_vida` (`visao.rs:7500`, hospedeira),
`ganchos_depois_dos_filhos` (`visao.rs:4942`), `depois_dos_filhos`
(`visao.rs:4980`), `entradas_de` (`visao.rs:5056`), `na_primeira_checagem`
(`visao.rs:8976`).

### 14.3 Ligações (`bindAndWriteToRenderer`)

**Regra** (`VC/property_binder.dart`, linhas do arquivo):

- constantes (fonte imutável) juntam-se num `if (firstCheck) {…}`
  (reaproveitado, `:25-70`); dinâmicas depois;
- ligação direta (`isDirect`): só a ação (`_directBinding :148-166`);
- checada (`_checkBinding :176-230`, `_bind :301-348`):

```dart
final currVal_K = ⟨expr⟩;
if (importC.checkBinding(this._expr_K, currVal_K, '<fonte>', '<local>')) {
  ⟨ação⟩
  this._expr_K = currVal_K;
}
```

  com campo `Object? _expr_K;` e `K` do contador da visão
  (`view_name_resolver.dart:93`); fonte imutável → `_bindLiteral`
  (`:357-384`): ação com `currVal_K` substituído pela expressão, dentro de
  `if ((⟨expr⟩ != null)) {…}` se anulável, nada se a expressão é `null`;
  interpolação de um só primitivo checa o valor e interpola na ação
  (`:453-478`).
- entradas de diretiva: `calcChanged` (componente `onPush` ou
  `AfterChanges`) → `changed = false;` antes (não na hospedeira) e
  `changed = true;` em cada ação; componente `onPush`:
  `if (changed) { this._compView_N.markAsCheckOnce(); }` (`:108-146`).

**Rust.** Coberto — `propriedade` (`visao.rs:3483`), `escrever_ligacoes`
(`visao.rs:3529`), `entradas_de` (`visao.rs:5056`).

### 14.4 `detectHostChanges`

**Regra.** Visão de componente com `@HostBinding` do próprio componente:
método `void detectHostChanges(bool firstCheck)` com `final _ctx = this.ctx;`
(se lido) e as ligações de `bindAndWriteToRenderer` sobre `rootElement` com
`isHtmlElement` do elemento raiz (`view_binder.dart:173-212`,
`view_builder.dart:558-571`). Quem o chama: o pai
(`this._compView_N.detectHostChanges(firstCheck)`) e a hospedeira
(`this.componentView.detectHostChanges(firstCheck)`). Diretiva com
`@HostBinding`: `this._Dir_N_M.detectHostChanges(<visão>, el)` no
detector `NgCd` (`property_binder.dart:389-435`).

**Rust.** Coberto — `gerar_componente_com` (`host_changes`),
`ligar_diretivas` (`visao.rs:7378`), `classe_ngcd` (`visao.rs:9686`).

---

## 15. `destroyInternal`

**Regra** (`view_builder.dart:709-721`): `this._appEl_N.destroyNestedViews();`
por `viewContainer`; `this._compView_N.destroyInternalState();` por
visão-filha (não na hospedeira); `destroyMethod` (`ngOnDestroy` de diretivas,
depois pipes). Omitido se vazio.

**Rust.** Coberto — `gerar_componente_com`, `corpo_da_embutida`,
`ciclo_de_vida`.

---

## 16. Componentes genéricos

**Regra.** Os parâmetros de tipo da classe (`originType.typeParameters`)
entram em: `ViewX0<T…>`, `_ViewXk<T…>`, `_ViewXHost0<T…>`,
`viewFactory_Xk<T…>`, `viewFactory_XHost0<T…>`, `createXFactory<T…>`; o limite
é escrito `T extends Limite` (omitido se `dynamic`,
`dart_emitter.dart:344-363`); os argumentos `<T…>` entram no tipo de contexto
(`contextType`) e no `return _ViewXk<T>(…)`; `_XNgFactory` e o getter usam o
tipo cru.

**Rust.** Coberto — `parametros_de_tipo` (`visao.rs:11151`),
`fabrica_do_molde` (`visao.rs:5843`). `Typed` para diretivas/filhos
genéricos: Falta (`metadados.rs:279`).

---

## 17. OnPush

**Regra.**

- Visão de componente `onPush` → `super(…, ChangeDetectionCheckedState.checkOnce)`
  (§4.3);
- entradas de filho `onPush` → `if (changed) markAsCheckOnce()` (§14.3);
- `ChangeDetectorRef` do componente `onPush` é a visão
  (`compile_element.dart:393-398`), usado pelas consultas
  (`queryChangeDetectorRefs`);
- componente que é "link" de detecção (`isChangeDetectionLink`) →
  `detectChangesInCheckAlwaysViews()` com `detectChangesInCheckAlwaysViews`
  dos `viewContainers` e dos filhos que também são link
  (`compile_view.dart:1382-1394`).

**Rust.** Coberto nos três primeiros itens (`ciclo_de_vida`, `entradas_de`,
`consultas_de_conteudo_no` `visao.rs:4808`); `detectChangesInCheckAlwaysViews`:
Falta.

---

## 18. `@i18n`

**Regra** (`createI18nMessage`, `compile_view.dart:519-593`), mensagens
deduplicadas por visão, nomeadas `_message_<ordinal>`:

- sem HTML: campo `static final String _message_N = importI.Intl.message('texto', desc: '…'[, locale: …][, meaning: …][, skip: true]);`
  (lido como `_message_N`);
- com HTML: método `static String _message_N(String startTag0, …) { return importI.Intl.message('…', desc: …, name: 'ViewX0__message_N', args: [...], examples: const {...}); }`
  e nó `final _html_N = importA.createTrustedHtml(_message_N('<b>', …));`
  + `pai.append(_html_N)` (`:595-604`); o texto é reescapado com
  `HtmlEscapeMode.element` (`:683-698`).

**Rust.** Parcial — `mensagem` (`visao.rs:3207`), `filhos_i18n`
(`visao.rs:7102`), `filhos_i18n_html` (`visao.rs:3237`); recusa `@i18n` com
filho/diretiva/`*` na mesma visão, em componente filho, e HTML + handler de
evento na mesma visão.

---

## Lacunas do porte

Por prioridade (impacto em arquivos reais × risco de divergência silenciosa):

1. **Ordem `initSubscriptions` × `@HostListener` do componente** (§9):
   o Rust escreve os ouvintes do hospedeiro antes do `initSubscriptions`; o
   oficial, depois (`view_builder.dart:840-852`). Divergência silenciosa de
   bytes — corrigir em `gerar_componente_com` (`visao.rs:10113`), movendo
   `.chain(&hospedeiro)` para depois do `initSubscriptions`.
2. **`*` de diretiva estrutural arbitrária** (§13.4): só as conhecidas saem
   (`Estrutural::conhecida`); o oficial é genérico
   (`visitEmbeddedTemplate` + provedores). Maior fonte de recusa esperada.
3. **Host attributes de diretivas** e `style` mesclado (§11.1): recusados;
   exigem `_mergeHtmlAndDirectiveAttrs` completo (precedência, contagem,
   interpolação `class`/`style`).
4. **Elementos com namespace (SVG)** (§10.4): `createElementNS` + `append`;
   hoje recusados. Ao implementar, `SvgSvgElement` precisa vir de `dart:svg`,
   não do prefixo de `dart:html` (`dom.rs:160`).
5. **`Typed` / `lookupTypeArgumentsOf`** (§12.1, §16): argumentos de tipo em
   campos de filhos e diretivas genéricas.
6. **`detectChangesInCheckAlwaysViews`** (§4.5, §17): componentes
   `isChangeDetectionLink`.
7. **`<ng-content #ref>`** (§8.1, §10.6): nó `_ngContent_N`, consumo de índice e
   provedor `NgContentRef`.
8. **Caixa de tag** (§10.3): `detectHtmlElementFromTagName` é sensível a
   maiúsculas; o Rust normaliza (`dom.rs:164-178`, `elemento_html`). Afeta tipo
   e `updateChildClass` × `NonHtml` em tags escritas com maiúsculas.
9. **`@i18n` com filho/diretiva/`*` na mesma visão** (§4.2, §18): falta
   intercalar os campos `_message_N` na ordem de alocação geral.
10. **`export` do fonte** (§2.1): flag `exportUserCodeFromTemplate` não
    tratada em `montar_arquivo`.
11. **`data-debug-source`** (§10.3): só com `enableDataDebugSource`; baixa
    prioridade (desligado por padrão).
