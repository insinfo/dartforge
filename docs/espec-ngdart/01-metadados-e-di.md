# 01 — Metadados de diretivas, componentes, pipes e injeção de dependência

Seção da especificação do compilador AngularDart (`ngcompiler-3.0.0-dev.3` +
runtime `ngdart-8.0.0-dev.4`), derivada da leitura do código-fonte, para o porte
em Rust (`crates/gerador_ng`). Cobre: extração de metadados de
`@Component`/`@Directive`/`@Pipe`, entradas/saídas e herança, `@HostBinding`/
`@HostListener`, consultas, ganchos de ciclo de vida, `changeDetection`,
`encapsulation`, `visibility`, provedores, tokens, dependências de construtor e
o `@GenerateInjector` (leitura e emissão).

Tudo que está aqui é o que o código mostra. Quando o comportamento depende do
`package:analyzer` (ordem de filhos, `allSupertypes`), a fonte citada é o
`analyzer-6.11.0` do pub-cache (o `pubspec` do ngcompiler pede `analyzer: ^6.0.0`).

## 0. Convenções

Caminhos relativos a `/root/.pub-cache/hosted/pub.dev/ngcompiler-3.0.0-dev.3/lib/`:

| Sigla | Arquivo |
|---|---|
| FC | `v1/src/source_gen/template_compiler/find_components.dart` |
| TCM | `v1/src/source_gen/template_compiler/compile_metadata.dart` |
| AI | `v1/src/source_gen/template_compiler/annotation_information.dart` |
| DOU | `v1/src/source_gen/template_compiler/dart_object_utils.dart` |
| LH | `v1/src/source_gen/template_compiler/lifecycle_hooks.dart` |
| PV | `v1/src/source_gen/template_compiler/pipe_visitor.dart` |
| PI | `v1/src/source_gen/template_compiler/provider_inference.dart` |
| CB | `v1/src/source_gen/template_compiler/code_builder.dart` |
| AM | `v1/src/source_gen/common/annotation_matcher.dart` |
| UR | `v1/src/source_gen/common/url_resolver.dart` |
| CM | `v1/src/compiler/compile_metadata.dart` |
| AC | `v1/src/compiler/analyzed_class.dart` |
| CONV | `v1/src/compiler/output/convert.dart` |
| TYP | `v1/src/angular_compiler/analyzer/types.dart` |
| COM | `v1/src/angular_compiler/analyzer/common.dart` |
| LNK | `v1/src/angular_compiler/analyzer/link.dart` |
| DIR | `v1/src/angular_compiler/analyzer/view/directive.dart` |
| TR | `v1/src/angular_compiler/analyzer/view/typed_reader.dart` |
| TOK | `v1/src/angular_compiler/analyzer/di/tokens.dart` |
| PRV | `v1/src/angular_compiler/analyzer/di/providers.dart` |
| DEP | `v1/src/angular_compiler/analyzer/di/dependencies.dart` |
| MOD | `v1/src/angular_compiler/analyzer/di/modules.dart` |
| INJ | `v1/src/angular_compiler/analyzer/di/injector.dart` |
| GSS | `v1/src/angular_compiler/analyzer/di/global_singleton_services.dart` |
| EMI | `v1/src/angular_compiler/emitter/injector.dart` |
| OUT | `v1/src/angular_compiler/outliner.dart` |
| NUL | `v2/analyzer.dart` (extensão `isExplicitlyNullable`/`isExplicitlyNonNullable`) |

Runtime: `RT/…` = `/root/.pub-cache/hosted/pub.dev/ngdart-8.0.0-dev.4/lib/src/meta/…`.
Porte: `RS/…` = `/home/user/dartforge/crates/gerador_ng/src/…`.

Estado no porte: **Coberto**, **Parcial**, **Falta**, **Recusa** (o porte não gera e
registra o motivo — saída ausente, não errada), **Divergente** (gera algo diferente).

O porte lê metadados em duas camadas:
- `RS/componente.rs` — leitura **sintática** da classe do próprio arquivo
  (`ler`, `ler_pipe`), usada para o componente sendo gerado;
- `RS/metadados.rs` — leitura **resolvida** pelo programa (`ler` → `Leitor::diretiva`;
  `ler_injetores`), usada para diretivas filhas, provedores e injetores;
- `RS/injetor.rs` — emissão do `@GenerateInjector`.

### 0.1 Utilitários de leitura de constantes (valem para toda a seção)

- **R0.1 `getField` percorre `(super)`**: `getField(obj, f)` devolve `obj.getField(f)`
  se não nulo, senão recursa em `obj.getField('(super)')` (DOU:116-123). Logo um
  campo de `Directive` é lido também num `Component` (que estende `Directive`).
- **R0.2 `coerceX`**: `coerceBool/Int/String/List/StringList` devolvem o padrão se o
  campo falta ou não é do tipo (DOU:10-55). `coerceStringList` **descarta em silêncio**
  itens que não são `String` (`whereType<String>()`, DOU:53).
- **R0.3 `coerceEnum`**: campo nulo → padrão; senão procura o valor pelo **nome**
  (`'$field'.split('.')[1]` como campo não nulo no objeto) e depois pelo `index`;
  se nenhum, `ArgumentError` (DOU:62-80, 99-107).
- **R0.4 `visitAll`**: percorre uma lista de `DartObject`; item que é lista é
  achatado recursivamente; o resultado nulo do visitante é descartado (DOU:129-144).
- **R0.5 Correspondência de anotação é por tipo exato**: `TypeChecker.fromUrl(
  'package:ngdart/src/meta/<arquivo>.dart#<Classe>')` (TYP:17-62) com
  `isExactlyType` (AI:41-54, AM:24-32,45-49). Subclasse de `Input` não conta.
  *Porte*: **Coberto** em `RS/metadados.rs:classe_da_anotacao` (189-201) e
  `anotacao_do_ngdart` (207-235), que resolvem o nome para a classe e comparam
  `uri`+nome. `RS/componente.rs` compara só o **nome escrito**
  (`crate::nome_da_anotacao`) — **Parcial** (um `Input` de outro pacote seria aceito).
- **R0.6 `moduleUrl(element)`** (UR:8-23): `dynamic` e parâmetro de tipo → `null`;
  `dart:` mantém a URI; demais viram `asset:` (via `toAssetUri`).
- **R0.7 Nulabilidade** (NUL:4-33): `isExplicitlyNonNullable` = não-dynamic e sufixo
  `none` (exceto `FutureOr<T?>`); `isExplicitlyNullable` = sufixo `?` ou
  `FutureOr<T?>`. `dynamic` não é nenhum dos dois.

---

## 1. Descoberta das classes

### R1.1 Visita da biblioteca
`findComponentsAndDirectives` aplica `_NormalizedComponentVisitor` (um
`RecursiveElementVisitor`) à biblioteca; para cada `ClassElement`, roda um
`_ComponentVisitor` novo; se o resultado é componente, resolve `directives`,
`directiveTypes`, `pipes` e empilha `NormalizedComponentWithViewDirectives`; senão
empilha em `directives` (FC:36-96). Profundidade de normalização = 1 (FC:68-71).
- *Porte*: `RS/lib.rs:achar` (253-330) acha por nome de anotação; o índice
  (`RS/lib.rs:Indice`, `juntar` 694, `expandir` 915) resolve os filhos. **Parcial**
  (ver R2.10).

### R1.2 Escolha da anotação
Percorre `element.metadata` na ordem (FC:233-251):
1. `computeConstantValue()` nulo → `handleWarning(AngularAnalysisError)` e segue;
2. a **primeira** anotação que casa `isDirective` (= `Component` ou `Directive`
   exatos, AM:24-29) vira `directiveInfo`;
3. anotação cujo tipo é exatamente `_ChangeDetectionLink`
   (`RT/change_detection_link.dart`, constante `changeDetectionLink`) vira `linkInfo`;
4. pára quando os dois foram achados.
Sem `directiveInfo` → `null`. Classe privada → `log.severe('Components and
directives must be public: …')` e `null` (sem exceção) (FC:253-257).

