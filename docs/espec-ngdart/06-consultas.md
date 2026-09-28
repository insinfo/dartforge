# 06 — Consultas (`@ViewChild[ren]`, `@ContentChild[ren]`)

Especificação derivada da leitura do `ngcompiler-3.0.0-dev.3` (caminhos relativos a
`lib/v1/src/`) e do runtime `ngdart-8.0.0-dev.4`. Abreviações usadas nas citações:

| Abreviação | Arquivo |
|---|---|
| `CQ` | `compiler/view_compiler/compile_query.dart` |
| `CE` | `compiler/view_compiler/compile_element.dart` |
| `CV` | `compiler/view_compiler/compile_view.dart` |
| `VB` | `compiler/view_compiler/view_builder.dart` |
| `VC` | `compiler/view_compiler/view_compiler.dart` |
| `VU` | `compiler/view_compiler/view_compiler_utils.dart` |
| `PP` | `compiler/provider_parser.dart` |
| `CM` | `compiler/compile_metadata.dart` |
| `FC` | `source_gen/template_compiler/find_components.dart` |
| `DE` | `compiler/output/dart_emitter.dart` |
| `RT:*` | runtime `ngdart` (`src/meta/directives.dart`, `src/core/linker/view_container.dart`, `src/runtime/queries.dart`) |

Os exemplos marcados **(oráculo)** foram conferidos com a saída real do compilador em
`corpus/ngdart/oraculo/*.template.dart`. Os marcados **(derivado)** foram deduzidos só do
código, sem um caso do corpus que os confirme.

Porte Rust: `crates/gerador_ng/src/visao.rs` (`V`) e `componente.rs` (`C`).

---

## 1. Metadados da consulta

### R1.1 — Forma de `CompileQueryMetadata`
`selectors`, `descendants`, `first`, `propertyName`, `isElementType` e `read` (`CM:378-409`).
São preenchidos em `_getQuery` (`FC:496-530`):

- `propertyName = element.displayName`, ou seja, o nome do campo ou do setter, sem o `=`
  (`FC:362-366`, `FC:386-390`).
- `descendants` e `first` vêm do valor constante da anotação (`coerceBool`, com padrão
  `false`).
- `read` fica como `CompileTokenMetadata(identifier: {name, moduleUrl})` do tipo passado em
  `read:` (`FC:501-511`).
- `isElementType` é verdadeiro quando o tipo do parâmetro do setter é atribuível a
  `dart:html#Element`, ou quando é `Iterable<E>` (ou um subtipo, como `List`) com `E`
  atribuível a `Element` (`FC:518-524`).

**Rust:** `C:consulta_simples` (para visão) e `C:consultas_de_conteudo` (para conteúdo).
`Consulta.tipo` guarda o tipo escrito, e `V:e_tipo_de_elemento` decide `isElementType` por
nome resolvido. **Parcial:** o caso `Iterable<E>` só é tratado quando se trata de
`@ViewChildren` (a partir do argumento de tipo).

### R1.2 — Padrões de cada anotação (runtime)
Cada anotação passa valores fixos ao construtor da base (`RT:meta/directives.dart`):

| Anotação | Linhas | `descendants` | `first` |
|---|---|---|---|
| `ViewChild` | 622-626 | `true` | `true` |
| `ViewChildren` | 586-590 | `true` | `false` |
| `ContentChild` | 393-397 | `true` | `true` |
| `ContentChildren` | 374-379 | `true` por padrão, configurável | `false` |

Só `ContentChildren` aceita `descendants:`. `first` e `descendants` nunca são configuráveis
pelo usuário nas anotações de visão.

**Rust:**
- `C:consultas_de_conteudo` aceita `descendants:` só em lista. Coberto.
- `C:consulta_simples` recusa qualquer opção além de `read:`. Isso é coerente, porque o
  construtor não expõe outras.

### R1.3 — Seletores
`_getSelectors` (`FC:458-488`):

- Uma string é partida por `','` **sem `trim`**. Cada pedaço vira
  `CompileTokenMetadata(value: s)`, e `'a, b'` gera o token `' b'`.
- Um `Type` vira um token de identificador `{name, moduleUrl}`.
- Um seletor nulo gera o erro `Missing selector argument`.

**Rust:** `C:consulta_simples` recusa `'a,b'`, espaços nas pontas e string vazia
(`"@ViewChild('a,b')"`). **Falta** o caso de vários seletores.

### R1.4 — Ordem das consultas na classe
- Onde vão: `@ContentChild[ren]` vai para `_queries`, `@ViewChild[ren]` para `_viewQueries`
  (`FC:374`, `FC:398`). Esses campos viram `queries`/`viewQueries` do diretivo
  (`FC:702-703`).
- Classe: a coleta visita `element.visitChildren(this)`, começando pelos supertipos, em
  `allSupertypes.reversed`, e depois a própria classe (`FC:596-621`). As consultas herdadas
  **entram antes** e não são deduplicadas.
- Membros: dentro de uma classe, a ordem é a da visita do analyzer, com os acessores
  explícitos (setters) antes dos campos. O corpus confirma: em `j81`, `editorRef` (setter)
  tem índice 0, `dentroRef` (setter) tem 1 e `caixa` (campo) tem 2. Em `j72`, o setter
  `pelo` é atribuído antes dos campos.

