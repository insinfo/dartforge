# Casos do ngcomponents: regras oficiais e porte

Cada recusa do placar do ngcomponents 3.0.0-dev.1 foi levada ao código do
ngcompiler 3.0.0-dev.3 (`lib/v1/src/compiler/`) antes de mexer no Rust. Aqui
fica a regra oficial que decide cada caso, onde ela está e o que o porte faz.
As seções gerais (01–07) descrevem o compilador inteiro; esta registra os
casos que apareceram no ngcomponents.

## Atributo interpolado com prefixo (`attr.x="{{..}}"`) — j107

- **Regra:** `_createPropertyForAttribute` (`template_parser/ast_template_parser.dart:348-375`)
  manda o nome do atributo para o mesmo `createElementPropertyAst` de `[x]`,
  com o valor `Interpolation`. `attr.x`, `class.x`, `style.x` e propriedade
  simples são as ligações de sempre.
- **Valor:** a `Interpolation` nunca é nula (`canBeNull` falso: sai
  `setAttribute`, não `updateAttribute`) e o `_TypeResolver` não a tipa
  (`dynamic`: `[style.x.px]` ganha `.toString()`).
- **Porte:** `visao.rs` `atributo_interpolado`. Sem prefixo, nomes que o esquema
  renomeia (`readonly`, `tabindex`, `for`, `formaction`) seguem recusados até
  a tabela completa do esquema (seção 02).

## `@SkipSelf` para token do próprio nó ("dependência cíclica") — j108

- **Regra:** `_getDependency` só consulta o nó se `!dep.isSkipSelf`
  (`provider_parser.dart:325`); `_getLocalDependency` devolve `@Attribute`,
  `Injector` e os embutidos do elemento sem criar provedor (`:266-301`). Na
  emissão, `provider_resolver.dart:189-204` sobe pelos pais e cai no injetor
  (`injectorGet`/`injectorGetOptional`).
- **Casos:** `tooltipControllerBinding` (`FactoryProvider` com
  `[TooltipController, Optional(), SkipSelf()]`), `ModalComponent`
  (`@Optional() @SkipSelf() Modal`), `MaterialStackableDrawerComponent`,
  `MaterialDateRangePickerComponent`, `MaterialPopupComponent`.
- **Porte:** `diretivas.rs` `criar` só segue as arestas locais (`local`); o
  fecho `dependencia` aceita `pular` indo direto a `de_fora`. `@Self`/`@Host`
  em dependência de provedor continuam recusados.

## `#x="valor"` no elemento de um filho — j109

- **Regra:** `identifierForReference` (`ast_template_parser.dart:854-868`):
  sem valor, o componente; com valor, a primeira diretiva ligada com
  `exportAs == valor`. `compile_element.dart:228-238` registra
  `addLocal(nome, _providers.get(token).build())`; `view_name_resolver.dart`
  lê de visões acima com `getPropertyInView`
  (`unsafeCast<ViewX0>((this.parentView!))._Dir_n_k`).
- `buildChangeDetectorRef()` só existe para componente `onPush`: um `#ref`
  que vale uma diretiva não gera `queryChangeDetectorRefs`.
- `@ViewChild('x')` estático de um `#ref` com valor num filho atribui a
  instância da diretiva no fim do `build()`.
- Numa visão embutida, um `#ref` que nenhuma consulta dinâmica alcança é só
  um nome: a consulta estática (primeiro resultado fora de `*`,
  `compile_query.dart:128-150`) não chega à embutida.
- **Porte:** `visao.rs` `componente_filho` (alvo por `exportAs`,
  `refs_ao_componente`), `formas_contra_o_template` (aceita `NoFilho`) e
  `emitir_embutida` (`consultados_aqui`).

## Componente `onPush` com `@Input` herdado — j110

- **Regra:** `hasInputs = directive.inputs.isNotEmpty`
  (`semantic_analysis/matched_directive_converter.dart:72`), com as entradas
  herdadas (`_collectInheritableMetadata`, `find_components.dart:595-621`);
  `bindDirectiveInputs` (`view_compiler/property_binder.dart:108-146`) escreve
  `if (changed) { componentView.markAsCheckOnce(); }` na hospedeira.
- **Porte:** `visao.rs` `gerar_componente_com` (`marca` conta as entradas dos
  metadados de quem herda).

## Seção 03 aplicada — j112, j113, j114

- `exports:` é resolvido no parse, antes do local de template de mesmo nome;
  `StaticRead` é sempre imutável; `Classe.metodo` é imutável; `-x` é `(0 - x)`.
- Três ou mais `{{ }}` num valor: `interpolateN([t0, e0, …, tn])`.
- `_attrToPropMap`: `[tabindex]`/`[readonly]` e `tabindex="{{..}}"` são
  `setProperty` com o nome mapeado; `style="{{..}}"` é a propriedade `style`.