Exemplo: `@Directive(selector: 'a') @Component(selector: 'b') class X {}` → vale a
`@Directive` (primeira).
- *Porte*: `RS/metadados.rs:diretiva` (237-249) pega a primeira anotação que resolve
  para `Directive`/`Component` do `directives.dart` — **Coberto**. Classe privada:
  **Falta** a recusa. `changeDetectionLink`: **Falta** (ver R2.6).

### R1.3 Ordem interna de `_createCompileDirectiveMetadata` (FC:623-713)
1. `_directiveClassElement = element`;
2. `DirectiveVisitor(onHostBinding, onHostListener).visitDirective(element)` (§4);
3. `_collectInheritableMetadata(element)` (§3);
4. **só então** `directiveInfo.hasErrors` → `handle(AngularAnalysisError)`, `null`;
5. `CompileTypeMetadataVisitor` → `componentType` (§8.1);
6. template (componente) ou `CompileTemplateMetadata()` vazio (diretiva); template
   nulo → `null`;
7. `AnalyzedClass(element)`, `extractLifecycleHooks`, `_validateLifecycleHooks`;
8. validação do seletor, `changeDetection`, link;
9. monta `CompileDirectiveMetadata` (providers/viewProviders/exports lidos aqui).
Consequência testável: um `@HostBinding` privado dispara `BuildError` (R4.2) mesmo
se a anotação da classe tem erro de avaliação.
- *Porte*: sem equivalente de ordem de erros; **Parcial** (irrelevante para saída
  bem-sucedida).

---

## 2. Argumentos de `@Component`/`@Directive`

Campos do runtime (`RT/directives.dart:39-104`): `Directive{selector (required),
providers=[], exportAs, visibility=local}`; `Component` estende e acrescenta
`changeDetection=checkAlways, viewProviders=[], exports=[], templateUrl, template,
preserveWhitespace=false, styleUrls=[], styles=[], directives=[],
directiveTypes=[], pipes=[], encapsulation=emulated`.

### R2.1 `selector`
`coerceString(annotation,'selector')`; `null` ou `''` → `handle(ErrorMessageForAnnotation
('Selector is required, got "$selector"'))`, **mas o metadado continua a ser
montado** com `selector: null/''` (FC:661-667, 689). O seletor não é analisado aqui.
- *Porte*: `RS/metadados.rs:argumentos` (302-305): não-texto → `fora`; vazio não é
  verificado. `RS/componente.rs:ler` (590) guarda texto literal. **Parcial**.

### R2.2 `exportAs`
`coerceString(annotation,'exportAs')` (FC:690), nulo permitido.
- *Porte*: `RS/metadados.rs:argumentos` 306-310 **Coberto**. Em `RS/componente.rs`,
  `exportAs` **não** está em `ARGUMENTOS_CONHECIDOS` (componente.rs:776-789) →
  componente com `exportAs:` é **Recusa** (`o_que_nao_entendemos`, 824-889).

### R2.3 `visibility`
`coerceEnum(annotation,'visibility', Visibility.values, defaultTo: Visibility.local)`
(FC:705-710; valores `local`, `all` em `RT/visibility.dart:98,125`).
Obs.: o padrão do construtor de `CompileDirectiveMetadata` é `all` (CM:486), mas o
leitor sempre passa o valor lido/`local`.
- *Porte*: `RS/metadados.rs:argumentos` 311-316: só a forma `Visibility.x`
  resolvida (`Valor::Membro`); `d.visivel = nome == "all"`, padrão `false` (local).
  Constante que aponta para o enum → `fora`. **Parcial**.

### R2.4 `changeDetection`
`coerceEnum(.., ChangeDetectionStrategy.values, defaultTo: checkAlways)`
(FC:814-822). Valores do ngdart 8: `checkAlways`, `onPush`
(`RT/change_detection_constants.dart:22-47`). Lido também para `@Directive` (o
campo via `getField` não existe → padrão).
- *Porte*: `RS/componente.rs:e_on_push` (403-408): textual, último segmento após
  `.` == `onPush`. Não resolve constantes nem valida o tipo. **Parcial**.

### R2.5 `encapsulation`
Só em componente, dentro de `_createTemplateMetadata`: `coerceEnum(..,
ViewEncapsulation.values, defaultTo: emulated)` (FC:771, 807-812); valores
`emulated`, `none` (`RT/view.dart`).
- *Porte*: `RS/componente.rs:valor_de_encapsulamento` (413-425) aceita a forma
  textual `[prefixo.]ViewEncapsulation.emulated|none`; outra forma → **Recusa**
  (`Motivo::Encapsulamento`, 858-866). **Parcial**.

### R2.6 `changeDetectionLink`
Se há `linkInfo` e **não** (componente **e** `onPush`) → erro `'Only supported on
components that use "ChangeDetectionStrategy.onPush" change detection'`
(FC:673-681); `isChangeDetectionLink = linkInfo != null` (FC:711).
- *Porte*: **Falta** (a anotação nem é lida; em `RS/componente.rs` não aparece em
  argumentos, então o componente segue gerado sem o flag — **Divergente** se o
  flag muda a visão).

### R2.7 Template (só componente; FC:734-805)
- `template` e `templateUrl` ambos → erro `'Cannot supply both "template" and
  "templateUrl" for an @Component'` e o componente inteiro é descartado (`null`,
  FC:742-746, 655).
- `templateUrl` que `Uri.parse` rejeita → erro e `null` (FC:748-758).
- `styleUrls`: cada item cuja extensão não termina em `.css` → erro (não aborta)
  (FC:759-769).
- `styles`, `styleUrls` por `coerceStringList` (R0.2); `preserveWhitespace` por
  `coerceBool(.., defaultTo: false)` (FC:775-781).
- `templateOffset` (FC:785-805): argumento nomeado `template` do AST;
  `SingleStringLiteral` → `contentsOffset` (início do conteúdo, depois das aspas);
  `AdjacentStrings` → `offset` da expressão (**posição da primeira aspa, não do
  conteúdo**); qualquer outra forma → `0`.
- *Porte*: `RS/componente.rs:ler` (596-612) lê `template`, `templateUrl`,
  `styleUrls`/`styles` (não literal → `estilos_ilegiveis`, recusa na visão).
  `deslocamento_do_template` (308-361) só para literal simples sem escape/interp.;
  demais → `None`. `preserveWhitespace:` não está em `ARGUMENTOS_CONHECIDOS` →
  **Recusa**. Conflito `template`+`templateUrl` e extensão `.css`: **Falta**
  (diagnóstico). `AdjacentStrings` → offset da aspa: **Falta**.

### R2.8 `exports` (FC:839-911)
Lido do **AST** da anotação, não da constante:
1. Sempre inclui primeiro a própria classe: `CompileIdentifierMetadata(name,
   moduleUrl(library), analyzedClass)`;
2. se `exports:` falta ou não é `ListLiteral` → só o item 1;
3. algum elemento que não é `Identifier` → erro `'Item $x in the "exports" field
   must be an identifier'` e retorna a lista parcial;
4. `PrefixedIdentifier` só com prefixo de biblioteca (`PrefixElement`), senão erro;
   `name = identifier`, `prefix = prefixo`;
5. `staticElement` nulo → acumula e no fim `UnresolvedExpressionError`; classe →
   `analyzedClass`; `moduleUrl(staticElement.library)`.
- *Porte*: `RS/componente.rs:ler` (607, 625-633) guarda os nomes escritos
  (`nomes_da_lista`) e os tira do mapa de membros. Sem o item implícito e sem
  resolução. **Parcial**.

### R2.9 `directives`, `pipes` (FC:98-121, 130-189)
`_getResolvedArgumentsOrFail`: anotação `@Component` com erro → `handle` e `[]`;
`coerceList`; se vazio, inspeciona o AST: argumento não-`ListLiteral` →
`UnresolvedExpressionError`; lista com elemento de `staticType` dinâmico/nulo →
`UnresolvedExpressionError`. Os valores passam por `visitAll` (R0.4, listas
constantes aninhadas achatadas) e `typeDeclarationOf(value)?.accept(visitor)`
(DOU:86-93): item que não é diretiva/pipe some em silêncio.
- *Porte*: `RS/componente.rs:nomes_da_lista` (378-399) guarda nomes; expansão de
  listas constantes pelo índice (`RS/lib.rs:expandir`, 915). **Parcial** (sem os
  diagnósticos).