**Rust:**
- `C:consultas_da_classe` e `C:consultas_de_conteudo` põem os setters antes dos campos.
  Coberto.
- **Falta** herdar consultas de superclasse/mixin, porque `C` não vê os herdados
  (comentário em `C:659`).

### R1.5 — Restrições que geram erro
São reportadas e recuperadas; nenhuma gera código.

- `@ViewChild`/`@ContentChild` em tipo não anulável gera o erro
  `ViewChild and ContentChild queries must be nullable.` (`FC:368-370`, `FC:392-394`,
  `FC:417-425`).
- Campo `late` com consulta gera o erro `View and content queries cannot be "late".`
  (`FC:427-435`).
- O membro precisa ser um setter público ou um campo não final. Caso contrário, há um
  `log.severe` (`FC:376-378`, `FC:400-402`).

**Rust:** `C:consulta_simples` recusa campos `final`/`late`/estáticos e campos sem tipo. A
mensagem de erro do oficial não é reproduzida, o que é aceitável porque o caso é uma recusa.

---

## 2. Construção dos objetos `CompileQuery`

### R2.1 — Consultas de visão
- Só existem numa `CompileView` do tipo `ViewType.component`. A visão hospedeira e as
  embutidas não criam nenhuma (`CV:464-480`).
- O `queryIndex` é o índice de cada consulta em `component.viewQueries`, começando em 0 e
  contando todas, sejam estáticas ou dinâmicas (`CV:468-477`).
- O `boundDirective` é `BuiltInSource(component, DetectChangesVars.cachedCtx)`, ou seja, o
  resultado é escrito em `_ctx.<prop>` (`CV:466-467`). Isso força
  `final _ctx = this.ctx;` no método que o lê (`VU:607-620`, `VB:858-861`, `CV:1455`).
- O `nodeIndex` vale `-1` (`_viewQueryNodeIndex`, `CQ:11`, `CQ:99-114`).

**Rust:** `V` (bloco perto de `V:10370-10392`) numera `i` por `c.consultas.iter().enumerate()`
e escreve `_ctx.{prop} = …`. Coberto, com a ressalva sobre herança de R1.4.

### R2.2 — Consultas de conteúdo
- São criadas em `CompileElement.beforeChildren`, uma por `directive.queries` de cada
  diretiva do elemento, na ordem de `_directives`. O `boundDirective` é a instância da
  diretiva (`CE:212-218`).
- O `queryIndex` é `_queryCount`, um contador **por elemento** que atravessa todas as
  diretivas do elemento (`CE:464-479`). O `nodeIndex` é o índice do nó hospedeiro.
- O `queryRoot` é a `view` do elemento hospedeiro, isto é, a visão de **quem usa** o
  componente ou diretiva, e não a do próprio componente.

**Rust:** `V:consultas_do_filho`, `V:consultas_das_diretivas` e `V:consultas_de_conteudo_no`
tratam só a forma estática (ver R6.4).

### R2.3 — `CompileTokenMap` agrupa as consultas por token
`addQueryToTokenMap` (`CQ:498-510`) insere a consulta na lista de **cada** seletor.
`CompileTokenMap.values` devolve as listas na ordem em que cada token foi inserido pela
primeira vez (`CM:235-275`, um `LinkedHashMap` indexado por `assetCacheKey`).

A chave `assetCacheKey` é calculada assim:
- token de string: a própria string;
- token de tipo: `'name|moduleUrl|identifierIsInstance|value|typeArgs'` (`CM:178-192`).

Duas consequências:
- **R2.3a — ordem de emissão.** As atualizações são emitidas percorrendo `values` e, dentro
  de cada lista, as consultas (`CV:506-511`, `CE:356-362`). Por isso a ordem **não** é a de
  declaração quando há tokens repetidos. Por exemplo, as consultas `A('x')`, `B('y')` e
  `C('x')` emitem na ordem `A`, `C`, `B` **(derivado)**.
- **R2.3b — duplicação.** Uma consulta com dois seletores (`'a,b'`) aparece em duas listas.
  Por isso `createImmediateUpdates`/`createDynamicUpdates` são chamados duas vezes e as
  instruções saem **duplicadas** (o campo sujo é alocado uma vez só, por causa do cache
  `_dirtyFieldIfNeeded`, `CQ:356-365`) **(derivado)**.

**Rust:**
- As estáticas de visão saem por `c.consultas.iter().enumerate()`, em ordem de declaração,
  no bloco `V:10420-10550`.
- As dinâmicas saem na ordem de `consultas_dinamicas`, também em ordem de declaração.
- **Falta R2.3a**: o agrupamento por token só coincide com o oficial quando cada seletor
  aparece uma vez ou quando as consultas repetidas estão contíguas. **Falta R2.3b**, que é
  recusado.
- Nas consultas de conteúdo, `V:consultas_de_conteudo_no` segue a ordem de declaração.
  **Falta** o mesmo agrupamento.

---

## 3. Casamento: que elemento produz resultado para qual consulta

### R3.1 — Provedores e referências de cada elemento
Em `beforeChildren` (`CE:220-277`), a lista `queriesWithReads` é montada nesta ordem:

1. Para cada provedor de `_resolvedProvidersArray`, na ordem:
   - `TemplateRef` vem primeiro nos `<template>`, porque é inserido no índice 0
     (`CE:176-189`);
   - em seguida vêm diretivas, componente e `providers:`.

   Cada provedor gera um `_QueryWithRead(q, provider.token)` para cada `q` em
   `_getQueriesFor(token)` (`CE:220-225`).
2. Para cada referência `#nome` de `referenceTokens`, na ordem de declaração no elemento
   (`CE:101-107`), gera `_QueryWithRead(q, CompileTokenMetadata(value: nome))` para cada `q`
   em `_getQueriesFor(value: nome)` (`CE:228-238`).

O `read` efetivo é `metadata.read ?? tokenCasado` (`CE:482-487`).

`ElementRef`, `Element`, `HtmlElement`, `Injector`, `ViewContainerRef`,
`ChangeDetectorRef` e `ComponentLoader` são adicionados a `_providers` (`CE:115-129`,
`CE:193-206`), mas **não** a `_resolvedProvidersArray`. Por isso
`@ViewChild(ElementRef)` sem `#ref` não casa com nada.

**Rust:**
- `V:arvore_da_consulta`, `V:onde_esta`, `V:onde_casa` e `V:casa_a_chave` fazem o
  casamento em pré-ordem.
- `V:primeiro_resultado_dinamico` documenta que os provedores vêm antes das referências.
- **Parcial**: o casamento por tipo só é aceito para componentes filhos
  (`V:formas_contra_o_template`: `"@ViewChild(Tipo) fora da forma estática"`). Diretiva de
  atributo, serviço de `providers:` e `@ViewChildren(TemplateRef)` por tipo **faltam**.

### R3.2 — `_getQueriesFor` e o `descendants`
Em `CE:442-462`, o laço começa no elemento atual e sobe por `parent` enquanto
`parent != null`. Em cada elemento `currentEl`:

- são incluídas as consultas de `currentEl._queries[token]` que satisfazem
  `descendants || distance <= 1`;
- depois, se `currentEl` tem diretivas, `distance++`.

Ao final do laço, **todas** as consultas de visão de `view.componentView.viewQueries[token]`
são acrescentadas, sem filtro de distância.

Como resultado, `distance` conta os elementos **com diretivas** entre o candidato
(inclusive) e o hospedeiro da consulta (exclusive). Um elemento sem diretivas no meio não
conta. O próprio hospedeiro pode ser resultado da própria consulta, com `distance = 0`.

**Exemplo (oráculo `j72`):** o grupo tem
`@ContentChildren(J72Item, descendants: false) List<J72Item> diretos` e o template é

```html
<div j72-grupo>
  <p j72-item><span j72-item></span></p>
  <b j72-marca></b>
  <section><i j72-item j72-marca></i></section>
</div>
```

A saída contém
`this._J72Grupo_0_5.diretos = [this._J72Item_1_5.instance, this._J72Item_5_5.instance];`.

- `span`: distância 2 (`span` e `p` têm diretivas), logo fica fora.
- `i`: distância 1 (`section` não tem diretivas), logo entra.

O `PP:_getQueriesFor` (`PP:165-187`) repete a mesma regra no nível do parser, para decidir a
avidez (R3.5).

**Rust:** `V:consultas_de_conteudo_no` calcula
`distancia = r.acima[pos+1..].filter(|(_, d)| *d).count()` e aplica
`!q.descendentes && distancia > 1`. Coberto para conteúdo estático.

### R3.3 — Cadeia de pais em visão embutida
Os filhos de um `<template>` são visitados com `parent =
embeddedView.declarationElement.parent ?? declarationElement` (`VB:460-464`). A cadeia de
`parent` **pula o próprio `<template>`**. Isso tem duas consequências:

- as diretivas do `<template>` (por exemplo `NgIf`) não contam em `distance`;
- as consultas de conteúdo de uma diretiva **no próprio `<template>`** não veem os filhos.

As consultas de visão continuam alcançáveis, porque `componentView` de uma embutida é o da
declaração (`CV:453-462`).

**Rust:** em `V:classe_em_embutida` e `V:arvore_da_consulta`, o `*` e o `<template>` escrito
são atravessados sem contar distância. **Não verificado** no Rust o caso de uma consulta de
conteúdo numa diretiva do próprio `<template>`.

### R3.4 — O valor de cada resultado
Laço em `CE:242-277`:

| Caso | Condição | Valor | `changeDetectorRef` |
|---|---|---|---|
| (a) | `read.identifier != null`: consulta por tipo, ou qualquer consulta com `read:` | `_providers.get(read)?.build()` | o do provedor |
| (b) | `read` é string (`#ref`) e `referenceTokens[ref] != null` (componente, `exportAs` ou `TemplateRef`) | `_providers.get(token).build()` | o do provedor |
| (c) | `read` é string e o `#ref` não tem token | `isElementType ? renderNode : elementRef`, com `elementRef = ElementRef(_el_n)` (`CE:109-113`) | — |

No caso (a), se o provedor não existe no elemento, **não há resultado**: a omissão é
silenciosa. O `changeDetectorRef` só não é nulo quando o provedor é um componente `onPush`,
e nesse caso vale `componentView`, isto é, `_compView_n` (`CE:392-397`).

Valores conforme o `read:`:

| `read:` | Valor |
|---|---|
| `Element`/`HtmlElement` | o nó (`_el_n`) |
| `ElementRef` | `ElementRef(_el_n)` |
| `ViewContainerRef` | `this._appEl_n` |
| `TemplateRef` num `<template>` | `_TemplateRef_n_m` |
| outro tipo | a instância do provedor (`this._Dir_n_m`, ou `.instance` quando envolto em detector) |

**Exemplos (oráculo):**
- `j14`: `@ViewChild('e', read: ElementRef)` gera `return ElementRef(nestedView._el_1);`.
- `j89`: `@ViewChild('marca', read: J89Servico)` gera `return nestedView._J89Servico_7_6;`.
- `j72`: `@ContentChildren(J72Marca, read: HtmlElement)` gera
  `this._J72Grupo_0_5.nos = [_el_3, this._el_5];`.

**Rust:**
- `V:valor_de_elemento` cobre o caso (c) e `read: ElementRef/Element`.
- `V:token_de_leitura` e `V:chave_de_leitura` cobrem o `read:` de provedor.
- O `TemplateRef` de `<template #t>` só é aceito na forma estática (`V:formas_contra_o_template`).
- **Falta** `read: ViewContainerRef`.
- **Falta** `read: TemplateRef` por tipo e em embutida.
- **Falta** `read:` numa consulta por tipo, recusado com
  `"@ViewChild(.., read: T) fora de elemento estático"`.

### R3.5 — Avidez dos provedores consultados
`ProviderElementContext` (`PP:88-112`) monta `queriedTokens` com o `read ?? token` de cada
consulta que casa com cada provedor ou `#ref` do elemento. Com isso:

- um provedor cujo token está em `queriedTokens` vira `eager` e é criado no `build()`, em
  vez de ficar `late`;
- se o token `ViewContainerRef` foi pedido, `_requiresViewContainer = true`, e o elemento
  ganha `_appEl_n` (`PP:99-104`).

Além disso, o `TemplateRef` de um `<template>` só vira campo
(`isReferencedOutsideBuild`) quando é publicado por `#ref` ou quando existe uma consulta cujo
**seletor** é `TemplateRef`. Um `read: TemplateRef` não conta para isso (`CE:173-174`).

**Rust:** a avidez por consulta é tratada em `V` (provedores lidos por `read:` entram como
campos). **Falta** `_requiresViewContainer` por consulta.

---

## 4. Árvore de resultados e estático × dinâmico

### R4.1 — `addQueryResult`
Em `CQ:165-197`:

1. `_resolvePathToRoot(origin)` sobe `declarationElement` de `origin` até `_queryRoot` e
   devolve a lista de elementos-âncora, da raiz para a folha (`CQ:304-316`).
2. Para cada âncora, reaproveita o **último** item da lista corrente se ele for um
   `_NestedQueryValues` da mesma `embeddedView`. Caso contrário, cria um novo
   (`CQ:175-186`).
3. Acrescenta `_QueryValue(result, cdRef)` ao nível final (`CQ:189`).
4. Se o caminho não é vazio, chama `_setParentQueryAsDirty(origin)` (`CQ:194-196`).

Os resultados chegam em pré-ordem do template inteiro, porque a visão embutida é
construída dentro de `visitEmbeddedTemplate` (`VB:414-470`).

**Rust:** `V:ItemDeConsulta` (`Valor`/`Aninhada`) e `V:arvore_da_consulta`. Coberto, com
estas restrições: resultado no próprio `<template>` escrito é recusado, e os resultados em
diretiva de tag ou por tipo que não seja um filho também.

### R4.2 — Estática ou dinâmica
`_isSingle = metadata.first` (`CQ:125`) e `_isStatic = !_shouldMapNestedViews(_values)`
(`CQ:128`, `CQ:144-153`):

- Uma consulta única (`first`) é dinâmica **só se o primeiro item** é aninhado.
- Uma lista é dinâmica se **algum** item é aninhado.

Uma consulta única cujo primeiro resultado está nesta visão é estática e ignora os
resultados aninhados que vêm depois (oráculo `j17`: `_ctx.primeiro = this._el_0;` no
`build()`).

**Rust:** `V:consulta_e_dinamica`. Coberto.

### R4.3 — Consulta estática: atribuição no `build()`
- `createImmediateUpdates` devolve `_createUpdates()` quando a consulta é estática
  (`CQ:435-437`).
- Para uma consulta de **visão**, isso vai para `_createMethod` em `afterNodes()`
  (`CV:502-511`), chamado por `_finishView` (`VC:78`). A atribuição entra depois de tudo que
  o builder e o binder puseram no `build()`, e antes de `initSubscriptions`/`initRootNode`
  (`VB:840-843`).
- Para uma consulta de **conteúdo**, vai para `_createMethod` em
  `afterChildren(elementoHospedeiro)` (`CE:355-362`, `VB:266-269`). Ou seja, sai logo depois
  dos filhos do elemento, antes do `createAndProject`/`create` do componente hospedeiro
  (`VB:339-351`).

`_createUpdates` (`CQ:439-467`) segue estas regras:
- Consulta única sem resultado: nada é emitido (`CQ:452-454`).
- Consulta única com resultado: `values.first`. Lista: `[v1, v2, …]`, e `[]` quando não há
  resultado (`CQ:473-474`).