- A `Interpolation` é `String` para o `_TypeResolver`, exceto no atalho
  primitivo, em que vale o tipo da expressão crua (`calc({{100-x}}%)` do
  `material_slider` × `{{int}}` no j107).
- A visão embutida refaz a emissão sem o `TextBinding` quando nenhuma
  interpolação dela é mutável, como a do componente.

## `preserveWhitespace: true` — j115

- **Regra:** `ast_template_parser.dart:155-164` troca o `MinimizeWhitespaceVisitor`
  pelo `_PreserveWhitespaceVisitor` (só `&ngsp;` → espaço, em todo texto), e
  `expression_converter.dart:151,161` não comprime as pontas das interpolações.
- **Porte:** `componente.rs` (`preservar_espacos`), `html::analisar_no_modo`,
  `Corpo::preservar_espacos` em `valor_interpolado`.
- Ao sair a recusa, apareceu no `paper_tooltip` a lacuna L1 da seção 05 (provedor
  preguiçoso do pai pedido por um nó abaixo). Ela é recusada exatamente
  (`ProvedorAcima::preguicoso`) até a simulação do `ProviderElementContext`.

## Provedor do filho pedido pelo conteúdo, com a numeração certa — j116

- **Regra:** o `_getDependency` da diretiva do conteúdo sobe e transforma o
  provedor preguiçoso do filho durante a visita dela, com o `eager` dela,
  antes do `afterElement` do filho (`provider_parser.dart:318-366`). O índice
  (`uniqueId`) é a posição nessa ordem (`_PopupRef_0_9`, e não `_0_12`).
- **Porte:** `resolver_no_do_filho(.., pedidos)`: os pedidos do conteúdo
  (`pedidos_ao_no_do_filho`) são criados logo depois da passada ansiosa. O
  reordenamento posterior (`ansiosos_pelo_conteudo`), que mantinha os nomes
  antigos, saiu.

## `@ViewChild(Diretiva)` — j117

- **Regra:** o token da diretiva está no `_resolvedProvidersArray` do elemento;
  `_getQueriesFor` acha a consulta de visão, e o valor é
  `_providers.get(tipo).build()` (o campo; `.instance` numa `XNgCd`), atribuído
  no fim do `build()` quando a consulta é estática (`compile_element.dart:191-278`).
- **Porte:** `formas_contra_o_template` aceita a diretiva de `usadas` casada por
  seletor em elemento da própria visão; o nó registra cada diretiva sob a
  `chave_de_tipo`, antes dos `#ref`.

## `hostAttributes` de diretiva, `exportAs` de componente, `class.x.y` — j118, j119

- `_computeHostBindingImmutability` (`compile_metadata.dart:570-600`): o
  `@HostBinding` em estático imutável fora de `class.x`/`style.x` é
  `hostAttribute` e fica fora da `XNgCd` (sem consumir índice). Sem
  `hostProperties`, não há `XNgCd`. Quem usa a diretiva ainda é recusado (a
  mescla no elemento, `mergeHtmlAndDirectiveAttributes`, falta para diretivas).
- `exportAs:` em `@Component` só decide a quem aponta o `#x="nome"`.
- `class.x.y` (host e template): a classe é `parts[1]`, o resto some
  (`template_parser.dart:100-102`).

## Projeção concatenada, escrita em setter, `ChangeDetectorRef` do filho — j120

- **Projeção:** `createFlatArrayForProjectNodes` concatena as listas com
  `..addAll(..)` quantas vezes for preciso; o dart_style quebra a cascata uma
  seção por linha, recuada dois espaços além do literal da cabeça, no
  `createAndProject` e no `initRootNodesAndSubscriptions` (que então quebra os
  dois argumentos) — `modal`, `dropdown_menu`, `icon_tooltip`.
- **Escrita:** o `PropertyWrite` de receptor implícito vira `_ctx.x` para campo
  herdado e para `set x(..)` sem getter (`material_popup`,
  `(focus)="visible = false"`). O porte procura `x_=` na hierarquia
  (`Resolucao::tem_setter`).
- **`ChangeDetectorRef`:** `beforeChildren` registra
  `componentView ?? o.thisExpr` (`compile_element.dart:199`): uma diretiva no
  elemento de um filho recebe `this._compView_n`. Porte: `criar_instancias(.., detector)`.
- **Ordem dos campos:** o emissor escreve primeiro os campos com inicializador
  (`dart_emitter.dart:198-205`): na hospedeira, os provedores preguiçosos antes
  do `_appEl_0`.

## `read: ViewContainerRef` em `<template>`, `ComponentLoader`, `@changeDetectionLink` — j121