### R2.10 `directiveTypes` (FC:105-113, 191-203, 915-935; TR:46-153)
Cada item é `Typed<T>`/`Typed<T>.of([...])` lido por `TypedReader`:
- raiz precisa ser `Typed` exato; `T` genérico com argumentos, anotado com
  `@Directive`/`@Component`; `on:` só na raiz; `typeArguments` de `Symbol` (deve
  ser parâmetro de tipo da classe hospedeira → `TypeLink(sym, null)`), `Type`
  (`linkTypeOf`) ou `Typed` aninhado; argumento privado → erro (TR:128);
- cada `CompileTypedMetadata(symbol, import, [fromTypeLink(g)], on)`;
- `_errorOnUnusedDirectiveTypes`: todo `directiveTypes` deve casar (`moduleUrl#name`)
  com um item de `directives`, senão `UnusedDirectiveTypeError`.
- *Porte*: **Recusa** (argumento desconhecido em `o_que_nao_entendemos`).

### R2.11 `providers` / `viewProviders`
Ver §8. `viewProviders` usa o mesmo código com o campo `'viewProviders'` (FC:699-700).

---

## 3. `@Input` / `@Output` e herança

### R3.1 Coleta por membro (`_visitClassMember`, FC:285-405)
Chamada por `visitFieldElement` (com `isGetter = getter != null`, `isSetter =
setter != null`) e `visitPropertyAccessorElement` (`isGetter/isSetter` do acessor)
(FC:262-283). Só age com `_directiveClassElement` definido. Para cada anotação do
membro, na ordem, testa **nesta precedência** `Input`, `Output`,
`ContentChild|ContentChildren`, `ViewChild|ViewChildren` (tipos exatos, AI:28-34).

### R3.2 `@Input` (FC:300-347)
- Exige `isSetter && element.isPublic` (campo não-final/não-const, ou setter
  explícito). Senão `log.severe('@Input can only be used on a public setter or
  non-final field, …')` e ignora (não é erro).
- Campo `late final` → `reportAndRecover('Inputs cannot be "late final".…')`.
- `_setterFor` (FC:441-456): `thisType.lookUpSetter2(displayName, library)!`;
  setter sem parâmetro → erro `'Invalid setter…'` e ignora o membro.
- Nome da ligação: `coerceString(annotation,'bindingPropertyName', defaultTo:
  displayName)` (FC:586-589; `RT Input([bindingPropertyName])`).
- Destino: `_fieldInputs` se o elemento é `FieldElement`, senão `_setterInputs`;
  chave = `displayName` (nome do membro), valor = nome da ligação; checagem de
  mudança contra `_inputs` (R3.6).
- `inputTypes[displayName]` (FC:314-343): tipo do parâmetro do setter com
  `resolveToBound(dynamic)`; `getTypeName` (COM:23-41; função → `null` → sem
  entrada); nome primitivo (`bool,int,num,double,String`, `property_binder.dart:437-447`)
  → `CompileTypeMetadata(name)`; senão `CompileTypeMetadata(moduleUrl:
  moduleUrl(element) /* do MEMBRO, não do tipo */, name, typeArguments:
  fromDartType(args do alias ou da interface))`.

Exemplo: `@Input('v') set valor(int x)` → `inputs{'valor':'v'}`,
`inputTypes{'valor': int}`.
- *Porte*: `RS/metadados.rs:membros` (795-848) — campos (não final/const/static,
  públicos) e setters de instância públicos; apelido pelo 1º argumento posicional
  texto (`primeiro_texto`, 1049). `inputTypes` só como `booleana`
  (`booleana`, 1061-1077). `late final`: **Falta** (diagnóstico). `RS/componente.rs:
  entradas_da_classe` (1354-1399) só a própria classe. **Parcial**.

### R3.3 `@Output` (FC:348-354)
Exige `isGetter && isPublic` (todo campo tem getter, inclusive `final`); senão
`log.severe`. `_addPropertyBindingTo(_outputs, …)` (checagem contra o próprio
`_outputs`). Sem tipo registrado.
- *Porte*: `RS/metadados.rs:membros` 849-870, 969-974 **Coberto**;
  `RS/componente.rs:saidas_da_classe` 1401-1443 (só a classe).

### R3.4 Herança e ordem (`_collectInheritableMetadata`, FC:596-621)
```
for type in element.allSupertypes.reversed: collectOn(type.element)
collectOn(element)
collectOn(e): se e é dart:core Object → return
              e.visitChildren(this)            // dispara R3.1 por membro
              _inputs.addAll(_fieldInputs).addAll(_setterInputs); limpa os dois
```
- `allSupertypes` (analyzer `class_hierarchy.dart:41-103`): `append(supertype)`,
  (restrições `on` de mixin), cada interface, cada mixin — onde `append(t)` põe `t`
  e depois os `allSupertypes` de `t` —, deduplicado por elemento (primeiro vence).
  Invertido: os mixins/interfaces declarados por último vêm primeiro; a superclasse
  direta vem por último antes da própria classe.
- `visitChildren` de `InterfaceElementImpl` (analyzer `element.dart:5108-5115`):
  **acessores, campos, construtores, métodos, parâmetros de tipo**. Logo, numa
  classe, setters/getters explícitos são vistos antes dos campos.
- Mapas Dart são `LinkedHashMap`: reatribuir chave existente troca o valor e
  **mantém a posição**.

Ordem resultante (testável):
- `inputs`: chaves na ordem da primeira aparição percorrendo a hierarquia de cima
  para baixo; dentro de uma classe, **entradas de campo antes das de setter**
  (apesar dos setters serem visitados antes); valor = o da classe mais derivada.
- `outputs`: primeira aparição; dentro da classe, getters explícitos antes dos
  campos.
- `queries`/`viewQueries` (listas): acumulam na hierarquia inteira, supertipos
  primeiro; dentro da classe, setters antes de campos.

Exemplo:
```dart
class B { @Input() set b(v){}  @Input() String? a; }
@Directive(selector:'x') class D extends B { @Input('z') String? b; }
```
`inputs` = `{a: a, b: z}` (B: campo `a` antes do setter `b`; D sobrescreve `b`).
- *Porte*: `RS/metadados.rs:supertipos` (162-186) reproduz `allSupertypes`;
  `diretiva` (259-277) inverte e anexa a classe; pula bibliotecas SDK (o oficial só
  pula `Object`); `membros` (771-989) separa campo/setter e aplica "chave mantém
  posição" (961-968). **Coberto** para entradas/saídas/`@HostBinding`.
  **Divergente** para `@ContentChild`: `d.consultas_de_conteudo = consultas_setter`
  (metadados.rs:959) **sobrescreve** a lista a cada classe da hierarquia — as
  consultas herdadas se perdem (o oficial acumula). `RS/componente.rs`: só a
  própria classe (`herda` marca o caso).

### R3.5 Membros sintéticos
O porte assume que os acessores sintéticos de um campo não carregam anotações
(o campo é visto uma vez, como `FieldElement`). Não verificado contra a versão exata
do analyzer usada no build oficial.

### R3.6 Mudança de nome herdado (`_prohibitBindingChange`, FC:937-949)
Se a chave já existe no mapa de controle com outro nome → `log.severe("'X'
overwrites the binding name of property 'p' from 'a' to 'b'.")`; **a atribuição
acontece mesmo assim** (FC:590-592).
- *Porte*: sobrescreve sem log. **Coberto** (efeito), **Falta** (log).

---

## 4. `@HostBinding` / `@HostListener`

### R4.1 Visita (`DirectiveVisitor.visitDirective`, DIR:67-99)
Mesma ordem de hierarquia de R3.4 (`allSupertypes.reversed`, depois a classe;
aqui **sem** pular `Object`). Por classe: `accessors`, depois `methods`, depois
`fields` (DIR:74-84). Por membro: todos os `@HostBinding` exatos, depois todos os
`@HostListener` exatos (`annotationsOfExact`, `throwOnUnresolved: false`).

### R4.2 `@HostBinding` (DIR:101-104; FC:530-557)
- Membro não público → `BuildError('@HostBinding must be on a public member')`
  (lançado; aborta).
- Chave = `coerceString(value,'hostPropertyName', defaultTo: element.name)`;
  valor = `PropertyRead(ImplicitReceiver, name)`.