- Antes da atribuição, vêm os registros de `ChangeDetectorRef` (R5.3).
- A atribuição é sempre `<boundDirective>.<propertyName> = <valor>;`. Um setter e um campo
  geram o mesmo texto (`CQ:461-465`).

**Exemplo (oráculo `j15`):** o template é `<b #fora>1</b><div *ngIf="a"><i #dentro>2</i></div><u #depois>3</u>`,
com três `@ViewChild` do tipo `Element?`. O fim do `build()` fica:

```dart
    _ctx.fora = _el_0;
    _ctx.depois = _el_3;
```

**Exemplo (oráculo `j24`):** um conteúdo estático com filhos `onPush`.
`@ContentChildren(D07FilhoOnPush) abas` e `@ContentChild(D07FilhoOnPush) primeira` geram:

```dart
    import7.View.queryChangeDetectorRefs[this._D07FilhoOnPush_1_5] = this._compView_1;
    import7.View.queryChangeDetectorRefs[this._D07FilhoOnPush_2_5] = this._compView_2;
    this._J23AbasOnPush_0_5.abas = [this._D07FilhoOnPush_1_5, this._D07FilhoOnPush_2_5];
    import7.View.queryChangeDetectorRefs[this._D07FilhoOnPush_1_5] = this._compView_1;
    this._J23AbasOnPush_0_5.primeira = this._D07FilhoOnPush_1_5;
    this._compView_0.createAndProject(this._J23AbasOnPush_0_5, [
```

**Exemplo (oráculo `j72`):** listas vazias num hospedeiro sem conteúdo: `<ul j72-grupo></ul>`
gera `this._J72Grupo_6_5.pelo = [];` e as demais listas. `marca` (única) não recebe nada.

**Exemplo (oráculo `j23`, visão hospedeira):** o hospedeiro não tem filhos, então
`this.component.abas = [];` sai logo depois de `this.component = …;`, e `@ContentChild` não
gera nada.

**Rust:**
- Visão estática: bloco de `V:10420-10550`, com `_ctx.{} = [..]`, o primeiro valor,
  `queryChangeDetectorRefs` e nada para a única vazia. Coberto.
- Conteúdo: `V:consultas_de_conteudo_no`. Coberto.
- Hospedeira: `V:consultas_da_hospedeira`. Coberto, mas recusa as consultas que achariam o
  próprio nó.
- **Parcial**: `@ViewChildren` de filho `onPush` é recusado (`"@ViewChildren de filho onPush"`),
  e `#ref` repetido em filho `onPush` também.

### R4.4 — Consulta dinâmica: atualização em `detectChangesInternal`
`createDynamicUpdates` (`CQ:418-432`) gera:

```dart
if (this.<campoSujo>) {
  <registros de ChangeDetectorRef>
  <boundDirective>.<prop> = <valor>;
  this.<campoSujo> = false;
}
```

Consultas de visão e de conteúdo vão ambas para `_updateContentQueriesMethod`
(`CV:1120-1122`). O `_updateViewQueriesMethod` **nunca é preenchido**: ele só aparece em
`CV:404`, `CV:1402` e `CV:1443`.

Em `writeChangeDetectionStatements` (`CV:1396-1466`), a ordem é:

1. entradas;
2. `this._appEl_n.detectChangesInNestedViews();`;
3. `if ((!importN.debugThrowIfChanged)) { <consultas de conteúdo>; <consultas de visão>; <ganchos AfterContent> }`;
4. propriedades de renderização;
5. `detectChanges()` dos filhos;
6. o bloco `AfterView`.

As consultas de conteúdo vêm antes das de visão porque são adicionadas durante o build
(`afterChildren`), enquanto as de visão entram em `afterNodes`.

`valor` é montado em `_createUpdatesNested` (`CQ:489-495`):
- com exatamente um item, esse item; com mais, a lista `[..]`;
- se a consulta é única, `.first` quando `_values.hasStaticValues` (há um resultado na
  raiz) e `importN.firstOrNull(..)` caso contrário (`CQ:477-483`, `CQ:533-537`,
  `RT:queries.dart:2`).

**Exemplo (oráculo `j15`):**

```dart
    this._appEl_2.detectChangesInNestedViews();
    if ((!import12.debugThrowIfChanged)) {
      if (this._viewQuery_dentro_1_isDirty) {
        _ctx.dentro = import13.firstOrNull(this._appEl_2.mapNestedViewsWithSingleResult((_ViewJ15ConsultasMisturadas1 nestedView) {
          return nestedView._el_1;
        }));
        this._viewQuery_dentro_1_isDirty = false;
      }
    }
```

**Exemplo (oráculo `j17`):** uma lista com resultado estático e dois `*`:

```dart
        _ctx.todos = [
          this._el_0,
          ...this._appEl_2.mapNestedViewsWithSingleResult((_ViewJ17VariosResultados1 nestedView) {
            return nestedView._el_1;
          }),
          ...this._appEl_3.mapNestedViewsWithSingleResult((_ViewJ17VariosResultados2 nestedView) {
            return nestedView._el_1;
          })
        ];
```

Aqui `_el_0` é promovido a campo porque é lido fora do `build()` (`VU:97-101`).