- **Regra:** `_addQueryReadsTo` dos `#ref` do nó (`provider_parser.dart:95-104`):
  uma consulta que lê `ViewContainerRef` liga o `_requiresViewContainer`. O
  `ViewContainer` fica público (`detectChangesInNestedViews`/
  `destroyNestedViews`), o `TemplateRef` passa ao índice 8 e o valor atribuído
  no `build()` é `this._appEl_n`.
- **`ComponentLoader`:** também liga o `_requiresViewContainer`
  (`provider_parser.dart:290-295`) e é o mesmo `appViewContainer`
  (`compile_element.dart:204-206`) — no construtor do componente, do filho ou
  de diretiva.
- **`@changeDetectionLink`:** toda visão não hospedeira do componente ganha
  `detectChangesInCheckAlwaysViews` entre o `injectorGetInternal` e o
  `detectChangesInternal`, com cada `ViewContainer` público e depois cada filho
  também ligado (`view_builder.dart:522`, `compile_view.dart:1383-1395`); vazio,
  o método não sai.
- **Porte:** `le_container`, `Contexto::moldes_com_container`,
  `resolver_de_molde(.., forcar_container, ..)`, `metodo_de_link`.

## Ordem dos campos de nó — j122

- **Regra:** os nós viram campo quando o `NodeReferenceStorageVisitor` os acha
  fora do escopo do `build()`, método a método (`view_builder.dart:570-579`).
  No `detectChangesInternal` as declarações dos locais vêm no topo, então o nó
  de um `#ref` lido na detecção é promovido antes dos nós das ligações
  (`_el_8` antes de `_el_1` no `material_stepper`).
- **Porte:** `campos_el_em_ordem` (pelos `locais_raiz`).

## Embutidos no nó de um filho, `[attr.x.if]`, `[attr.ns:x]` — j123, j124

- Diretiva no elemento de um filho: `ViewContainerRef`/`ComponentLoader` são o
  `_appEl_n` do nó, `ChangeDetectorRef` é a `_compView_n`, `HtmlElement` o nó
  (j123, o `MaterialTooltipDirective` do `material_menu`).
- **`attr.x.if`:** `unit == 'if'` (`template_parser.dart:83-88`) vira
  `isConditional` (`binding_converter.dart:147`): o valor é `(v ? '' : null)`
  e sai sempre `updateAttribute` (`update_statement_visitor.dart:63-94`).
  Outra unidade é erro; `class.if` também.
- **`attr.ns:x`:** o prefixo vira a URI de `namespaceUris` (`xlink`, `svg`,
  `xhtml`, `view_compiler_utils.dart:21-25`) e sai `updateAttributeNS`; fora
  da tabela, `null`, e o atributo sai sem namespace (`ir/model.dart:396`).

## Locais de `*ngFor` em visões aninhadas e o tipo do `$implicit` — j125

- **Leitura:** o `getLocal` do `ViewNameResolver` sobe pelas
  `declarationElement.view` e lê
  `unsafeCast<_ViewX1>((this.parentView!)).locals['\$implicit']`, com o
  `unsafeCast<T>` do tipo do local (`view_name_resolver.dart:42-70`).
- **Tipo:** o retorno do getter `single` no tipo da coleção
  (`getIterableElementType`, `analyzed_class.dart:39-42`): o analyzer
  instancia o receptor e substitui os parâmetros subindo pela hierarquia
  (`MenuItemGroup<T>` → `LabeledList<T>` → `DelegatingList<T>`;
  `ListBase<E>` → `ListMixin<E>`). Tipo cru é instanciado pelos limites e o
  `fromDartType` escreve os argumentos (`J125Grupo<dynamic>`).
- **Porte:** `Resolvedor::membro_da_classe_com`/`membro_com_argumentos`
  (substituição textual, só com argumentos que se escrevem igual em qualquer
  escopo), `instanciar_crus`. A coleta (`coletar_abaixo`) passou a estender os
  `locais_proprios`, o que mostrou as causas reais dos pendentes que citavam
  "local ancestral".

## `directiveTypes:`, o `T` do componente e o `_TypeResolver` — j126, j127

- **`directiveTypes:`** (`Typed<X>.of([#T])`, `Typed<X<A>>(on: 'ref')`): o
  `lookupTypeArgumentsOf` (`compile_view.dart:1523-1569`) tipa só os campos —
  `_compView_n` (`ViewX0<T>`), a instância e o campo da diretiva (ou o
  `XNgCd`) —, nunca a criação. `on:` vale só no nó com aquele `#ref`; sem
  `on:`, o primeiro. `#T` é o parâmetro do componente (`fromTypeLink`). Os
  imports dos argumentos vêm logo depois do da classe, na ordem do campo.