- Membro estático (acessor ou campo `isStatic`): se declarado fora da própria
  classe da diretiva → **ignorado** (não herda); senão valor =
  `PropertyRead(StaticRead(<classe>), name)`.
- Mapa `_hostBindings`: último vence, na posição do primeiro.
- Derivados (CM:545-600): `hostAttributes` = ligações com valor imutável
  (`isImmutable`, AC:115-152: literal, `StaticRead`, ou `PropertyRead` de campo
  não sintético `final`/`const` ou de método) **e** estático (literal ou receptor
  `StaticRead`) **e** nome sem prefixo `style.`/`class.`; `attr.` é removido do
  nome. Resto → `hostProperties`. `requiresDirectiveChangeDetector` = diretiva
  (não componente) com `hostProperties` não vazio.

Exemplo: `@HostBinding('attr.role') static const role = 'button';` → atributo
estático `role` escrito uma vez; `@HostBinding('class.on') bool on = false;` →
propriedade.
- *Porte*: `RS/metadados.rs:membros` 883-924 — ordem acessores/métodos/campos ✓,
  estático herdado ignorado ✓, estático próprio → `hospedeiro_estatico` (pendência
  em diretiva, `RS/diretivas.rs:pendencia` 292-340); só a **primeira** variável de
  uma declaração múltipla; membro privado não é rejeitado. `RS/componente.rs:
  ligacoes_do_hospedeiro` (934-999): só a própria classe, marca `imutavel`/
  `estatico`. Classificação atributo/propriedade: feita na visão (`RS/visao.rs`,
  fora desta seção). **Parcial**.

### R4.3 `@HostListener` (DIR:106-121; FC:559-568)
- Membro estático ou não `ClassMemberElement` → `BuildError('@HostListener must be
  on a non-static member')`; não público → `BuildError('… public member')`.
- Se `args` é lista: o método precisa ter exatamente `len(args)` parâmetros
  **posicionais obrigatórios**, senão `BuildError('@HostListener expected a method
  with N parameter(s)')` (DIR:53-58).
- `member as MethodElement` (em getter/campo o cast falha).
- Handler = `'$methodName(${args.join(', ')})'`; se `args` vazio **e**
  `parameters.length == 1` (todos os parâmetros, inclusive opcionais/nomeados) →
  `args = ['\$event']`.
- Mapa `_hostListeners` chave `eventName`: último vence, posição do primeiro.

Exemplo: `@HostListener('click') void f(e) {}` → `{'click': 'f($event)'}`;
`@HostListener('keyup', ['\$event.key']) void g(String k)` → `'g($event.key)'`.
- *Porte*: `RS/metadados.rs:ouvinte` (1080-1133) monta o texto ✓, mapa ✓ (873-880);
  **Falta** a checagem de aridade (DIR:53-58) e a de visibilidade.
  `RS/componente.rs:ouvinte` (1033-1082): exige evento nativo, recusa evento
  duplicado. **Parcial**.

---

## 5. Consultas (`@ViewChild(ren)`, `@ContentChild(ren)`)

### R5.1 Anotações do runtime (`RT/directives.dart:300-405, 580-626`)
`_Query(selector, {descendants=false, first=false, read})`:
- `ContentChildren(sel, {descendants = true, read})`;
- `ContentChild(sel, {read})` → `descendants: true, first: true`;
- `ViewChildren(sel, {read})` → `descendants: true`;
- `ViewChild(sel, {read})` → `descendants: true, first: true`.
**Não existe campo `static`** no ngdart 8; se a consulta é estática é decidido pelo
compilador de visão (fora desta seção).

### R5.2 Coleta (FC:355-402)
Exige `isSetter && isPublic` (campo não-final ou setter), senão `log.severe`.
`queryType` = tipo do parâmetro do setter (`_setterFor`). Com `first` e tipo
explicitamente não-nulo → `reportAndRecover('ViewChild and ContentChild queries must
be nullable.…')`; campo `late` → `reportAndRecover('View and content queries cannot
be "late".…')`. Content → `_queries`; View → `_viewQueries`. Ordem: R3.4.

### R5.3 `CompileQueryMetadata` (`_getQuery`, FC:496-528; `_getSelectors` 458-491)
- `selectors`: campo `selector` nulo → erro `'Missing selector argument for
  "@X"'` e `[]`; `String` → `split(',')` **sem trim**, cada parte um
  `CompileTokenMetadata(value: s)`; `Type` → `CompileTokenMetadata(identifier:
  (name, moduleUrl(element)))`; outro → erro, `[]`.
- `descendants`, `first`: `coerceBool(.., false)` (R0.1: vêm do `super`).
- `propertyName` = `displayName` do membro (sem `=`).
- `isElementType` = (tipo tem elemento e `dart:html#Element` é atribuível dele) **ou**
  (`dart:core#Iterable` atribuível, tipo parametrizado, e `Element` atribuível do
  primeiro argumento).
- `read`: `getField('read')?.toTypeValue()`; tipo → token identificador; qualquer
  valor não-tipo é **ignorado em silêncio** (`read = null`).

Exemplo: `@ViewChild('a, b') Element? x;` → seletores `['a', ' b']`, `first`,
`descendants`, `isElementType = true`.
- *Porte*: ViewChild — `RS/componente.rs:consulta_simples` (1084-1209): só a
  própria classe; seletor texto único (vírgula → **Recusa**) ou identificador
  simples; `read:` só identificador; campo `static/final/const/late` recusado; não
  checa nulabilidade; `@ViewChildren` exige `List<T>`. Em `RS/metadados.rs` só vira
  `d.consultas = true` (pendência). `isElementType`: `RS/visao.rs:e_tipo_de_elemento`
  (2103-2112) aproxima por "nome termina em `Element` e vem de `dart:html`".
  ContentChild — `RS/metadados.rs:consulta_de_conteudo` (994-1046): alvo texto ou
  tipo de pacote do usuário (tipos do ngdart/dart recusados), `descendants:` só em
  lista, `read:` só `Element/HtmlElement` ou classe do usuário; **não** divide por
  vírgula; perde os herdados (R3.4). **Parcial**.

---

## 6. Ganchos de ciclo de vida

### R6.1 Detecção (LH:5-21; tipos em TYP:52-62)
`extractLifecycleHooks(cls)` devolve, **nesta ordem fixa**, os ganchos cujo
`TypeChecker.isAssignableFrom(cls)` é verdadeiro: `onInit, onDestroy, doCheck,
afterChanges, afterContentInit, afterContentChecked, afterViewInit,
afterViewChecked` (interfaces `package:ngdart/src/meta/lifecycle_hooks.dart`).
Detecção só por **tipo** (herdado por `extends`/`implements`/`with` conta); ter o
método `ngOnInit` sem a interface não conta.

### R6.2 `ngDoCheck` assíncrono (FC:715-732)
Com `doCheck`: `getMethod('ngDoCheck') ?? lookUpInheritedMethod(...)`; se
`isAsynchronous` → `reportAndRecover('ngDoCheck should not be "async"…')`.
- *Porte*: `RS/metadados.rs:ganchos` (1136-1156) sobre a hierarquia resolvida —
  **Coberto** (flags; a ordem é irrelevante na struct `Ganchos`).
  `RS/componente.rs:ganchos_da_classe` (797-818): só nomes escritos em
  `implements`/`with` da própria classe; quando a classe tem `extends`/`with`
  (`herda`), `RS/visao.rs` (11011-11027) e `RS/lib.rs` (769) trocam pelos
  `metadados::ganchos` (sem eles, recusa). Resta perder a interface indireta
  (`implements MinhaInterface` que estende `OnInit`) — **Parcial**. R6.2: **Falta**.

---

## 7. Pipes (`PipeVisitor`, PV:23-84)

- Anotação: primeira que casa `isPipe` (exato) (`annotationWhere`, AI:59-79); erro
  de avaliação → `handle`, `null`; classe privada → `reportAndRecover('Pipes must
  be public')`, `null`.
- `transform`: `thisType.lookUpMethod2('transform')` (herdado vale) → tipo do
  método; senão getter `transform` cujo retorno é `FunctionType`; senão erro
  `'Pipes must implement a "transform" method'`, `null`.