**Exemplo (derivado):** uma única com o `*` antes do estático, como em
`<div *ngIf="a"><i #s></i></div><b #s></b>` com `@ViewChild('s')`. Os itens são
`[Aninhada, Valor]`, e a saída é
`_ctx.s = [...this._appEl_0.mapNestedViewsWithSingleResult((… nestedView) {…}), this._el_2].first;`.

**Rust:**
- `V:resolver_consultas`, `V:MontagemDaConsulta::resultados` e
  `V:MontagemDaConsulta::mapa` montam o valor.
- `V:tem_resultado_estatico` decide entre `.first` e `firstOrNull`.
- `V:consulta_em_embutida` decide quais consultas se traduzem.
- Coberto para consultas de visão cujos resultados são elementos HTML, `ElementRef`,
  provedor por `read:` ou instâncias de filho.
- **Falta** a forma dinâmica das consultas de conteúdo (R6.4).

### R4.5 — `_buildQueryResults` e `_mapNestedViews`
`_buildQueryResults` (`CQ:200-235`) percorre os itens em ordem:

- Um `_QueryValue` é acrescentado. Numa consulta única, o laço **para** no primeiro
  `_QueryValue` (`CQ:219`), e isso vale em todo nível, inclusive dentro de um fecho.
- Um `_NestedQueryValues` vira `_mapNestedViews(v)`. Se o nível tem mais de um item, o
  resultado é espalhado com `...` (`CQ:225-228`).

`_mapNestedViews` (`CQ:240-286`) segue estas regras:

- O receptor é `value.view.declarationElement.appViewContainer`, isto é, `this._appEl_n`.
- O parâmetro é `(<ClasseDaEmbutida> nestedView)`.
- Um resultado gera `return x;`. Mais de um gera `return [a, b];` (`CQ:258-260`).
- Cada `ReadClassMemberExpr`/`ReadNodeReferenceExpr` vira `nestedView.<nome>`
  (`VU:86-110`). Os `ReadNodeReferenceExpr` são **promovidos a campo** da embutida
  (`VU:98-100`), o que gera `late final … _el_n;` nela.
- O método é escolhido por `_hasMultipleResults` (`CQ:134-141`):
  - numa consulta única, é `mapNestedViews` se o primeiro item do nível é aninhado, e
    `mapNestedViewsWithSingleResult` caso contrário;
  - numa lista, é `mapNestedViews` se há um aninhado ou mais de um item, e
    `mapNestedViewsWithSingleResult` caso contrário.
- Dentro do fecho, antes do `return`, vêm os registros de `ChangeDetectorRef` com
  `nestedView.` (`CQ:263-283`).
- No runtime, `mapNestedViews` faz `addAll(callback(v))` e `…WithSingleResult` faz
  `add(callback(v))`. Sem visões, ambos devolvem `const <Never>[]`
  (`RT:view_container.dart:191-219`).

**Exemplo (oráculo `i97`):** o template é
`<div *ngIf="a"><div *ngIf="b"><span #fundo>x</span></div></div>` com `@ViewChild('fundo')`:

```dart
        _ctx.fundo = import13.firstOrNull(this._appEl_0.mapNestedViews((_ViewI97ViewChildDoisNiveis1 nestedView) {
          return nestedView._appEl_1.mapNestedViewsWithSingleResult((_ViewI97ViewChildDoisNiveis2 nestedView) {
            return nestedView._el_1;
          });
        }));
```

**Exemplo (oráculo `j89`):** vários resultados na mesma embutida:

```dart
          ...this._appEl_0.mapNestedViews((_ViewJ89ConsultasNaMesmaEmbutida1 nestedView) {
            return [nestedView._el_1, nestedView._el_4];
          }),
```

**Exemplo (oráculo `j86`):** filho `onPush` no nível mais fundo:

```dart
          return nestedView._appEl_2.mapNestedViewsWithSingleResult((_ViewJ86ConsultaEmMolde2 nestedView) {
            import3.View.queryChangeDetectorRefs[nestedView._J86Tabela_0_5] = nestedView._compView_0;
            return nestedView._J86Tabela_0_5;
          });
```

**Rust:** `V:MontagemDaConsulta::mapa` escolhe o método por `varios`, escreve a lista literal
com `V:lista_literal` e trata o `break` da única e o `...` quando `itens.len() > 1`. Coberto.
Os ordinais por visão (`ItemDeConsulta::Valor.ordinal` com `V:chave_de_ordinal`) servem para
achar o **campo** certo quando o mesmo `#ref` se repete numa visão. Isso é um artefato do
porte: o oficial usa a expressão do próprio nó.

---

## 5. Campos "sujos" e `dirtyParentQueriesInternal`

### R5.1 — Nome e declaração
O campo sujo é criado em `_createQueryDirtyField` (`CQ:370-395`):

| Consulta | Nome do campo |
|---|---|
| de visão | `_viewQuery_<sel>_<queryIndex>_isDirty` |
| de conteúdo | `_query_<sel>_<nodeIndex>_<queryIndex>_isDirty` |

`<sel>` é `selectors.first.name` (`CM:209-214`):
- num token de string, a string com `\W` trocado por `_`;
- num token de tipo, o nome da classe (por exemplo `_viewQuery_J89Item_1_isDirty`).

A declaração é `bool <nome> = true;`, com o modificador `private` e alocação em
`view.storage` da visão `queryRoot`.

