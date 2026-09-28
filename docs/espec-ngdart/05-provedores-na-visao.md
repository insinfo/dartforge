# 05 — Provedores na visão (DI por nó)

Especificação derivada da leitura do `ngcompiler-3.0.0-dev.3` (caminhos relativos a
`lib/v1/src/`). Cobre o caminho completo de um provedor: da resolução no template
(`ProviderElementContext`) à forma exata do campo, da instrução no `build()` e do
`injectorGetInternal` da classe de visão gerada.

| Abreviação | Arquivo |
|---|---|
| `PP` | `compiler/provider_parser.dart` |
| `PR` | `compiler/view_compiler/ir/provider_resolver.dart` |
| `PS` | `compiler/view_compiler/ir/provider_source.dart` |
| `PF` | `compiler/view_compiler/provider_forest.dart` |
| `CE` | `compiler/view_compiler/compile_element.dart` |
| `CV` | `compiler/view_compiler/compile_view.dart` |
| `VB` | `compiler/view_compiler/view_builder.dart` |
| `VC` | `compiler/view_compiler/view_compiler.dart` |
| `VU` | `compiler/view_compiler/view_compiler_utils.dart` |
| `K`  | `compiler/view_compiler/constants.dart` |
| `TA` | `compiler/template_ast.dart` |
| `TP` | `compiler/template_parser/ast_template_parser.dart` |
| `CM` | `compiler/compile_metadata.dart` |
| `ID` | `compiler/identifiers.dart` |
| `OA` | `compiler/output/output_ast.dart` |
| `DE` | `compiler/output/dart_emitter.dart` |
| `FC` | `source_gen/template_compiler/find_components.dart` |
| `SM` | `source_gen/template_compiler/compile_metadata.dart` |

Os exemplos marcados **(oráculo)** estão na saída real do compilador em
`corpus/ngdart/oraculo/*.template.dart` (o arquivo é citado). Os marcados **(derivado)**
foram deduzidos só do código, sem caso do corpus que os confirme.

Porte Rust: `crates/gerador_ng/src/diretivas.rs` (`D`) e `crates/gerador_ng/src/visao.rs`
(`V`). Os números de linha do porte são aproximados (os arquivos mudam); a função é a
referência.

> **Nota sobre nomes pedidos.** Nesta versão não existe `_createProviderProperty` nem
> getter preguiçoso: quem cria o campo é `CompileView.createProvider` (`CV:1134-1283`), e o
> provedor preguiçoso vira campo `late` **com inicializador**, nunca getter. `injectorGet`/
> `injectorGetOptional` não são escritos em `compile_element.dart`, mas em
> `injectFromViewParentInjector` (`VU:112-124`).

---

## 0. Visão geral do pipeline

Duas fases independentes, cada uma com seu algoritmo de busca:

1. **Fase de template (análise)** — `_ProviderVisitor` (`TP:1255-1322`) cria um
   `ProviderElementContext` por `ElementAst`/`EmbeddedTemplateAst`. Ela decide **quais**
   provedores existem no nó, **em que ordem** (`transformProviders`), **quem é ansioso**
   (`ProviderAst.eager`), reescreve dependências (atributos, `@Optional` sem provedor) e
   decide `requiresViewContainer`. Nada de código é gerado aqui.
2. **Fase de visão (emissão)** — `CompileElement` + `ProviderResolver` (`CE`, `PR`) leem
   `ast.providers` naquela ordem e produzem as expressões: embutidos, campos
   (`createProvider`), argumentos de construtor (`_getDependency` estático pela cadeia de
   `CompileElement`, senão `injectorGet`) e o `injectorGetInternal` (`PF`).

As duas fases **não** usam as mesmas regras de visibilidade (ver R2.6, R3.4): a fase de
template decide ordem/ansiedade; a de visão decide a expressão final.

---

## 1. Modelo de dados

### R1.1 — Token e igualdade
`CompileTokenMetadata{value, identifier, identifierIsInstance}` (`CM:159-172`). A igualdade
usada em todos os mapas é `equalsTo` por `assetCacheKey` (`CM:178-207`):

- com `identifier` e `moduleUrl`: `"name|moduleUrl|identifierIsInstance|value|typeArgs"`
  (typeArgs: `ExternalType {moduleUrl:name:generics}` por argumento, ou `{notExternalType}`);
- com `identifier` sem `moduleUrl`: chave `null` → **nunca** igual a nada;
- sem `identifier`: a chave é o próprio `value`.

`name` (`CM:209-214`): `value` com todo `\W` trocado por `_`, senão `identifier.name`.
Ex.: `const MultiToken<Object>('NgValidators')` → `NgValidators`; `OpaqueToken('j40.token')`
→ `j40_token`; classe `J61Config` → `J61Config`.

Rust: `D:Token::nome` cobre a sanitização (ASCII alfanumérico + `_`; `\W` do Dart também é
ASCII) ✔. A igualdade no porte é `PartialEq` estrutural do `enum Token` (classe por
URI+nome; multi/opaco por nome+tipo) — equivalente para os casos modelados.

### R1.2 — `CompileTokenMap`
Mapa por `assetCacheKey`, **ordem de inserção** (`values` = `_valueMap.values`,
`CM:271`). `add` de uma chave já presente **lança** `BuildError` "Failed to register provider
for token …" (`CM:238-259`). Consequência: prover manualmente um token que já é embutido do
nó (ex.: `ElementRef`, `Element`) quebra a compilação em `ProviderResolver.add`/
`addDirectiveProviders` (R3.1). Rust: não modela o erro; recusa esses casos antes
(`D:resolver_com` → `"provedor que depende de embutido do elemento"` etc.) — **parcial**.

### R1.3 — `ProviderAst`
Campos (`TA:430-478`): `token`, `multiProvider`, `providers` (lista de
`CompileProviderMetadata`), `providerType`, `eager`, `isReferencedOutsideBuild` (padrão
`true`), `typeArgument`, `visibleForInjection` (padrão **`false`**).

`ProviderAstType` (`TA:483-499`): `publicService` (`providers:`), `privateService`
(`viewProviders:`), `component`, `directive`, `builtin` (TemplateRef, NgContentRef).