- `CompilePipeMetadata(type: CompileTypeMetadataVisitor (§8.1, com deps),
  transformType: fromFunctionType, name: coerceString('name'), pure:
  coerceBool('pure', true), lifecycleHooks)`. Runtime: `Pipe(this.name, {pure=true})`.
- *Porte*: `RS/componente.rs:ler_pipe` (478-560): nome posicional/`name:`, `pure:`
  literal; `transform` herdado ou getter → **Recusa**; construtor só vazio ou
  `ChangeDetectorRef` → demais **Recusa**; `OnDestroy` só por `implements` escrito.
  **Parcial**.

---

## 8. Tipo da diretiva e provedores de componente (caminho `CompileTypeMetadataVisitor`)

### R8.1 `CompileTypeMetadata` da classe (TCM:37-80, 245-269)
- Classe privada → `throw BuildError('Provided classes must be public: …')`.
- `typeParameters`: `o.TypeParameter(name, bound: fromDartType(bound,
  resolveBounds: false))`.
- `diDeps` (com `enforceClassCanBeCreated`): `unnamedConstructor` —
  construtores vazios → `throw 'No constructors found'`; escolhe o sem nome, senão
  o **primeiro**; se privado → `throw 'No constructors found'`; classe abstrata com
  construtor não-`factory` → `logWarning` e `diDeps = []`; mais de um construtor →
  só `logFine`.
- *Porte*: `RS/metadados.rs:dependencias` (1160-1197): abstrata → sem deps
  (**Divergente** se o sem-nome for `factory`: o oficial lê os parâmetros dele);
  construtor privado → `fora`; classe privada não verificada.

### R8.2 Dependências do construtor (TCM:295-345, 679-748)
Parâmetros **nomeados são pulados** (TCM:300-303). Para cada posicional,
`ParameterInfo` lê cada anotação: erro de avaliação → `handle`; valor nulo →
`reportAndRecover('Error evaluating annotation')`; senão classifica (TCM:728-747,
primeiro teste que casar):
`Attribute` exato → `attribute`; `Self`; `Host`; `SkipSelf`; `Optional`; valor
**atribuível a `OpaqueToken`** (o token usado como anotação, ex. `@ngValidators`)
→ `opaqueToken`; `Inject` exato → `injectValue`.
- `isOptional = @Optional || parâmetro posicional opcional ([x])` (TCM:315, 326).
- Não-atributo: `_checkForOptionalAndNullable` (TCM:136-158): tipo explicitamente
  não-nulo **e** opcional → `throw`; explicitamente nulo **e** não opcional →
  `throw` (`messages.optionalDependenciesNullable`).
- Token (TCM:339-359): `@Attribute` → `CompileTokenMetadata(value:
  attributeName)` e `isAttribute`; `@Inject` → `_token(inject.token,
  annotation)`; token-anotação → `_token(valor)`; senão `_tokenForType(p.type)`.
- `ArgumentError` → `logWarning('Could not resolve token…')` e
  `CompileDiDependencyMetadata()` vazio (TCM:328-336).

Exemplo: `X(@Optional() Foo? f, [Bar? b], {Baz? z})` → `[f: Foo optional, b: Bar
optional]` (z pulado).
- *Porte*: `RS/metadados.rs:dependencia` (1199-1287): `@Optional/@Self/@Host/
  @SkipSelf/@Inject/@Attribute` ✓; token-anotação só identificador simples
  (prefixado → erro); `HtmlElement/Element` → `Token::Elemento`,
  `ChangeDetectorRef` → `Token::Detector`; `this.x`/`super.x` sem tipo
  (`tipo_do_campo` 1352, `tipo_do_super` 1296) ✓; `dynamic` → erro; outra anotação
  do `di_arguments` → erro. Checagem de nulabilidade: **Falta**. **Parcial**.

### R8.3 `_token(DartObject, [annotation])` (TCM:380-426)
Em ordem:
1. `null`: sem anotação → `logWarning`, `value: 'OpaqueToken__NOT_RESOLVED'`; com
   anotação → `_annotationToToken` (1º argumento do AST: identificador simples ou
   prefixado → `identifier(name, moduleUrl(lib))`, senão `throw 'Could not read
   token'`) (TCM:361-378);
2. atribuível a `OpaqueToken` → `_canonicalOpaqueToken` (R8.4);
3. `String` / `bool` / `int` / `double` → `CompileTokenMetadata(value: v)`;
4. tipo → `_tokenForType` (`identifier = _idFor(type)`: nome via `getTypeName`,
   `moduleUrl` do alias ou do elemento; tipo função sem typedef → `throw 'A function
   type: … Please add typedef'`, TCM:456-489);
5. instância de classe (`InterfaceType`): com argumentos de construtor →
   `logWarning` e `'OpaqueToken__NOT_RESOLVED'`; sem → `_tokenForType(type,
   isInstance: invocation != null)`;
6. função → `identifier = _identifierForFunction` (prefixo = classe se estática,
   `emitPrefix: true`, TCM:563-574);
7. senão `ArgumentError('@Inject is not yet supported for $token.')`.
- *Porte*: `RS/metadados.rs:token_lido` (716-767): só tipo, `OpaqueToken`/`MultiToken`
  **exatos** (sem nome: erro fora do injetor). Tokens texto/número/bool, instância
  de classe, função, subclasses de `OpaqueToken`: **Falta**.

### R8.4 `OpaqueToken`/`MultiToken` canônico (TCM:428-454; TOK:20-137)
- `TokenReader.parseTokenObject`: nulo → erro; tipo → `TypeTokenElement(linkTypeOf)`;
  instância de `OpaqueToken` → `_parseOpaqueToken`; outro → `BuildError('Not a
  valid token for injection…')`.
- `_parseOpaqueToken`: `typeArgs` = argumentos do próprio tipo se é
  `OpaqueToken`/`MultiToken` exato; se subclasse, os do **supertipo direto**;
  `identifier = _uniqueName` (`RT/di_tokens.dart`: `OpaqueToken([_uniqueName = ''])`);
  `isMultiToken`; `classUrl = linkToOpaqueToken(tipo)`; `typeUrl =
  linkTypeOf(typeArgs.first)` ou `null`.
- `linkToOpaqueToken` (TOK:84-137) exige de subclasse: não implementa/mistura
  nada; não abstrata nem privada; exatamente um construtor, sem nome, `const`, sem
  parâmetros, sem parâmetros de tipo; supertipo direto `OpaqueToken` ou
  `MultiToken`. Violação → `BuildError` com a mensagem citada.
- No caminho do componente: `CompileTokenMetadata(value: identifier ou null,
  identifier: CompileIdentifierMetadata(name: classUrl.symbol, moduleUrl,
  typeArguments: [fromTypeLink(typeUrl)] só se a classe é `OpaqueToken`/`MultiToken`
  embutida e `typeUrl != null`), identifierIsInstance: true)`.
- O `T` de `MultiToken<T>` é o argumento do `MultiToken` (não o `List<T>` do
  supertipo).
- `CompileTokenMetadata.name` = `value` sanitizado (`\W` → `_`) ou
  `identifier.name` (CM:209-215); unicidade por `assetCacheKey` (`name|moduleUrl|
  identifierIsInstance|value|typeArgs`, CM:178-196); registro duplicado num
  `CompileTokenMap` → `BuildError('Failed to register provider for token…')`
  (CM:238-263).

Exemplo: `const t = OpaqueToken<String>('url');` → `value:'url'`,
`identifier: OpaqueToken<String>`, `name: 'url'`.
- *Porte*: `RS/metadados.rs:token_lido` (716-767) + `tipo_de_token` (484-510):
  exatos ✓; sem argumento de tipo → `Object` ✓; `<dynamic>` → erro; subclasses →
  **Falta**. `Token::nome` (`RS/diretivas.rs:80-98`) = sanitização ✓.

### R8.5 Provedores de componente (FC:824-837; TCM:82-129)
1. `ModuleReader.extractProviderObjects(getField(annotation, campo))` (MOD:47-77):
   nulo → `[]`; lista → achata recursivamente; `Module` exato → `include` (cada um
   recursivamente) **antes** de `provide`; outro → `[valor]`.