- **Quando é alocado:** de forma preguiçosa (`CQ:356-365`), na primeira chamada de
  `_setParentQueryAsDirty` que passa do teste `_isStatic` (durante o build, em pré-ordem) ou
  em `createDynamicUpdates`.
- **Onde aparece na classe:** o emissor põe primeiro os campos **com** inicializador e
  depois os sem inicializador (`DE:198-205`). Por isso os campos sujos abrem a classe, na
  ordem de alocação, antes de `late final ViewContainer _appEl_n`.

**Exemplo (oráculo `j39`):** as consultas são `campo` (índice 0), dentro do segundo `*ngIf`,
e `botao` (índice 1), dentro do primeiro. A classe começa assim:

```dart
  bool _viewQuery_botao_1_isDirty = true;
  bool _viewQuery_campo_0_isDirty = true;
  late final ViewContainer _appEl_0;
```

**Rust:**
- Visão: `format!("_viewQuery_{}_{i}_isDirty", q.referencia)`. Isso **só** está certo para
  seletores de string sem caracteres `\W`. Um `#ref` com `-` exigiria a sanitização, mas
  `#ref` válido não tem hífen. Uma consulta por tipo usa `q.referencia`, que é o nome da
  classe. Coberto.
- A ordem é aproximada por `V:primeiro_resultado_dinamico`, com desempate pelo índice.
  Coberto para os casos do corpus (`j39`, `j66`, `j89`).
- **Falta** o nome `_query_…` das consultas de conteúdo.

### R5.2 — Marcar a consulta como suja a partir da visão embutida
`_setParentQueryAsDirty(origin)` (`CQ:398-415`) segue estas regras:

- Não faz nada se a consulta é estática naquele momento.
- Emite uma única instrução por `origin`, controlada pelo conjunto `_queryResultOrigins`.
- `origin` é a visão **mais funda**, onde o resultado está. As visões intermediárias não
  recebem nada.
- A instrução vai para `origin.dirtyParentQueriesMethod` e tem a forma
  `getPropertyInView(this.<campo>, origin, queryRoot).set(true)`.

`getPropertyInView` (`VU:44-74`) monta `parentView!`, `(parentView!).parentView!` e assim por
diante, e embrulha o resultado em `unsafeCast<ClasseDoQueryRoot>(…)`. As propriedades
`componentStyles`, `parentIndex` e `parentView` não levam cast (`VU:28-32`).

O método só é emitido em visões `embedded`, como `dirtyParentQueriesInternal()` com
`@override` (`VB:539-547`). Ele fica entre `detectChangesInternal` e `destroyInternal` e,
quando está vazio, é omitido (`VB:586`, filtro `body.isNotEmpty`).

A ordem das linhas é a ordem das chamadas `addQueryResult` que atingem aquela `origin`
(pré-ordem e, dentro do elemento, R3.1).

**Exemplo (oráculo `i97`, dois níveis):**

```dart
  @override
  void dirtyParentQueriesInternal() {
    import7.unsafeCast<ViewI97ViewChildDoisNiveis0>(((this.parentView!).parentView!))._viewQuery_fundo_0_isDirty = true;
  }
```

**Exemplo (oráculo `j66`):** no template
`<div *ngIf="mostrar" #rolagem …><table #tabela …></table></div>`, a linha de `rolagem`
(índice 1) sai antes da de `tabela` (índice 0), porque o `div` é visitado primeiro.

**Rust:** `V` monta as linhas em `format!("{util}.unsafeCast<{}0>({vista}).{campo} = true;")`
(perto de `V:8730-8752`). A ordem vem de `sujas.sort_by_key(primeiro_resultado)` (perto de
`V:5932`) e a profundidade de `niveis`. Coberto para consultas de visão. **Falta** o mesmo
para consultas de conteúdo, porque aí o `queryRoot` pode ser uma embutida e o cast vai para a
classe dela.

### R5.3 — Registro de `ChangeDetectorRef` (`onPush`)
`_createAddQueryChangeDetectorRefs` (`CQ:519-531`) gera
`importN.View.queryChangeDetectorRefs[<valor>] = <cdRef>;` para cada valor com detector, na
ordem dos valores:

- na forma estática e na raiz da dinâmica, antes da atribuição;
- dentro do fecho, antes do `return`.

Numa consulta única, só o do primeiro valor entra, por causa do `break` em `CQ:219`. No
runtime, isso alimenta o `markChildForCheck` (`RT:view.dart:32`, `128`).

**Rust:** `V:MontagemDaConsulta::resultados` (nas dinâmicas), o bloco estático de visão e
`V:consultas_de_conteudo_no`. Coberto, exceto `@ViewChildren` estático de filhos `onPush`,
que é recusado.

---

## 6. Consultas de conteúdo e quem projeta

### R6.1 — Quem atualiza
O `queryRoot` de uma consulta de conteúdo é a visão onde o elemento hospedeiro está
declarado (R2.2). O conteúdo projetado são os filhos desse elemento no **mesmo** template.
Por isso:

- quem atualiza é o `build()` ou o `detectChangesInternal()` da visão de quem projeta;
- a visão do próprio componente nunca atualiza as próprias consultas de conteúdo;
- na visão hospedeira, sem filhos, as listas recebem `[]` e as únicas não recebem nada
  (oráculo `j23`).