- **O `T` do componente é livre:** as visões são genéricas, então um
  argumento que é parâmetro do próprio componente entra como está na
  substituição pela hierarquia (`J127ComOpcoes<T>` → `J127Opcoes<T>` →
  `J127Grupo<T>`; `extends J127No<T?>` → `J127Grupo<T?>`). O receptor
  implícito é `Classe<T>`.
- **`_TypeResolver`** (`analyzed_class.dart:232-370`): o tipo de `m(args)` é
  o retorno de `m` no receptor, sem olhar os argumentos; um nome fora das
  `_variables` cai no membro, ou em `dynamic`. Na retipagem da coleção do
  `NgFor` só sai do escopo o local sem tipo que um membro de mesmo nome
  substitui (antes, a conversão falhava no argumento e o tipo virava
  `dynamic` em silêncio, no `material_tree_group`).
- **Retorno que cita parâmetro não resolvido:** `Metodo::incerto`, tipo
  desconhecido — nunca `dynamic`.
- **Consulta de conteúdo com token fornecido por outra diretiva** (o
  `FocusableItem` do `FocusItemDirective`, por `ExistingProvider`): a checagem
  de resultado em `*` olha os `providers:` do nó (`_getQueriesFor`), o que
  impediu um `listItems = []` errado no `material_select` (a consulta
  dinâmica de conteúdo ainda é recusada).

## Consulta de conteúdo dinâmica — j128

- **Regra** (`compile_query.dart`, `compile_element.dart:354-362, 442-460`):
  `_getQueriesFor` sobe pelos elementos a partir do resultado (atravessando
  as visões embutidas); sem `descendants`, só distância ≤ 1 — e o `<template>`
  de um `*`, que tem diretiva, já soma 1. `@ContentChild` é sempre
  `descendants: true`. O token casa pelos provedores resolvidos do nó (a
  diretiva, o componente e os `providers:` deles, `ExistingProvider`).
- **Com resultado em `*`** (`_shouldMapNestedViews`): campo
  `bool _query_<Token>_<nó>_<i>_isDirty = true;` (`i` é o `_queryCount` do
  nó); a atualização vai no `_updateContentQueriesMethod`, no `afterChildren`
  do nó — antes das consultas de visão, que entram no `afterNodes`
  (`compile_view.dart:502-511`), e dos ganchos de conteúdo; o valor é o
  `_buildQueryResults`/`_mapNestedViews` (`nestedView._X_n_m.instance`,
  `...` quando há mais de um item; a única pega `.first` ou `firstOrNull`).
  Cada visão embutida com resultado marca o campo no
  `dirtyParentQueriesInternal` (`unsafeCast<Visão da consulta>(cadeia)`), na
  ordem do primeiro resultado de cada consulta naquela visão (no mesmo nó, a
  de conteúdo antes da de visão).
- **Campos com inicializador** (provedor preguiçoso, sujo de conteúdo, sujo
  de visão): na ordem de alocação — a posição do nó do preguiçoso, a do
  primeiro resultado em `*` do sujo (pré-ordem do template, as embutidas no
  lugar).
- **Porte:** `arvore_de_conteudo`, `conteudo_dinamico_no`,
  `resolver_conteudo_dinamico`, `campos_com_inicializador`; cada nó registra
  pela posição (`chave_de_no`) os tokens que fornece.

## Pedido que atravessa `<template>` e filho projetado em consulta — j130, j131

- **Regra** (`provider_parser.dart:318-345`): ao subir de um nó que é raiz de
  visão embutida (`_isViewRoot`), `currEager` vira `false`: o provedor do nó
  de cima é transformado na visita de quem pede (muda de posição — logo de
  índice), mas segue preguiçoso (`late dynamic _X_n_m = ..`), e quem pede lê
  o campo pela cadeia de `parentView`. Um nó que pede vários tokens pede na
  ordem em que cria os próprios provedores (cada diretiva ansiosa, as
  dependências dela em ordem). As diretivas de um `<template>` escrito pedem
  desta visão; o conteúdo dele atravessa.
- **Porte:** `pedidos_do_conteudo(.., ansioso)`, `resolver_com` com pedidos
  `(token, ansioso)` (a preguiça é por provedor, a posição é a da visita),
  `ProvedorAcima::leitura_preguicosa` (só para dono de nó de filho, cujos
  pedidos o porte modela), `ordem_dos_pedidos_do_no`.
- **Consulta de visão/conteúdo com filho no conteúdo projetado de outro**
  (`NoFilhoProjetado`): também é resultado (a instância, criada nesta
  visão), com o `ChangeDetectorRef` do `onPush` e a marca no
  `dirtyParentQueriesInternal` da embutida.

## Pendentes com regra já levantada