2. `visitAll(..., createProviderMetadata)` descartando nulos.
3. `createProviderMetadata`:
   - literal de tipo: não-interface → `logWarning`, `null`; senão
     `visitClassElement` (R8.1, privada lança) → `token = identifier(tipo)`,
     `useClass = mesmo tipo`;
   - senão `token` nulo → `reportAndRecover("A provider's token field failed to
     compile.")`, `null`;
   - `useClass` (TCM:174-200): `useClass:` explícito → classe (não-interface →
     erro, `null`); senão, se `useValue` é o sentinela `'__noValueProvided__'`
     **e** `useFactory`/`useExisting` nulos, o **tipo do token** (se é tipo e não
     `Null`);
   - `useExisting` → `_token(useExisting)`;
   - `useFactory` (TCM:215-238, 576-593): função → `CompileFactoryMetadata(name,
     moduleUrl, prefix=classe se método estático, emitPrefix, diDeps: deps: não
     vazio ? _factoryDiDep : parâmetros da função (R8.2))`;
   - `useValue` (TCM:271-293): sentinela → `null`; senão `_useValueExpression`
     (R8.6) — **`useValue: null` explícito vira `o.nullExpr`**;
   - `multi = $MultiToken.isAssignableFromType(token.type)` (inclui subclasses);
   - `typeArgument = inferProviderType` (R8.7);
   - serviço global (R10.3) → `reportAndRecover(removeGlobalSingletonService)`.
   O metadado carrega **todos** os `use*` lidos; a escolha é feita depois.
4. `_factoryDiDep` (TCM:596-635): tipo ou `OpaqueToken` → `dep(token)`; lista →
   `token = _token(lista[0])` e flags pelos tipos **exatos** `Self/Host/SkipSelf/
   Optional` nos demais; outro → `BuildError('Could not resolve dependency …')`.
- *Porte*: `RS/metadados.rs:provedores/provedor` (332-479): listas ✓; `Module` →
  **Falta**; um `use*` só (mais de um → erro); `useValue: null` → erro;
  `deps:` só com `useFactory`; `fonte_de_classe` (540-560) recusa classe genérica e
  abstrata; `fonte_de_fabrica` (564-612) só função de topo (método estático →
  erro); `dependencia_de_deps` (616-648) ✓; serviço global no componente: **Falta**.
  `viewProviders` não vazio → `fora` (321-324); em `RS/componente.rs` →
  **Recusa**. `providers` de `@Directive` do próprio arquivo → **Recusa**
  (componente.rs:854-857). **Parcial**.

### R8.6 `_useValueExpression` (TCM:491-525)
Ordem: nulo → `null`; `String`/`bool`/`int`/`double` → literal tipado; lista →
`const [..]` recursivo; mapa → `const {..}` recursivo; tipo → referência ao tipo;
enum (`isDartCoreEnum`) → `Tipo.campo` do campo de enum igual; enum protobuf
(supertipo exato `ProtobufEnum`) → `Tipo.<name>`; instância → `_expressionForType`
(TCM:536-561: sem invocação → só a referência; com → `const Tipo[.ctor](pos...,
nome: v...)`; construtor nomeado privado → erro sugerindo `useFactory`); função →
referência; senão `ArgumentError`.
- *Porte*: `RS/metadados.rs:valor_const` (671-705): texto, inteiro, booleano, objeto
  de classe não genérica **com um nível** de argumentos desses. Lista, mapa,
  `double`, tipo, enum, protobuf, função: **Falta**.

### R8.7 `inferProviderType` (PI:9-73)
1. token atribuível a `MultiToken`: exato → seu `T`; subclasse cujo supertipo
   direto não é `MultiToken` exato → `BuildError`; senão `T` do supertipo;
2. `T` do `Provider<T>` do objeto, se não `dynamic` nem `Object`;
3. token `OpaqueToken<T>` com `T` não-dynamic, e o provedor **não** é `Provider`
   exato → `T`;
4. `null`.
- *Porte*: `RS/metadados.rs:provedor` 437-472: 1 ✓ (só exato), 2 ✓ (`Object` →
  `None`; `dynamic` → erro), 3 ✓ (`forToken`), e um caso extra que deduz `T` do
  valor em `ValueProvider(Tipo, v)` (a inferência Dart faz isso no `T`). **Parcial**.

---

## 9. Dependências no `@GenerateInjector` (caminho `DependencyReader`)

Regras **diferentes** das de R8.2 (TCM e DEP divergem no próprio oficial).

### R9.1 Escolha do construtor (`findConstructor`, DEP:35-44)
Sem-nome se existe **e** a classe não é abstrata; senão o primeiro com
`(isPublic && !abstrata) || isFactory`; nenhum → `BuildError('Could not find a valid
constructor')` (DEP:162-171).
- *Porte*: `RS/metadados.rs:classe_do_injetor` (2085-2124) ✓.

### R9.2 Parâmetros (`_parseDependencies`, DEP:96-131)
Para cada parâmetro:
- `isRequired = sem @Optional exato`; `hasInjectToken = @Inject exato`;
  `hasOpaqueToken = alguma anotação cujo valor é OpaqueToken`;
- **pula** nomeado opcional; **pula** posicional opcional sem `@Optional` e sem
  token; **inclui nomeado obrigatório como posicional**;
- token (`parseTokenParameter`, TOK:142-149): `@Inject(t)` → `parseTokenObject(t)`,
  senão a primeira anotação `OpaqueToken`, senão o tipo do parâmetro
  (`TypeTokenElement(linkTypeOf)`);
- `_checkForOptionalAndNullable(isOptional: !isRequired)` (DEP:138-160);
- `DependencyElement(token, type: tipo do parâmetro se tem token explícito,
  host/self/skipSelf por anotação exata, optional: !isRequired)`.
- **Opcional aqui é só `@Optional`** (um `[x]` com `@Inject` não é opcional).
- `@Attribute` é ignorado (o token sai do tipo).
- *Porte*: `RS/metadados.rs:dependencia_do_injetor` (2129-2177): pulos ✓,
  `opcional = @Optional` ✓; nomeado obrigatório → **erro** (oficial o passa
  posicionalmente: **Divergente/Recusa**); `@Attribute`, `HtmlElement`,
  `ChangeDetectorRef` → erro (oficial gera `this.get(Tipo)`: **Recusa**);
  nulabilidade: **Falta**.

### R9.3 `deps:` explícito (`parseDependenciesList`, DEP:67-94)
Item lista → `[token, ...meta]`; flags por tipo exato em `meta`; senão o item é o
token. Token por `parseTokenObject` (texto/número são **erro** aqui, ao contrário de
R8.3).
- *Porte*: `RS/metadados.rs:fabrica_do_injetor` (2181-2231) via
  `dependencia_de_deps` ✓. Observação: `deps: const []` vazio → oficial usa
  `parseDependenciesList` com lista vazia (`manualDeps.isList`, PRV:167-170) →
  **zero dependências**; o porte trata `Some(Lista([]))` como lista vazia ✓.

---

## 10. `@GenerateInjector` — leitura (`InjectorReader`)

### R10.1 Descoberta (INJ:25-40, 64-73)
Variáveis de topo da **unidade de definição** da biblioteca com `@GenerateInjector`
exato; uma `InjectorReader` por variável, `doNotScope = uri da biblioteca`.
Runtime: `GenerateInjector(List<Object> _providersOrModules)` e
`GenerateInjector.fromModules(List<Module>)` (redirecionante, mesmo campo).
O outliner declara `external _ng.Injector <nome>$Injector(_ng.Injector parent);`
(OUT:78-83, 176-179).
- *Porte*: `RS/metadados.rs:ler_injetores` (1767-1802) ✓ (primeira unidade da
  biblioteca; todas as variáveis da declaração anotada). Biblioteca fora de `lib/` →
  **Recusa** (`RS/lib.rs` 1543-1545).

### R10.2 Provedores (`_computeProviders`, INJ:100-136)
1. `annotation.read('_providersOrModules')` nulo → `BuildError('Unable to parse
   @GenerateInjector. You may have analysis errors')`;
2. `parseModule` (MOD:94-135): lista → itens que são módulo (lista ou `Module`
   exato) vão para `include` (recursivo), os demais para `provide` via
   `parseProvider`; `Module` → `include`/`provide` precisam ser listas
   (`FormatException`); outro → `FormatException('Expected Module, got "T".')`;