### R1.4 — Visibilidade
- `@Directive`/`@Component`: `visibility` lida com `defaultTo: Visibility.local`
  (`FC:705-710`).
- Provedores (`CompileProviderMetadata`): `visibility = Visibility.all` (`CM:93`).
- `ProviderAst.visibleForInjection = provider.visibility == Visibility.all` (`PP:511`); os
  `ProviderAst` embutidos (TemplateRef `CE:179-187`, NgContentRef `VB:180-187`) ficam com o
  padrão `false`.

**`visibleForInjection` só é lido em um lugar**: `_createProviderInstances` (`CE:297`),
isto é, só decide se o token entra no `injectorGetInternal`. Não restringe a busca local
nem a subida estática pelos pais (R2.5, R3.4). Exemplo **(oráculo)** `j79_host_e_self`:
`J79Linha` é `Visibility.local` e mesmo assim o `J79Item` filho a recebe estaticamente —
`import1.J79Item(this._J79Linha_0_5, null, …)` (linha 170).

Rust: `Diretiva::visivel` → `Instancia::injetavel_por` (`D:resolver_com`) ✔; o porte também
empurra o token não visível para `acima` (`V`, "A visibilidade só decide o
`injectorGetInternal`") ✔ — exceto quando a instância não visível tem apelidos (ver L7).

### R1.5 — Tipo `T` do provedor (`typeArgument`)
`SM:111-127`: `providerType = inferProviderType(provider, token)`; se for `InterfaceType`,
`typeArgument = _getCompileTypeMetadata(elemento, typeArguments)`, com os argumentos
convertidos por `fromDartType` (`SM:266`); senão `null`. Um tipo solto na lista
(`providers: [Foo]`) vira `CompileProviderMetadata(token: Foo, useClass: Foo)` **sem**
`typeArgument` (`SM:95-101`). `multi = MultiToken.isAssignableFromType(token.type)`.

Rust: `Provedor::tipo: Option<TipoDeToken>` (lido em `metadados.rs`) ✔; tipos com
argumentos concretos e `T` fora de `dart:core` são recusados (`V:provedores_escreviveis`).

---

## 2. Fase de template — `ProviderElementContext` (`PP`)

### R2.1 — Contexto da visão
`ProviderViewContext(component)` (`PP:27-52`): `viewQueries` = mapa token→consultas de
`component.viewQueries`; `viewProviders` = tokens de `component.viewProviders`
normalizados (`_normalizeProviders`, achata listas; `Type` → `useClass`), dedup por token.
É o contexto raiz do template: `ProviderElementContext(ctx, null, false, [], [], [], null)`
(`TP:196-206`), sem diretivas.

Cada elemento: `ProviderElementContext(root, pai, isViewRoot, directives, attrs, refs, span)`
com `isViewRoot = false` para `ElementAst` (`TP:1272`) e **`true` para
`EmbeddedTemplateAst`** (`TP:1306`). Os filhos são visitados **dentro** do construtor do
pai e **antes** do `afterElement()` do pai (`TP:1274-1278`, `1308-1312`).

### R2.2 — Tabela de provedores do nó (`_ProviderResolver.resolve`, `PP:444-523`)
Ordem de inserção em `_allProviders`:

1. Para cada diretiva **na ordem de `ast.directives`**: `ProviderAst(token: tipo,
   providers: [useClass: tipo, visibility: directive.visibility], providerType:
   component|directive, eager: true, visibleForInjection: visibility == all)`
   (`PP:453-460`).
2. Depois, `providers:` e `viewProviders:` de cada diretiva, **componente antes das
   diretivas** (`PP:461-481`): para cada diretiva nessa ordem, primeiro os `providers`
   (`publicService`, `eager: false`), depois os `viewProviders` (`privateService`,
   `eager: false`).

`_resolveProviders` (`PP:487-522`): se o token já existe e `multiProvider != provider.multi`
→ erro "Mixing multi and non multi provider…" (e segue). Se não existe, cria o
`ProviderAst`. Se existe: não-multi **limpa** a lista e adiciona (sobrescreve, mantendo o
`ProviderAst` original — tipo, `eager`, `visibleForInjection`, posição); multi acumula.

Exemplo (derivado): `<div a b>` com `A(providers:[ClassProvider(S)])` e
`B(providers:[ClassProvider(S, useClass: S2)])` → `_allProviders = [A, B, S]`, `S.providers
= [useClass S2]`.

Rust: `D:resolver_com` (bloco `todos`) ✔ na ordem (diretivas, depois componente, depois
diretivas). Diferenças: sobrescrever o token de uma diretiva/componente com `providers:` é
recusado (`"provedor com o token do componente"`); `viewProviders:` não vazio é recusado em
`metadados.rs` (`d.fora`) — **falta** `privateService`.

### R2.3 — Tokens consultados e `requiresViewContainer` (`PP:88-104`)
`queriedTokens`: para cada provedor de `_allProviders` e para cada `#ref`
(`CompileTokenMetadata(value: nomeDaRef)`), `_addQueryReadsTo` junta `query.read ?? token`
de toda consulta que casa com o token (`_getQueriesFor`, `PP:165-187`: consultas de
conteúdo das diretivas deste elemento e dos ancestrais, filtradas por `descendants ||
distance <= 1`, `distance` incrementa em cada ancestral **com diretivas**; mais as
`viewQueries` do componente). Se `ViewContainerRef` está em `queriedTokens` →
`_requiresViewContainer = true`.

Rust: os tokens chegam prontos em `D:resolver_consultado(…, consultados)` ✔ (quem calcula
é `V`); `requiresViewContainer` por consulta com `read: ViewContainerRef` — ver L9.

### R2.4 — Ansiosos no construtor, preguiçosos no `afterElement`
- Construtor (`PP:105-112`): para cada provedor de `_allProviders`, em ordem, se
  `provider.eager || queriedTokens.contains(token)` →
  `_getOrCreateLocalProvider(provider.providerType, token, eager: true)`.
- `afterElement()` (`PP:124-130`): para **todos**, em ordem,
  `_getOrCreateLocalProvider(…, eager: false)` (os já transformados retornam cedo).

`_getOrCreateLocalProvider` só insere o token em `_transformedProviders` **depois** de
resolver as dependências (pós-ordem, `PP:218-263`). Logo a ordem final
(`transformProviders`) é:

1. ansiosos do construtor em pós-ordem de dependências;
2. **provedores que os filhos pediram** (R2.7), na ordem em que os filhos os pediram;
3. o resto em pós-ordem (`afterElement`).

`eager` final = `provider.eager || forceEager` (`PP:387-400`), onde `forceEager` é o
`eager` da **primeira** chamada que o transformou. Um provedor preguiçoso pedido por um
ansioso vira ansioso (a dependência herda `eager` em `_getLocalDependency`, `PP:303-305`).

Exemplo **(oráculo)** `j71_provedores_de_diretiva`, nó 1 `<div j71-espiao>`, com
`J71Espiao(J71Servico)` e `providers: [ClassProvider(J71Servico), ClassProvider(J71Preguicoso),
ExistingProvider(J71Rotulo, J71Externo)]`: `_allProviders = [J71Espiao, J71Servico,
J71Preguicoso, J71Rotulo]` → transformados `[J71Servico(ansioso), J71Espiao(ansioso),
J71Preguicoso(preg.), J71Rotulo(preg.)]` → campos `_J71Servico_1_5`, `_J71Espiao_1_6`,
`_J71Preguicoso_1_7`, `_J71Rotulo_1_8` (linhas 27-35).

Rust: `D:resolver_com` → `criar` (DFS pós-ordem), primeiro os `eager`, depois todos; a
fronteira `ansiosos` dá `Instancia::preguicosa` ✔. **Falta** o item 2 (L1).

### R2.5 — `_getOrCreateLocalProvider(requestingType, token, eager)` (`PP:189-264`)
Retorna `null` (não é local) se:
- o token não está em `_allProviders`; ou
- quem pede é `directive`/`publicService` e o provedor é `privateService`
  (`viewProviders` não são vistos por diretivas nem por serviços públicos); ou
- quem pede é `privateService`/`publicService` e o provedor é `builtin`.

(Quem pede `component` não tem restrição.) Se já transformado → devolve. Se já "visto" e
não transformado → erro **"Cannot instantiate cyclic dependency! {token.name}"** e `null`
(`PP:207-213`). Senão marca visto e transforma **cada** `CompileProviderMetadata` do token
(multi → vários):

- `useExisting: X` → `_getDependency(providerType, {token: X}, eager)`; se voltar token →
  `useExisting` = esse token; se voltar valor (`isValue`) → vira `useValue: valor` e
  `useExisting = null` (`PP:223-231`).
- `useFactory` → deps = `provider.deps ?? useFactory.diDeps`, cada uma por
  `_getDependency`; as `null` (erro) são descartadas (`PP:232-241`).
- `useClass` → idem com `useClass.diDeps` (`PP:242-251`).
- `useValue` → sem deps.

Note que o `requestingType` passado a `_getDependency` das dependências é o
**`providerType` do provedor sendo criado** (`PP:224`, `237`, `247`).

Rust: ciclo ✔ (`Err("dependência cíclica entre diretivas")`, recusa em vez de relatar e
seguir). As restrições por tipo não existem (não há `privateService`); a de `builtin` é
coberta indiretamente (serviço que depende de embutido é recusado).

### R2.6 — `_getLocalDependency(requestingType, dep, eager)` (`PP:266-310`)
Em ordem:
1. `@Attribute('n')` → registra `n` em `attributeDeps` e devolve
   `CompileDiDependencyMetadata(isValue: true, value: _attrs['n']?.value)` — texto literal,
   `I18nMessage`, ou `null` se o atributo não existe.
2. Se quem pede é `directive` ou `component`:
   - `ElementRef`, `HtmlElement`, `Element`, `ChangeDetectorRef`, `NgContentRef`,
     `TemplateRef` → devolve `dep` (embutido; **mesmo em elemento que não é `<template>`**);
   - `ViewContainerRef` → `_requiresViewContainer = true` e **continua** (não devolve);
   - `ComponentLoader` → `_requiresViewContainer = true` e devolve `dep`.
3. `Injector` → devolve `dep` (qualquer requerente).
4. `_getOrCreateLocalProvider(requestingType, token, eager)` não nulo → `dep`.
5. Senão `null`.

Consequência do item 2: `ViewContainerRef` nunca é "achado" no nível de template; cai no
R2.7 e, sem `@Self`, termina como `dep` (dinâmico) — na visão ele é achado como embutido
(R3.1). Com `@Self() ViewContainerRef` sem `@Optional` → erro "No provider for
ViewContainerRef"; com `@Self() @Optional()` → **valor `null`**, embora o nó ganhe o
`ViewContainer` (derivado).

Rust: `D:pede_container` marca o container só por dependência de `ViewContainerRef` ✔;
`ComponentLoader` não marca (L9). `@Attribute` → `Argumento::Atributo` ✔ (diretiva) e
`V:construcao_do_filho` (`Injetado::Atributo`, só atributo sem `{{`) ✔.

### R2.7 — `_getDependency(requestingType, dep, eager)` (`PP:318-366`)
```
se !dep.isSkipSelf: r = _getLocalDependency(requestingType, dep, eager); se r: retorna r
se dep.isSelf:
    r = dep.isOptional ? valor(null) : null
senão:
    atual = this; currEager = eager
    enquanto r == null e atual._parent != null:
        anterior = atual; atual = atual._parent
        se anterior._isViewRoot: currEager = false
        r = atual._getLocalDependency(publicService, dep, currEager)
    se r == null:
        se !dep.isHost
           ou component.type.isHost                       (visão-hospedeira)
           ou dep.token == tipo do componente do template
           ou viewProviders do componente têm o token:
            r = dep                                        (vai ao injetor)
        senão:
            r = dep.isOptional ? valor(null) : null
se r == null: erro "No provider for {token.name}"
```
Pontos exatos:
- A subida usa **`publicService`** como requerente: nos pais, embutidos (item 2 do R2.6)
  **não** são achados, e `viewProviders` (`privateService`) também não.
- A subida pelo pai **cria** o provedor do pai (se ainda não transformado) com
  `eager: currEager` — é o item 2 do R2.4. `currEager` vira `false` ao sair de um
  `<template>` (o elemento de onde se sai é `isViewRoot`); o próprio `<template>` ainda é
  consultado com o `eager` original.
- A subida vai até o contexto raiz (sem diretivas; só responde a `Injector`).
- `@Host` só é verificado **depois** de não achar em lugar nenhum do template; achar num
  ancestral qualquer do mesmo template satisfaz `@Host`.

Exemplos **(oráculo)** `j79_host_e_self` (`J79Item(@Host @Optional J79Linha,
@Self @Optional J79Servico, @SkipSelf @Optional J79Servico, @Host @Optional J79Servico)`):
- dentro de `<j79-linha>` no template: `J79Item(this._J79Linha_0_5, null,
  (this.parentView!).injectorGetOptional(import1.J79Servico, this.parentIndex), null)`
  (linha 170);
- solto no template: `J79Item(null, null, (this.parentView!).injectorGetOptional(…), null)`
  (linha 182);
- na visão-hospedeira (`isHost`): `@Host` vai ao injetor —
  `J79Item(this.injectorGetOptional(import1.J79Linha, this.parentIndex), null,
  this.injectorGetOptional(import1.J79Servico, …), this.injectorGetOptional(import1.J79Servico, …))`
  (linha 130).

Rust: diretiva → `D:fora_do_no` (sobe por `Acima`, `@Self`/`@Host`/`@Optional`) e o filho
→ `V:construcao_do_filho` (`OrigemDoServico`). `@Host` compara só com o próprio componente
(`proprio_componente`); **falta** a exceção `viewProviders` (L6). `fora_do_no` recusa
`@Host` sem provedor acima em vez de ir ao injetor quando o token é o componente (L6).

### R2.8 — Saída da fase (`PP:132-147`)
- `transformProviders` = `_transformedProviders.values` (ordem do R2.4) → vira
  `ElementAst.providers` (`TP:1286`) e, no `<template>`, também `ast.providers.addAll`
  (`TP:1313`).
- `transformedDirectiveAsts` = `ast.directives` ordenadas pelo índice do tipo da diretiva
  em `transformProviders` (as diretivas são **instanciadas e ligadas** nessa ordem, não na
  de `directives:`).
- `requiresViewContainer` → `ElementAst.hasViewContainer` (`TA:249-250`, `301-302`).

Rust: `NoResolvido::diretivas` na ordem dos provedores ✔.

---

## 3. Fase de visão — `CompileElement` + `ProviderResolver`

### R3.1 — Embutidos e o `uniqueId`
`CompileElement` cria `ProviderResolver(this, parent?._providers)` e adiciona embutidos
(`CE:100-130`, `191-206`), nesta ordem — cada `add` ocupa uma posição em `_instances`:

| # | Token | Expressão | Quando |
|---|---|---|---|
| 0 | `ElementRef` | `ElementRef(renderNode)` | sempre |
| 1 | `Element` | `renderNode` | sempre |
| 2 | `HtmlElement` | `renderNode` | sempre |
| 3 | `Injector` | `this.injector(nodeIndex)` (`InvokeMemberMethodExpr`) | sempre |
| – | `ViewContainer` | `this._appEl_n` | `hasViewContainer \|\| hasEmbeddedView` |
| – | `ViewContainerRef` | `this._appEl_n` | `hasViewContainer` (em `beforeChildren`) |
| – | `ChangeDetectorRef` | `componentView ?? this` (`_compView_n` no nó de componente) | sempre |
| – | `ComponentLoader` | `this._appEl_n` | `appViewContainer != null` |

Depois `addDirectiveProviders` (R3.3) dá a cada provedor `uniqueId = _instances.length`
no momento (`PR:130-134`) — aliases também ocupam posição (`PR:128`). Primeiro índice livre:

- elemento sem container: **5**;
- elemento com `hasViewContainer`: **8**;
- `<template>` (`hasEmbeddedView = true` sempre, `VB:420-432`): `ViewContainer` e
  `ComponentLoader` sempre; `ViewContainerRef` só se `hasViewContainer` → **7** ou **8**,
  ocupado pelo `TemplateRef`, e as diretivas começam em 8 ou 9.

Exemplo **(oráculo)** `j71`, nó 2 `<p *ngIf>`: `var _TemplateRef_2_8 = TemplateRef(this._appEl_2,
viewFactory_J71ProvedoresDeDiretiva1);` e `this._NgIf_2_9 = NgIf(this._appEl_2,
_TemplateRef_2_8);` (linhas 70-71).

Rust: `D:resolver_com` (`tamanho` = 5/8, ou índice do `TemplateRef` + 1) e
`D:resolver_de_molde` (7/8) ✔; `+1` por apelido ✔.

### R3.2 — `TemplateRef` e `NgContentRef`
`setEmbeddedView` (`CE:160-189`, chamado de `CV:489-491`) insere **na posição 0** de
`_resolvedProvidersArray` um `ProviderAst(TemplateRef, builtin, eager: true,
useValue: TemplateRef(this._appEl_n, viewFactory_…), isReferencedOutsideBuild:
_publishesTemplateRef || há consulta por TemplateRef)`. `_publishesTemplateRef` = alguma
`#ref` do nó resolve para `TemplateRef` (`CE:101-107`).
`<ng-content #r>` cria um `CompileElement` com `nodeIndex = ast.index` e um único
`ProviderAst(NgContentRef, builtin, eager: true)` (`VB:175-205`).

Rust: `TemplateRef` ✔ (`V`, `var _TemplateRef_n_k` ou `late final TemplateRef`); `NgContentRef`
como provedor — não verificado no porte.

### R3.3 — `addDirectiveProviders` (`PR:58-137`)
Para cada `ProviderAst` na ordem de `_resolvedProvidersArray`, para cada
`CompileProviderMetadata`:

- `useExisting: X`:
  - se `_instances` **já** contém `X` e o provedor **não** é multi → **apelido local**:
    nenhum campo; `_instances[token] = _instances[X']` onde `X'` é o alvo final (se `X` já é
    apelido, segue para o alvo dele); registra `_aliases[X'] += token` e
    `_aliasedProviders[token] = X'` (`PR:114-128`). Vale também para alvo **embutido**
    (`ElementRef` etc.), e nesse caso o apelido nunca entra no `injectorGetInternal`
    (o alvo não está em `_resolvedProvidersArray`) (derivado).
  - senão → `_getDependency({token: X})` (R3.4): leitura do campo local (multi), leitura de
    um pai, ou `injectorGet`.
- `useFactory` → `FactoryProviderSource(fn, deps)`; `useClass` →
  `ClassProviderSource(classe, deps)` e, se a classe é uma das diretivas do nó,
  `directiveMetadata` = ela; senão (`useValue`) → `ExpressionProviderSource(
  convertValueToOutputAst(useValue))`.

Se não apelido: `_instances[token] = createProviderInstance(ast, directiveMetadata,
sources, uniqueId)` → nome do campo `'_${token.name}_${nodeIndex}_$uniqueId'`
(`CE:385`) e `CompileView.createProvider` (R3.7).

Exemplo **(oráculo)** `j71`: `ExistingProvider(J71Rotulo, J71Externo)` sem `J71Externo` no nó
→ `late dynamic _J71Rotulo_1_8 = (this.parentView!).injectorGet(import1.J71Externo,
this.parentIndex);` (linha 32). `NgForm` com `providers: [ExistingProvider(ControlContainer,
NgForm)]` → apelido, sem campo (`a22_form_com_forms_directives`, R4.3).

Rust: `D:resolver_com` — apelido local (`apelidos`, só não multi, alvo já em `campos` ou
apelido) ✔; alvo embutido recusado (`"apelido de embutido do elemento"`); multi de apelidos
locais → `Criacao::Lista` ✔; `Criacao::Expressao`/`Multi` para `useClass`/`useFactory`/
`useValue` ✔.

### R3.4 — Resolução estática de dependência (`ProviderResolver._getDependency`, `PR:172-205`)
```
se dep.isValue: r = Expression(literal(value))   (ou createI18nMessage se I18nMessage)
se r == null e !dep.isSkipSelf: r = _instances[token]
atual = this
enquanto r == null e atual._parent._parent != null:
    atual = atual._parent; r = atual._instances[token]
retorna DynamicProviderSource(token, elemento, atual, r, isOptional)
```
- **Não** olha `isSelf`/`isHost`, visibilidade nem tipo do provedor: tudo isso já foi
  resolvido na fase de template (R2.7). `_instances` contém embutidos, diretivas
  `Visibility.local`, `viewProviders` e apelidos. Ex.: um `ClassProvider(S)` cujo
  construtor pede `ElementRef` recebe `ElementRef(_el_n)` do próprio nó (a fase de
  template o marcara como dinâmico) (derivado).
- A subida para no último elemento cujo pai é o `CompileElement.root()` (o `root` também
  tem embutidos, mas nunca é consultado).
- **Pai de nó de visão embutida**: os filhos de um `<template>` são visitados com pai =
  `declarationElement.parent ?? declarationElement` (`VB:461-464`), isto é, **o pai do
  `<template>`, não o `<template>`**. A cadeia estática pula os provedores do próprio
  `<template>` (a diretiva estrutural, `TemplateRef`, `ViewContainerRef`), que só são
  alcançados pelo `injectorGet` dinâmico (runtime: `parentView.injectorGet(token,
  parentIndex)` com `parentIndex` = índice do `<template>`) (derivado).

### R3.5 — `DynamicProviderSource.build` (`PR:322-350`) e `getPropertyInView` (`VU:44-74`)
`build()`:
1. `valor = source?.build() ?? injectFromViewParentInjector(view do elemento, token,
   optional)`;
2. `pai = findElementByResolver(atual)` (o elemento de onde veio o resultado, ou o mais
   alto da cadeia quando não achou);
3. `getPropertyInView(valor, view do elemento, pai.view)`.

`injectFromViewParentInjector` (`VU:112-124`): receptor `this` na visão-hospedeira, senão
`this.parentView!`; método `injectorGetOptional` se `@Optional`, senão `injectorGet`;
argumentos `[createDiTokenExpression(token), this.parentIndex]`.

`getPropertyInView`: mesma visão → valor intacto. Senão sobe `declarationElement.view` e
monta `viewProp = parentView!` / `viewProp.parentView!`… e troca cada
`ReadClassMemberExpr`/nó promovido por `viewProp.nome` — sem cast para `parentView`,
`parentIndex`, `componentStyles`; com `unsafeCast<ClasseDaVisão>(viewProp)` para os demais.

Formas exatas **(oráculo)**:
- mesma visão, achado no nó ou num pai: `this._J71Grupo_0_5`;
- achado numa visão ancestral: `import2.unsafeCast<ViewJ71ProvedoresDeDiretiva0>((this.parentView!))._J71Grupo_0_5`
  (`j71`, linha 151);
- não achado, visão de componente: `(this.parentView!).injectorGet(import1.J71Externo, this.parentIndex)`;
- não achado, a partir de uma embutida (o mais alto da cadeia está na visão do componente):
  `((this.parentView!).parentView!).injectorGetOptional(import1.J71Config, (this.parentView!).parentIndex)`
  (`j71`, linha 151);
- não achado, visão-hospedeira: `this.injectorGet(import1.J61Externo, this.parentIndex)`
  (`j61_provedor_do_proprio_no`, linha 82).

Rust: `V:provedores_acima_com` (`unsafeCast<C>((this.parentView!))…`), `V:visao_do_injetor`
(`(this.parentView!)`, `((…).parentView!)`), `V:texto_da_expr` (`Expr::Injetor`,
`vista`), `V:construcao_do_filho`, `V:construcao_do_componente` (hospedeira) ✔. **Diverge**
do R3.4 quanto ao `<template>`: o porte empurra os provedores do `<template>` em `acima`
antes do conteúdo (`V`, "Os provedores do `<template>` ficam acima do conteúdo dele") e os
leria por `unsafeCast` (L2).

### R3.6 — Criação dinâmica e `debugInjectorWrap`
`hasDynamicDependencies` (`PS:33`, `PR:256-263`, `349`): `DynamicProviderSource` é dinâmico
sse `source == null` (foi ao `injectorGet`) ou a fonte achada é dinâmica
(`_source?.hasDynamicDependencies != false`). `ClassProviderSource`/`FactoryProviderSource`
são dinâmicas se algum parâmetro é. Se sim, `build()` devolve
`debugInjectorWrap(createDiTokenExpression(token), criação)` (`PR:278-281`, `312-315`):

```dart
(import6.isDevMode
    ? import11.debugInjectorWrap(import1.J61ComConfig, () {
        return import1.J61ComConfig(this.injectorGet(import1.J61Externo, this.parentIndex), this._J61Config_0_5);
      })
    : import1.J61ComConfig(this.injectorGet(import1.J61Externo, this.parentIndex), this._J61Config_0_5))
```
(`VU:136-145`; **oráculo** `j61`, linhas 80-84). O token do wrap é o **token do provedor**
(para diretiva, a classe). A criação da diretiva/componente com dependência dinâmica usa a
mesma fonte (`ClassProviderSource`).

Rust: `Expr::dinamica` (só `Expr::Injetor` direto) e os textos em `V:texto_da_expr`,
`V:construcao_do_filho`, `V:construcao_do_componente` ✔. Diferença: `Expr::dinamica` não
propaga dinamismo de um `Expr::Campo`/`Leitura` (no oficial, local nunca é dinâmico — ok) ✔.

### R3.7 — `createDiTokenExpression` (`VU:186-206`)
- `identifierIsInstance` (MultiToken/OpaqueToken): `const Classe<T…>(valor?)` — o valor
  literal como primeiro argumento se houver; genéricos = `identifier.typeArguments` se
  não vazio, senão nenhum. Ex. **(oráculo)**: `const import12.OpaqueToken<String>('j40.token')`,
  `const import18.MultiToken<Object>('NgValidators')`.
- só `value`: `literal(value)` (token de texto: `'x'`).
- senão: a classe importada (`import1.J61Externo`).

Rust: `V:expr_do_token` ✔ (genérico `OpaqueToken<T>` de fora do `dart:core` recusado).

### R3.8 — Forma do campo (`CompileView.createProvider`, `CV:1134-1283`)
**Tipo** (`CV:1143-1175`):
- multi: `List<T>` com `T = importType(typeArgument, typeArgument.typeArguments)` ou
  `dynamic` → `List<dynamic>`;
- não multi: diretiva/componente (`directiveMetadata != null`) → `originType` com os
  argumentos de `lookupTypeArgumentsOf` (`Typed`, `CV:1523-1569`); senão `typeArgument`;
  senão **o tipo da expressão** (`resolvedProviderValueExpr.type`); `null` → `dynamic`.
  O tipo da expressão: `InstantiateExpr` com `type: importType(classe)` → a classe;
  `debugInjectorWrap` é um `ConditionalExpr` cujo tipo é o do `trueCase` (uma chamada, sem
  tipo) → **`dynamic`** (`OA:640-646`); `injectorGet` → `dynamic`; literal → o tipo do
  literal convertido.

`providerHasChangeDetector` = `directive` && `requiresDirectiveChangeDetector`
(`metadataType == directive && hostProperties.isNotEmpty`, `CM:545-547`).

**Formas** (modificadores emitidos por `DE:visitDeclareVarStmt`, `DE:132-162`):

| Caso | Campo | `build()` | Leitura |
|---|---|---|---|
| ansioso, `isReferencedOutsideBuild`, com `XNgCd` | `late final XNgCd<Args> _X_n_k;` | `this._X_n_k = XNgCd(X(…));` | `this._X_n_k.instance` |
| ansioso, hospedeira, `providerType == component` | — (`HostView.component`, `K:9`) | `this.component = X(…);` | `this.component` |
| ansioso, `isReferencedOutsideBuild` | `late final T _X_n_k;` | `this._X_n_k = expr;` | `this._X_n_k` |
| ansioso, **não** referenciado fora (só `TemplateRef`) | — | `var _TemplateRef_n_k = expr;` | local |
| preguiçoso | `late T _X_n_k = expr;` (`XNgCd(expr)` se tiver CD) | — | `this._X_n_k` |

Diretivas e componentes são sempre ansiosos (R2.2), então a linha "preguiçoso com `XNgCd`"
não ocorre na prática. `var` porque `WriteVarExpr.toDeclStmt()` não passa tipo
(`OA:462-465`).

**Ordem dos campos na classe** (`DE:198-205`): primeiro **todos** os campos com
inicializador (os preguiçosos), depois os sem, cada grupo na ordem de alocação.

Exemplos **(oráculo)** `j71`, visão 0 (linhas 27-37):
```dart
  late dynamic _J71Preguicoso_1_7 = (import2.isDevMode
      ? import3.debugInjectorWrap(import1.J71Preguicoso, () {
          return import1.J71Preguicoso((this.parentView!).injectorGet(import1.J71Externo, this.parentIndex));
        })
      : import1.J71Preguicoso((this.parentView!).injectorGet(import1.J71Externo, this.parentIndex)));
  late dynamic _J71Rotulo_1_8 = (this.parentView!).injectorGet(import1.J71Externo, this.parentIndex);
  late final import1.J71Grupo _J71Grupo_0_5;
  late final dynamic _J71Servico_1_5;
  late final import1.J71Espiao _J71Espiao_1_6;
```
Tipo pela expressão: `late final import1.J61Config _J61Config_0_5;` (`j61`, linha 74);
multi: `late List<import10.ControlValueAccessor<dynamic>> _NgValueAccessor_0_6 = [this.component];`
e `late List<Object> _i64Validadores_0_7 = [import1.I64Validador(), this.component];`.

Rust: `V:tipo_do_provedor` (multi → `List<T|dynamic>`; `T`; classe não dinâmica; literais
`String`/`int`/`bool`; objeto const; dinâmica/fábrica/injetor → `dynamic`) ✔;
`V:texto_de_provedor_preguicoso` (`late T _X = v;`) ✔; `V:escrever_provedores_da_hospedeira`
(preguiçosos antes, `late final` + atribuição) ✔; `XNgCd` → `Instancia::leitura =
campo.instance` ✔. `useFactory` sem `T` sai `dynamic` no porte — no oficial é o tipo de
`InvokeFunctionExpr`, também sem tipo → `dynamic` ✔. Tipo de `Expr::Campo`/`Leitura` (um
`useExisting` não apelido para campo) é recusado no porte (L8).

### R3.9 — Ordem das instruções no `build()`
Por elemento, na visita em pré-ordem (`VB:208-413`): criação do nó e atributos; para
componente, o `_compView_n` antes (`createComponentNodeAndAppend`, `VB:283-290`); o
`ViewContainer` no construtor do `CompileElement` (`CV:942-972`); em `beforeChildren`
(`VB:260-263`): as instruções dos provedores **na ordem de `_resolvedProvidersArray`**
(R2.4/R3.2), depois `registerDirectives`; depois os filhos; para componente,
`_compView_n.create(instância, projeções)` após os filhos (`VB:342-354`).

Exemplo **(oráculo)** `j61`, visão 0, nó 1 (linhas 216-222):
```dart
    this._J61Config_1_5 = import1.J61Config();
    this._J61ComConfig_1_6 = (import6.isDevMode … );
    this._compView_1.create(this._J61ComConfig_1_6);
```
Rust: `V:criar_instancias` na ordem de `NoResolvido::instancias` ✔.

---

## 4. `injectorGetInternal` e visibilidade

### R4.1 — Instâncias injetáveis de um nó (`CE:284-317`)
Para cada `ProviderAst` de `_resolvedProvidersArray` (inclui `TemplateRef`):
- pula se é apelido (`isAliasedProvider`);
- `tokens = [token se visibleForInjection] + _aliases[token]`;
- pula se `tokens` vazio;
- `ProviderInstance(tokens, _instances[token].build())` → lista de `viewProviders` se
  `privateService`, senão `providers`.

### R4.2 — Floresta (`CE:323-353`, `VB:266-273`, `PF`)
`childNodeCount = nodes.length − nodeIndex − 1` após os filhos. `ProviderNode(start =
nodeIndex, end = nodeIndex + childNodeCount, providers, children)`; se há filhos e
`viewProviders`, estes viram um filho extra `ProviderNode(n, n)` **no fim** de `children`;
sem filhos, ambos vão no mesmo nó. Nós sem provedores são substituídos pelos filhos
(`expandEmptyNodes`, `PF:157-176`).

`_build(nós, alvo, baixo, alto)` (`PF:44-88`), raiz com `(0, −1)`:
- nó sem filhos e com **um** provedor:
  `if ((tokenCond && indexCond)) { return expr; }`;
- senão: `if (indexCond) { <filhos recursivos com (start,end)>; if (tokenCond) { return expr; } … }`
  — filhos **antes** dos provedores do próprio nó.

`indexCond` (`PF:96-117`): `start == end` → `(start == nodeIndex)`; `start == baixo` →
`(nodeIndex <= end)`; `end == alto` → `(start <= nodeIndex)`; senão
`((start <= nodeIndex) && (nodeIndex <= end))`. `tokenCond`: `identical(token, T)` unidos
por `||` associando à esquerda (`PF:120-125`).

Método (`CV:1466-1480`, `1583-1590`):
```dart
  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    <instruções>
    return notFoundResult;
  }
```
omitido se não há instruções (`VB:586`, métodos de corpo vazio são filtrados).

### R4.3 — Exemplos **(oráculo)**
`a22_form_com_forms_directives` (`NgForm` `Visibility.all` + apelido `ControlContainer`, nó 0
com 2 descendentes):
```dart
    if (((identical(token, import2.NgForm) || identical(token, import10.ControlContainer)) && (nodeIndex <= 2))) {
      return this._NgForm_0_5;
    }
```
`j61`, visão 0 (componentes `Visibility.local`; só os `providers:` entram):
```dart
    if ((identical(token, import1.J61Config) && (1 == nodeIndex))) {
      return this._J61Config_1_5;
    }
    if ((2 == nodeIndex)) {
      if (identical(token, import1.J61Servico)) {
        return this._J61Servico_2_5;
      }
      if (identical(token, import1.J61Config)) {
        return this._J61Config_2_6;
      }
    }
```
Provedor preguiçoso aparece pelo campo (`return this._J71Preguicoso_1_7;`, `j71` linha 84).

Rust: `V:metodo_injetor` (floresta por contenção, condições, `||` à esquerda, formato) ✔;
entradas montadas de `injetavel_por` ✔. Não há `viewProviders` (nó extra `[n,n]`) — L5.

---

## 5. Visão-hospedeira

- `component.type.isHost` → `ViewType.host` (`CV:1572-1581`); o template é
  `<seletor></seletor>` com o componente como única diretiva (`angular_compiler.dart:149-170`).
- Contexto de template com `isHost`: toda dependência não achada, com ou sem `@Host`, vai
  ao injetor (R2.7).
- `injectFromViewParentInjector` usa `this` como receptor (`VU:117-119`):
  `this.injectorGet(T, this.parentIndex)`.
- O componente não ganha campo: `this.component = …` (R3.8), mas consome o `uniqueId`
  (**oráculo** `j61`: `late import1.J61Externo _J61Externo_0_6 = import1.J61Externo();`,
  linha 303, com `this.component = import1.J61ProvedorDoProprioNo();` na 308).
- Os `providers:` do componente viram campos do nó 0 e entram no `injectorGetInternal` com
  `(0 == nodeIndex)` (o nó não tem filhos).
- `ChangeDetectorRef` para o componente = `componentView` do nó (`CE:198-199`).

Rust: `D:resolver_hospedeira` + `V:provedores_da_hospedeira`,
`V:escrever_provedores_da_hospedeira`, `V:construcao_do_componente`,
`V:dependencia_anotada` ✔; recusa provedor ansioso depois do componente (só ocorre por
consulta de conteúdo, R2.3) e item de multi dinâmico.

---

## 6. Tabela de cobertura

| Regra | Rust | Estado |
|---|---|---|
| R1.1 nome/igualdade de token | `D:Token::nome`, `PartialEq` | ✔ |
| R1.2 erro de token duplicado | — | recusa antes |
| R1.4 visibilidade → só `injectorGetInternal` | `D:resolver_com` `injetavel_por`, `V` `acima` | ✔ (L7) |
| R1.5 `typeArgument` | `Provedor::tipo` | parcial (genéricos concretos recusados) |
| R2.2 tabela, sobrescrita, multi | `D:resolver_com` | parcial (sem `viewProviders`, sem sobrescrever diretiva) |
| R2.3 `queriedTokens` | `D:resolver_consultado` | parcial (L9) |
| R2.4 ordem ansiosos/preguiçosos | `D:resolver_com::criar` | parcial (L1) |
| R2.5 ciclo / restrições por tipo | `criar` | ciclo ✔; tipos — |
| R2.6 embutidos, `@Attribute` | `Argumento::*`, `Injetado::*` | parcial (L9) |
| R2.7 `@Self/@Host/@SkipSelf/@Optional` | `D:fora_do_no`, `V:construcao_do_filho` | parcial (L3, L6) |
| R2.8 ordem das diretivas | `NoResolvido::diretivas` | ✔ |
| R3.1 `uniqueId` | `tamanho`, `resolver_de_molde` | ✔ |
| R3.3 apelidos, fontes | `D:resolver_com` | ✔ (alvo embutido recusado) |
| R3.4 subida estática; pai do `<template>` | `V:provedores_acima_com` | diverge (L2) |
| R3.5 `getPropertyInView`, `injectorGet` | `V:visao_do_injetor`, `texto_da_expr` | ✔ |
| R3.6 `debugInjectorWrap` | `Expr::dinamica`, `V:*` | ✔ |
| R3.7 `createDiTokenExpression` | `V:expr_do_token` | ✔ (parcial em genéricos) |
| R3.8 forma/tipo do campo | `V:tipo_do_provedor` e afins | ✔ (L8) |
| R4.* `injectorGetInternal` | `V:metodo_injetor` | ✔ (L5) |
| R5 hospedeira | `V:provedores_da_hospedeira` | ✔ |

---

## 7. Lacunas do porte (prioridade decrescente)

1. **L1 — Provedor do pai criado pelo filho (R2.4 item 2, R2.7).** Um filho que depende de
   um `providers:` ainda não transformado do elemento pai o cria **durante a visita do
   filho**, antes do `afterElement` do pai: ele fica **ansioso** (se o filho é ansioso e não
   se cruzou `<template>`) e **muda de posição** em `transformProviders` — logo muda o
   `uniqueId` (nome do campo), a forma (`late final` + atribuição no `build()` em vez de
   `late … = …`) e a ordem das instruções. `D:resolver_com` resolve cada nó isolado e
   marcaria o provedor como preguiçoso e na ordem de `providers:`. Saída errada, não
   recusa. (Ex. derivado: pai `P(providers:[A, B])` sem depender deles, filho `C(B)` →
   oficial `_B_n_6` ansioso, `_A_n_7` preguiçoso; porte `_A_n_6`, `_B_n_7`, ambos
   preguiçosos.)
2. **L2 — Filhos de `<template>` não veem estaticamente os provedores do `<template>`
   (R3.4).** Pelo código (`VB:461-464`), o pai dos nós da embutida é o pai do `<template>`;
   a diretiva estrutural e os `providers:` dela só chegam por
   `((…parentView!).injectorGet(T, …parentIndex))` com `debugInjectorWrap`. O porte os
   empurra em `acima` e escreveria `unsafeCast<…>((this.parentView!))._X_t_k`. Confirmar
   com um caso de corpus (diretiva em `<template>` com `providers:`/`Visibility.all`
   injetada pelo conteúdo) antes de mudar.
3. **L3 — `@SkipSelf` ainda cria o provedor local (R2.7).** Em `D:resolver_com::criar`, a
   dependência de diretiva chama `pedir` mesmo com `dep.pular`; o oficial não consulta o
   nó, então o provedor local não é forçado a ansioso nem reordenado.
4. **L4 — `viewProviders:` (`privateService`).** Recusado em `metadados.rs`. Falta: tabela
   (R2.2), restrições de visão na fase de template (R2.5 — diretiva e serviço público não
   veem; a fase de visão vê tudo, R3.4), `@Host` permitido pelo token em `viewProviders`
   (R2.7) e o nó `[n,n]` do `injectorGetInternal` (R4.2).
5. **L5 — Nó extra de `viewProviders` no `injectorGetInternal`** (dependente de L4).
6. **L6 — Exceções do `@Host` (R2.7).** Com `@Host` sem provedor no template, o oficial vai
   ao injetor se o token é o próprio componente ou está nos `viewProviders` dele.
   `V:construcao_do_filho` cobre só o componente; `D:fora_do_no` recusa ambos.
7. **L7 — Diretiva não visível com apelidos.** Com `injetavel_por` não vazio só por
   apelidos, `V` não empurra o token da própria diretiva em `acima`; o oficial a acha pelo
   token na subida estática (`_instances`).
8. **L8 — Tipo de campo de `useExisting` não apelido para leitura local/de pai** (multi
   fica como lista; não multi com `Expr::Campo`/`Leitura`) recusado em `V:tipo_do_provedor`;
   o oficial usa o tipo da expressão de leitura (`ReadClassMemberExpr` tipado, ou o
   `unsafeCast`).
9. **L9 — `requiresViewContainer` por outras vias.** O oficial também o liga por dependência
   de `ComponentLoader` e por consulta com `read: ViewContainerRef` (R2.3, R2.6); o porte
   só por dependência de `ViewContainerRef` (`D:pede_container`). Muda o `uniqueId` (5→8)
   e cria o `_appEl_n`.
10. **L10 — Embutidos em serviços e subida de embutidos (R3.4).** Serviço (`providers:`) que
    depende de `ElementRef`/`ChangeDetectorRef`/`Injector` recebe o embutido do próprio nó
    (`ElementRef(_el_n)`, `this`/`_compView_n`, `this.injector(n)`); `@SkipSelf` de
    embutido lê o do pai. O porte recusa (`"provedor que depende de embutido do elemento"`,
    `"dependência @SkipSelf de Injector"`).
11. **L11 — Apelido de embutido (R3.3).** `ExistingProvider(X, ElementRef)` vira apelido sem
    campo e fora do `injectorGetInternal`; o porte recusa.
12. **L12 — `@Self() @Optional() ViewContainerRef`** recebe `null` (R2.6) mesmo com o
    `ViewContainer` criado; sem caso no porte (derivado).