**Rust:** `V:consultas_da_hospedeira` e `V:consultas_de_conteudo_no`. Coberto para a forma
estática.

### R6.2 — `boundDirective`
A atribuição é feita em `getDirectiveSource(directive).build()`:

- `this._Dir_n_m.<prop>`;
- `this._Dir_n_m.instance.<prop>` quando a diretiva tem detector (`CV:1208-1231`);
- `this.component.<prop>` na hospedeira.

**Rust:** `campo_inst` em `V:consultas_de_conteudo_no`. Coberto.

### R6.3 — Diretiva com consulta de conteúdo dentro de uma embutida
O `queryRoot` é a embutida, e as atualizações entram no `build()` ou `detectChangesInternal()`
dela. Um resultado numa embutida mais funda marca `unsafeCast<_ViewX1>(this.parentView!)…`
**(derivado)**.

**Rust:** não verificado. Provavelmente é recusado junto com R6.4.

### R6.4 — Consulta de conteúdo dinâmica
**(derivado)** Tome `<j-abas><j-aba *ngIf="a"></j-aba></j-abas>` com
`@ContentChildren(JAba) List<JAba>? abas` e com `j-abas` no nó 0. A saída esperada é:

```dart
  bool _query_JAba_0_0_isDirty = true;
  …
      if (this._query_JAba_0_0_isDirty) {
        this._JAbas_0_5.abas = this._appEl_1.mapNestedViewsWithSingleResult((_ViewX1 nestedView) {
          return nestedView._JAba_0_5;
        });
        this._query_JAba_0_0_isDirty = false;
      }
```

Isso fica dentro de `if ((!importN.debugThrowIfChanged)) {…}`, antes das consultas de visão.
Na embutida:

```dart
  void dirtyParentQueriesInternal() {
    importN.unsafeCast<ViewX0>((this.parentView!))._query_JAba_0_0_isDirty = true;
  }
```

Os sufixos `_5` e `_1` dependem da numeração de provedores e nós (spec de provedores).

**Rust:** **falta**. `V:consultas_de_conteudo_no` recusa com
`"@ContentChild do filho com resultado em visão embutida"`, e o `#ref` no conteúdo com
`"@ContentChild do filho com #ref no conteúdo"`.

---

## 7. Ordem entre várias consultas (resumo)

1. **No `build()`:**
   - as consultas de conteúdo estáticas saem no `afterChildren` de cada hospedeiro, em
     pós-ordem de elementos;
   - as consultas de visão estáticas saem no fim do build (R4.3);
   - em cada grupo, a ordem é a de `CompileTokenMap.values` e, dentro de cada token, a de
     declaração (R2.3a).
2. **No `detectChangesInternal()`:** um único bloco `if (!debugThrowIfChanged)`. Nele saem
   primeiro todas as dinâmicas de conteúdo, na ordem dos elementos, e depois as dinâmicas de
   visão, na mesma ordem de token. Os ganchos `AfterContent` vêm por último.
3. **Campos sujos:** na ordem de alocação (R5.1).
4. **`dirtyParentQueriesInternal`:** na ordem dos resultados (R5.2).

**Rust:** os itens 1 a 4 estão cobertos para visão quando não há token repetido fora de
ordem. As partes de conteúdo dinâmico faltam.

---

## Lacunas do porte (priorizadas)

1. **Consulta de conteúdo dinâmica** (R6.4, R5.1 `_query_…`, R5.2 com `queryRoot`
   embutido). É o padrão de `@ContentChildren` com `*ngIf`/`*ngFor` no conteúdo, comum em
   abas e listas. Hoje é recusada em `V:consultas_de_conteudo_no`.
2. **Agrupamento por token** (R2.3a). As estáticas e dinâmicas de visão e as de conteúdo
   saem em ordem de declaração. O oficial agrupa por seletor, na ordem da primeira
   ocorrência. Há divergência byte a byte sempre que um seletor se repete de forma não
   contígua (`@ViewChild('x')`, `@ViewChild('y')`, `@ViewChildren('x')`).
3. **Consultas herdadas** (R1.4). As consultas de superclasses e mixins entram antes e
   deslocam o `queryIndex` e os nomes `_viewQuery_*_N_isDirty`. `C` não as vê.
4. **Consulta por tipo de diretiva, serviço ou `TemplateRef`** (R3.1). Só componentes filhos
   são aceitos.
5. **`read: ViewContainerRef` e `read: TemplateRef`**, além do `_requiresViewContainer`
   induzido por consulta (R3.4, R3.5).
6. **`@ViewChildren` estático de filhos `onPush`** e `#ref` repetido em filho `onPush`
   (R5.3). Hoje são recusados.
7. **`read:` em consulta por tipo** e `read:` fora de elemento estático (R3.4).
8. **Vários seletores `'a,b'`** (R1.3, R2.3b): tokens sem `trim`, instruções duplicadas.
9. **Sanitização `\W→_` do nome do campo sujo** (R5.1). É irrelevante para `#ref` válidos,
   mas é necessária para seletores de string arbitrários.
10. **Consulta de conteúdo numa diretiva no próprio `<template>`** (R3.3), que não vê os
    filhos, e numa diretiva dentro de embutida (R6.3). Nenhuma das duas foi verificada.