3. `flatten` (MOD:154-161): recursivamente os `include` **antes** dos `provide`;
4. token `TypeTokenElement` que é serviço global → `BuildError(
   removeGlobalSingletonService)`;
5. `deduplicateProviders` (MOD:82-91): não-multi, na ordem **invertida**, primeiro
   de cada token (= **último** declarado vence e a ordem final fica invertida);
   depois todos os multi na ordem original.
6. Exceções → mensagens: `UnsupportedProvider`/`NullToken`/`FormatException` →
   `_throwParseError` ("A provider's token (…) was read as "null"…");
   `NullFactory`/`InvalidFactory` → `'Invalid provider (…): an explicit value of
   `T` was passed in where a function is expected.'`.

Exemplo: `[A, B, ValueProvider(A, a2)]` → ordem final `[ValueProvider(A), B]`.
- *Porte*: `RS/metadados.rs:injetor` (1807-1841), `modulo` (1844-1893),
  `Modulo::achatar` (1757-1762) ✓. Serviço global: o porte testa qualquer
  `package:ngdart/…` com os quatro nomes; o oficial compara `TypeLink` exato
  (GSS:3-23: `ApplicationRef@asset:ngdart/lib/src/core/application_ref.dart`,
  `AppViewUtils@…/core/linker/app_view_utils.dart`, `NgZone@…/core/zone/ng_zone.dart`,
  `Testability@…/testability/implementation.dart`) — **Coberto** na prática.

### R10.3 `parseProvider` (PRV:34-200)
- Literal de tipo → `UseClassProviderElement(TypeToken(linkTypeOf(thisType)),
  providerType: linkTypeOf(typeArgumentOf(o)), useClass: token, deps:
  parseDependencies(classe))`; elemento não-classe → `BuildError('Not a class
  element')`.
- Não atribuível a `Provider` → `FormatException('Expected Provider, got "T".')`.
- `_parseProvider` — **precedência**: `token` nulo → `NullTokenException`;
  `useClass` → classe; `useFactory` → fábrica (não-função → `InvalidFactory`);
  `useValue` nulo explícito → **valor `null`**; `useValue` ≠ sentinela → valor;
  `useExisting` → existente; token-tipo: se o provedor não é `Provider` exato →
  `NullFactoryException`, senão `useClass = tipo do token`; senão
  `UnsupportedProviderException`. (Diferente da precedência de runtime
  `_buildAtRuntime`: valor, fábrica, existente, classe — `RT/di_providers.dart`.)
- `_actualProviderType` (PRV:99-119): provedor não-`Provider`-exato com token
  opaco → `typeUrl ?? dynamic`; senão `linkTypeOf(T do Provider<T>)`.
- `isMulti` = token opaco com `isMultiToken` (PRV:221-224).
- *Porte*: `RS/metadados.rs:provedor_do_injetor` (1905-2031): precedência
  classe/fábrica/valor/existente ✓, `ClassProvider` = `useClass ?? token` ✓,
  `useValue: null` → `Revivido::Nulo` ✓, `_actualProviderType` via
  `tipo_do_provedor` ✓ (sem `T`: `Object`; `ValueProvider` sem `T`: tipo do
  valor), token por `token_lido(.., sem_nome=true)`. Provedor de classe genérica →
  erro. **Coberto** nas formas aceitas.

### R10.4 `linkTypeOf` / `linkToReference` (LNK:14-68; COM:8-41)
`void` → `TypeLink('void','dart:core')`; `Null`; `dynamic` → `TypeLink('dynamic',
null)`; parâmetro de tipo → seu limite (recursivo); `getTypeName` nulo (função sem
typedef) → `dynamic`; genéricos do alias ou da interface; `isNullable` pelo `?`;
`import = getTypeImport` (`asset:` normalizado). `linkToReference`: dinâmico ou
**privado** (`symbol` começa com `_`) → `dynamic` de `dart:core`; senão
`TypeReference(symbol, pathToUrl(import), types recursivos)` (nulabilidade **não** é
propagada).
- *Porte*: `RS/metadados.rs:tipo_escrito` (2038-2058): sem argumentos escritos →
  `dynamic` em cada parâmetro ✓; tipo privado → `dynamic`: **Falta**.

### R10.5 Referências (`_referSafe`/`_referRelative`, INJ:144-177)
URL nula → símbolo sem import; `asset:` com `doNotScope` → se o 2º segmento é
`lib` → `package:<pkg>/<resto>`; outro pacote → a `asset:` literal; mesmo pacote →
caminho **relativo**. Senão `refer(symbol, url)`.
- *Porte*: só bibliotecas em `lib/` (R10.1) — o ramo relativo não é necessário.

---

## 11. `@GenerateInjector` — emissão (`InjectorReader.accept` + `InjectorEmitter`)

### R11.1 Sequência de visita (INJ:245-313)
`visitMeta('_Injector$<var>', '<var>$Injector')`; para cada provedor com índice
`i` (a partir de 0, na ordem de R10.2):
- valor → `visitProvideValue(i, token, tokenExpr, linkToReference(providerType),
  _reviveAny(useValue), isMulti)` (`ReviveError` → `BuildError('While reviving
  providers for Injector: …')`);
- classe → `visitProvideClass(i, token, tokenExpr, _referSafe(useClass),
  nomeDoConstrutor ou null, deps, isMulti)`;
- fábrica → `visitProvideFactory(i, token, tokenExpr, linkToReference(providerType),
  _referSafe(fragmento, url sem fragmento), deps, isMulti)`;
- existente → `visitProvideExisting(i, token, tokenExpr,
  linkToReference(providerType), tokenExpr(redirect), isMulti)`.
Por fim o implícito: `visitProvideValue(n, null, Injector, Injector, this, false)`.

### R11.2 Expressão do token (`_tokenToIdentifier`, INJ:179-190)
Tipo → `refer(symbol, import)`; opaco → `linkToReference(classUrl)
.constInstance(identifier.isNotEmpty ? [literalString(identifier)] : [])`, i.e.
`const OpaqueToken<T>('nome')` / `const MultiToken<T>()` (subclasse:
`const MeuToken()`, sem genéricos).
- *Porte*: `RS/injetor.rs:Emissor::token` (140-165) ✓ para os embutidos;
  subclasse: **Falta** (não chega ao emissor, R8.4).

### R11.3 Dependências (`_computeDependencies`, INJ:192-242)
Precedência **self > skipSelf > host > optional > padrão**:

| flags | expressão |
|---|---|
| self, opcional | `injectFromSelfOptional(T, null)` |
| self | `injectFromSelf(T)` |
| skipSelf, opcional | `unsafeCast(injectFromAncestryOptional(T, null))` (`unsafeCast` de `package:ngdart/src/utilities.dart`) |
| skipSelf | `injectFromAncestry(T)` |
| host, opcional | `injectFromParentOptional(T, null)` |
| host | `injectFromParent(T)` |
| opcional | `provideUntyped(T, null)` |
| — | `this.get(T)` |
- *Porte*: `RS/injetor.rs:Emissor::dependencia` (168-209) **Coberto**.

### R11.4 Valores revividos (`_reviveAny`, INJ:316-427)
Nulo → `null`; lista → `literalConstList` recursivo; mapa → `literalConstMap`;
literal: texto → `_reviveString`, outro (`int`, `double`, `bool`) → `literal(v)`;
tipo → `ReviveError('Reviving Types is not supported…')`; senão
`ConstantReader.revive()` → `_revive`: acesso privado → `BuildError` longo
("…there was no way to access …"); `source.fragment` não vazio →
`const Classe[.accessor](pos, nomeados)` com `import = pathToUrl(source sem
fragmento)`; senão referência ao campo `accessor`.
`_reviveString` (INJ:384-414): `\` → `\\`; `$` não precedido de `\` → `\$`; `\n` →
`\n` literal; depois cada runa fora de `0x20..0x7E` → `\u{hex}` minúsculo; por fim
`literalString` (aspas simples).
- *Porte*: `RS/metadados.rs:reviver` (2235-2265), `reviver_objeto` (2271-2383)
  imita `revive()` (campo `const` público da própria classe, campo `const` de classe
  da biblioteca, construtor, constante de topo); `RS/injetor.rs:texto_revivido`
  (107-132) ✓ (teste 419-422). Mapa e `double`: **Falta**.

### R11.5 Forma do código (EMI:10-267; CB:26-47)
- Arquivo: `import '<fonte>';` [e `export`], depois os imports do injetor (emitidos
  à parte pelo `SplitDartEmitter`, `Allocator.simplePrefixing` → `_i1`, `_i2`… na
  ordem de primeira referência; `dart:core` sem prefixo), depois o código das
  visões, depois o corpo dos injetores; por injetor: **a função fábrica e depois a
  classe**.
- Fábrica: `Injector <var>$Injector(Injector parent) => _Injector$<var>._(parent);`
- Classe `_Injector$<var> extends HierarchicalInjector implements Injector`, membros
  na ordem de emissão do `code_builder` (construtores, campos, métodos —
  `code_builder/lib/src/emitter.dart` visitClass):
  - `_Injector$<var>._(Injector parent) : super(parent);`
  - campos (só classe e fábrica): `T? _field<i>;` — classe: `T` = `useClass` sem
    genéricos; fábrica: `T` = `providerType` com genéricos;
  - métodos, em ordem de índice:
    - classe: `T _get<Classe>$<i>() => _field<i> ??= T[.ctor](deps);`
    - fábrica: `R _get<R.symbol>$<i>() => _field<i> ??= f(deps);`
    - existente: `R _getExisting$<i>() => this.get(<redirect>);` (sem cache)
    - valor: `R _get<R.symbol>$<i>() => <valor>;` (sem cache, reavaliado)
    - implícito: `Injector _getInjector$<n>() => this;`
  - `@override Object? injectFromSelfOptional(Object token, [Object? orElse =
    throwIfNotFound]) { … return orElse; }` cujo corpo é: um
    `if (identical(token, <tokenExpr>)) { return <método>(); }` por provedor
    **não-multi** na ordem, **incluindo o `Injector` implícito por último**; depois,
    por token multi (ordem de primeira aparição; chave = igualdade de
    `TokenElement`), `if (identical(token, <tokenExpr>)) { return [m1(), m2()]; }`.
- Nome de método com `providerType` dinâmico: `_getdynamic$<i>`.

Exemplo (entrada):
```dart
const baseUrl = OpaqueToken<String>('baseUrl');
class Foo { Foo(Bar b); }
@GenerateInjector([ClassProvider(Foo), ValueProvider.forToken(baseUrl, 'x')])
final InjectorFactory inj = inj$Injector;
```
Saída (dedup inverte: o `ValueProvider` é o índice 0):
```dart
_i1.Injector inj$Injector(_i1.Injector parent) => _Injector$inj._(parent);

class _Injector$inj extends _i1.HierarchicalInjector implements _i1.Injector {
  _Injector$inj._(_i1.Injector parent) : super(parent);

  _i2.Foo? _field1;

  String _getString$0() => 'x';

  _i2.Foo _getFoo$1() => _field1 ??= _i2.Foo(this.get(_i2.Bar));

  _i1.Injector _getInjector$2() => this;

  @override
  Object? injectFromSelfOptional(
    Object token, [
    Object? orElse = _i1.throwIfNotFound,
  ]) {
    if (identical(token, const _i3.OpaqueToken<String>('baseUrl'))) {
      return _getString$0();
    }
    if (identical(token, _i2.Foo)) {
      return _getFoo$1();
    }
    if (identical(token, _i1.Injector)) {
      return _getInjector$2();
    }
    return orElse;
  }
}
```
(Os números `_iN` dependem da ordem de primeira referência no arquivo inteiro; a
quebra de linhas é a do formatador aplicado ao texto do `code_builder`.)
- *Porte*: `RS/injetor.rs:emitir` (260-277) e `emitir_um` (279-412) reproduzem essa
  forma, com a tabela de prefixos compartilhada (`Alocador`, 20-49), cabeçalho
  `// ignore_for_file: no_leading_underscores_for_library_prefixes`, quebra de
  argumentos quando há mais de um (`Ex::texto`, 68-97). `RS/lib.rs` (1541-1575)
  insere os imports logo após `import '<fonte>';` e o corpo no fim. **Coberto**.

---

## 12. Lacunas do porte (priorizadas)

Prioridade pelo risco de **saída errada** (P0) > forma comum sem suporte (P1) >
diagnóstico ausente (P2).

**P0 — saída divergente**
1. `@ContentChild` herdados perdidos: `RS/metadados.rs:membros` atribui
   `d.consultas_de_conteudo` a cada classe da hierarquia (linha 959) em vez de
   acumular (R3.4, R5.2). Agravante: `RS/lib.rs:776-779` retira a pendência
   "filho que herda" quando essa lista sai vazia — uma subclasse sem consulta
   própria de uma base com `@ContentChild` é gerada **sem** a consulta. Corrigir
   para acumular, na ordem supertipos → classe.
2. `changeDetectionLink` ignorado (R2.6): componente com `@changeDetectionLink` é
   gerado sem `isChangeDetectionLink`. Recusar até ter caso.
3. Ganchos por interface indireta (`implements X` com `X extends OnInit`) numa
   classe sem `extends`/`with`: `RS/componente.rs:ganchos_da_classe` não vê (R6.1).
4. Dependência de classe abstrata com construtor `factory` sem nome
   (`RS/metadados.rs:dependencias`, R8.1): o porte devolve zero deps.
5. Nomeado obrigatório em dependência de injetor (R9.2): o oficial o passa como
   argumento posicional; o porte recusa — manter a recusa, documentada.

**P1 — formas não suportadas (hoje recusa)**
6. Tokens: subclasses de `OpaqueToken`/`MultiToken` (R8.4, R11.2); tokens texto/
   número/bool, instância `const` sem argumentos e função (R8.3); `OpaqueToken`
   sem nome fora do injetor; `OpaqueToken<dynamic>`.
7. `useValue` (componente): lista, mapa, `double`, tipo, enum, protobuf, função,
   objetos aninhados em mais de um nível, `useValue: null` (R8.6); injetor: mapa e
   `double` (R11.4).
8. `Module` dentro de `providers:` de componente (R8.5); `viewProviders:` não vazio;
   `providers:` em `@Directive` do próprio arquivo.
9. `useFactory` com método estático (prefixo de classe, `emitPrefix`) (R8.5).
10. `@HostListener` com argumento não-nativo / duplicado no componente; `@ViewChild`
    com seletor múltiplo, tipo prefixado, `read:` não simples, seletor de tipo do
    ngdart; `@ViewChild` herdado (R5.3).
11. `exportAs:` em `@Component` (falta em `ARGUMENTOS_CONHECIDOS`, R2.2);
    `preserveWhitespace:`; `directiveTypes:` (R2.10).
12. `inputTypes` completos (só `booleana` hoje, R3.2): tipo não primitivo com
    `moduleUrl` do membro e argumentos de tipo.
13. `visibility:`/`changeDetection:`/`encapsulation:` escritos por constante ou
    alias (R2.3-R2.5): o porte lê só a forma literal `Enum.valor`.
14. `templateOffset` para `AdjacentStrings` (= offset da aspa) e literais com
    escape (R2.7).
15. Tipo privado em `linkToReference` deve virar `dynamic` (R10.4).
16. Token-anotação prefixado (`@p.meuToken`) em parâmetro (R8.2).
17. `@Attribute`/`HtmlElement`/`ChangeDetectorRef` em dependência de injetor
    (oficial: `this.get(Tipo)`) (R9.2).

**P2 — diagnósticos do oficial ausentes**
18. Nulabilidade × `@Optional` (R8.2, R9.2), `late final` em `@Input`, consultas
    `late`/não-nulas (R3.2, R5.2), `ngDoCheck` `async` (R6.2), aridade de
    `@HostListener` com `args` (R4.3), membro privado com `@HostBinding`/
    `@HostListener` (R4.2-R4.3), classe privada (R1.2), seletor vazio (R2.1),
    `template`+`templateUrl`, `styleUrls` sem `.css` (R2.7), serviço global em
    `providers:` de componente (R8.5), `_prohibitBindingChange` (R3.6),
    `exports:` não identificador/não resolvido (R2.8), `directives:`/`pipes:` não
    resolvidos (R2.9).
19. Correspondência de anotações em `RS/componente.rs` pelo nome escrito (R0.5):
    migrar para a resolução de `metadados.rs`.
