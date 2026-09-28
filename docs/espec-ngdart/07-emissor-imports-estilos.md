# 07 — Emissor, imports, formatação, estilos e arquivo trivial

Especificação derivada da leitura do `ngcompiler-3.0.0-dev.3` (caminhos relativos a
`lib/v1/src/`), do `ngdart-8.0.0-dev.4` (`lib/src/build.dart`), do `source_gen-1.5.0`, do
`code_builder-4.10.1` e do `dart_style-2.3.8` (versões do `pubspec.lock` usado para gerar o
oráculo). Esta seção cobre **tudo o que fixa os bytes do arquivo gerado fora da lógica de
visão**: o encadeamento de builders, o cabeçalho, a ordem e a forma dos imports, a alocação
dos prefixos `importN`, a formatação do emissor e o que o `DartFormatter` faz com ela, os
escapes de string, o arquivo trivial, vários componentes num arquivo, a ordem entre
componentes, `XNgCd` e injetores, os `.css.dart`/`.css.shim.dart` e `deferred`.

Abreviações usadas nas citações:

| Abreviação | Arquivo |
|---|---|
| `AE` | `compiler/output/abstract_emitter.dart` |
| `DE` | `compiler/output/dart_emitter.dart` |
| `OA` | `compiler/output/output_ast.dart` |
| `PU` | `compiler/output/path_util.dart` |
| `TC` | `compiler/template_compiler.dart` |
| `AC` | `compiler/angular_compiler.dart` |
| `CU` | `compiler/compiler_utils.dart` |
| `ID` | `compiler/identifiers.dart` |
| `SC` | `compiler/stylesheet_compiler/style_compiler.dart` |
| `SH` | `compiler/stylesheet_compiler/shadow_css.dart` |
| `SB` | `compiler/stylesheet_compiler/builder.dart` |
| `SP` | `compiler/stylesheet_compiler/processor.dart` |
| `SU` | `compiler/style_url_resolver.dart` |
| `DN` | `compiler/ast_directive_normalizer.dart` |
| `AS` | `angular_compiler/asset.dart` |
| `CB` | `angular_compiler/cli/builder.dart` |
| `FL` | `angular_compiler/cli/flags.dart` |
| `SDE` | `angular_compiler/emitter/split_dart_emitter.dart` |
| `GEN` | `source_gen/template_compiler/generator.dart` |
| `TP` | `source_gen/template_compiler/template_processor.dart` |
| `GC` | `source_gen/template_compiler/code_builder.dart` (`buildGeneratedCode`) |
| `FC` | `source_gen/template_compiler/find_components.dart` |
| `UR` | `source_gen/common/url_resolver.dart` |
| `ATP` | `compiler/template_parser/ast_template_parser.dart` |
| `NGB` | `ngdart-8.0.0-dev.4/lib/src/build.dart` e `build.yaml` |
| `SG:*` | `source_gen-1.5.0/lib/src/` (`builder.dart`, `generated_output.dart`) |
| `CBL:*` | `code_builder-4.10.1/lib/src/` (`allocator.dart`, `emitter.dart`, `specs/expression/literal.dart`) |
| `DS:*` | `dart_style-2.3.8/lib/src/` (`short/source_visitor.dart`, `ast_extensions.dart`) |

Exemplos **(oráculo)** foram conferidos com `corpus/ngdart/oraculo/*` ou com a saída real
do `build_runner` para o `limitless_ui` (`.dart_tool/build/generated/...`). Exemplos
**(derivado)** saem só da leitura do código.

Porte Rust: `crates/gerador_ng/src/` — `lib.rs` (`L`), `visao.rs` (`V`), `expr.rs` (`X`),
`injetor.rs` (`INJ`), `resolucao.rs` (`RS`), `css.rs`, `shadow_css.rs`; e
`crates/build/src/nativos/ng.rs`. As linhas de `visao.rs` mudam com frequência; por isso
as citações do porte usam o nome da função sempre que possível.

---

## 1. Encadeamento de builders e o arquivo como um todo

### R1.1 — Quem gera o quê
O pacote `ngdart` aplica três fábricas (`NGB` `build.yaml`): `templatePlaceholder`
(`.dart → .ng_placeholder`, conteúdo `''`, `CB:69-80`), `templateCompiler`
(`.dart → .template.dart`) e `stylesheetCompiler` (`.css → .css.dart` + `.css.shim.dart`).
`templateCompiler` devolve `Compiler(flags, generate).asBuilder(extension: '.template.dart')`
(`NGB` `build.dart:64`); com a opção `outline-only` gera `.outline.template.dart` (fora do
escopo).

`asBuilder` cria um `source_gen` `LibraryBuilder` com `header: ''` e
`formatOutput: DartFormatter(pageWidth: 1000000).format` (`CB:15`, `CB:44-51`).

- **Rust:** `crates/build/src/descritor.rs:79` (placeholder) e `ng.rs` (despacho).

### R1.2 — Que entradas produzem `.template.dart`
`LibraryBuilder.build` retorna sem escrever nada se a entrada não é biblioteca (parte com
`part of`) (`SG:builder.dart:86`). O `Compiler` não é `GeneratorForAnnotation`, então **toda
biblioteca** `.dart` do pacote (não excluída por `generate_for`) ganha um `.template.dart`,
mesmo sem nada de Angular (ver R1.6).

- **Rust:** coberto — `L:e_parte` (542) e o laço de `L:gerar_em` (437-440) pulam partes.

### R1.3 — Cabeçalho escrito pelo `source_gen`
`_generateForLibrary` (`SG:builder.dart:102-185`):

1. `header` é `''` → nada antes (`SG:builder.dart:61`, `118-120`).
2. `LibraryBuilder` não é parte → sem `part of`.
3. Para cada saída de gerador (aqui uma só): `writeln()` (linha vazia),
   `_headerLine`, `// Generator: AngularDart Compiler`, `_headerLine`, `writeln()`, a saída
   **aparada** (`trim()`, `SG:builder.dart:348`) e `\n` (`SG:builder.dart:152-163`).
   - `_headerLine = '// '.padRight(77, '*')`: `// ` + 74 `*` = 77 caracteres
     (`SG:builder.dart:388`).
   - A descrição é `'Generator: $gen'` porque `Compiler.toString()` =
     `'AngularDart Compiler'` não termina em `Generator` (`SG:generated_output.dart:17-23`,
     `CB:55`).
4. O texto todo passa pelo `DartFormatter` (R5); se ele lança, o texto sai sem formatação
   e só há log (`SG:builder.dart:167-181`).

O `DartFormatter` descarta a linha vazia inicial e mantém uma linha vazia depois do bloco de
comentário. Resultado (oráculo, `d08_servico.template.dart`):

```
// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'd08_servico.dart';
```

- **Rust:** coberto — `L:CABECALHO` (46), 77 caracteres por linha de asteriscos.

### R1.4 — `buildGeneratedCode`: a montagem do corpo
`GC:8-54` (arquivo `source_gen/template_compiler/code_builder.dart`):

```
buffer.writeln("import '$sourceFile';");          // sourceFile = fileName(inputId): só o basename
if (flags.exportUserCodeFromTemplate) buffer.writeln("export '$sourceFile';");
if (injetores não vazios) {
  // imports dos injetores (code_builder) num buffer, corpo em outro
  buffer.writeln(imports); buffer.writeln(compilerOutput); buffer.writeln(body);
} else {
  buffer.writeln(compilerOutput);
}
```

- `sourceFile = fileName(buildStep.inputId)` = último segmento do caminho (`GEN:21`,
  `UR:37-40`).
- `exportUserCodeFromTemplate` é sempre `false`: não há opção de linha de comando/`build.yaml`
  que o ligue (`FL` — a lista `knownArgs` em `parseRaw` não o contém; o padrão é `false`,
  `FL:118`). Logo **nunca** há `export`.
- `compilerOutput = outputs.templateSource?.sourceCode ?? ''` (`GC:18`): a saída do
  `DartEmitter` (R3/R4) ou vazio quando `AngularCompiler.compile` devolve `null` (R2.1).

Ordem final no arquivo (depois da formatação):

1. `import '<basename>.dart';` (sem prefixo);
2. imports dos injetores (`as _i1`, `_i2`…) — só se houver `@GenerateInjector`;
3. imports `importN` do emissor, na ordem de alocação (R3);
4. declarações das visões/`XNgCd` (R2);
5. `// ignore_for_file: no_leading_underscores_for_library_prefixes` e o código dos injetores.

- **Rust:** coberto — `L:gerar_interno` (1465-1574) insere os imports dos injetores logo
  depois de `import '<arquivo>';` e anexa `"\n" + corpo` no fim; `INJ:emitir` (260-275)
  gera `import 'u' as _iN;` e o corpo iniciado pelo `// ignore_for_file`.

### R1.5 — Imports e corpo dos injetores (`code_builder`)
- Alocador `Allocator.simplePrefixing()` (`GC:17`): prefixo `_i` + contador a partir de **1**,
  na ordem de primeira alocação; `dart:core` e referências sem URL ficam sem prefixo
  (`CBL:allocator.dart:73-96`).
- `SplitDartEmitter` (`SDE`) com `orderDirectives: false` e `useNullSafetySyntax: true`:
  cada diretiva vai para o buffer `imports`, escrita **sem quebra** entre elas
  (`CBL:emitter.dart:397-431`, `_newLineBetween` devolve `false` sem ordenação) — o
  formatador as separa em linhas.
- `visitLibrary` escreve, **no buffer do corpo**, a linha
  `// ignore_for_file: no_leading_underscores_for_library_prefixes` antes do código quando
  algum prefixo começa com `_` (`CBL:emitter.dart:534-537`).
- Para cada injetor, `createFactory()` e depois `createClass()` (`GC:33-40`). O conteúdo é
  assunto da seção de injetores.

Exemplo (oráculo, `limitless_ui_example/lib/src/shared/di.template.dart`):

```
import 'di.dart';
import 'package:ngdart/src/di/injector.dart' as _i1;
import 'package:ngforms/src/directives/radio_control_value_accessor.dart' as _i2;
...

// ignore_for_file: no_leading_underscores_for_library_prefixes
_i1.Injector injector$Injector(_i1.Injector parent) => _Injector$injector._(parent);
```

- **Rust:** coberto — `INJ:emitir`.
- Literais de string do `code_builder` (`literalString`) só escapam `'` e `\n`
  (`CBL:literal.dart:49-55`) — diferente do emissor de visões (R4.4). **Rust:** coberto —
  `INJ:literal_de_texto` (101).

### R1.6 — Arquivo trivial
`AngularCompiler.compile` devolve `null` quando a biblioteca não tem `@Component` nem
`@Directive` (`AC:57-58`; pipes e serviços não contam — `AngularArtifacts.isEmpty`, `TC:26`).
Se só há diretivas sem `@HostBinding`, `compile` devolve um módulo com zero instruções e o
emissor produz `''` (R4.1: `toSource` de um contexto vazio é `''`). Nos dois casos
`compilerOutput == ''`, o gerador devolve `"import 'x.dart';\n\n"`, que o `source_gen`
apara. Bytes finais (oráculo, `d08_servico.template.dart`, 219 bytes):

```
// ****...(74 *)\n// Generator: AngularDart Compiler\n// ****...\n\nimport 'd08_servico.dart';\n
```

- **Rust:** coberto — `L:template_trivial` (331); `L:gerar_visoes` (1577-1627) devolve o
  trivial sem componentes/diretivas/pipes e quando só há diretivas sem `@HostBinding`.

---

## 2. O que entra no módulo e em que ordem

### R2.1 — Descoberta de componentes e diretivas
`findComponentsAndDirectives` percorre a biblioteca com um `RecursiveElementVisitor`
(`FC:36-47`); em cada `ClassElement` com `@Component` acrescenta a `components`, com
`@Directive` a `directives` (`FC:71-94`). A ordem é a do percurso do analyzer: unidade
definidora e depois as partes, classes na ordem de declaração **(derivado: comportamento do
visitor do analyzer, não do ngcompiler)**. Pipes não entram em nenhuma lista.

- **Rust:** `L:achar` (253) varre só `unidade.declarations` da unidade analisada. Componente
  declarado num arquivo `part` **não é visto** pela biblioteca que o inclui — ver Lacunas.

### R2.2 — Ordem das instruções no módulo
`TemplateCompiler.compile` (`TC:62-79`):

1. Para cada componente, na ordem de R2.1, para cada visão em
   `[ComponentView, HostView]` (`AC:108-111`):
   - as instruções de estilo (`styles$X` para a visão do componente, `styles$XHost` para a
     hospedeira — `TC:95-97`), e
   - as instruções da visão (classe, fábricas, visões embutidas — seção de visões).
2. Depois de **todos** os componentes, para cada diretiva com
   `requiresDirectiveChangeDetector` (`hostProperties` não vazio, `ir/model.dart:76`), a
   classe `XNgCd` (`TC:71-76`).
3. O módulo inteiro é emitido **numa única chamada** a `emitStatements` (`TC:130-142`): a
   tabela de imports é compartilhada por todos os componentes e `XNgCd`.

O `moduleUrl` do módulo é `templateModuleUrl` do primeiro componente (ou da primeira
diretiva, se não houver componente): `asset:<pkg>/<dir>/<x>.template.dart` (`AC:171-179`,
`CU:5-10`).

Exemplo (oráculo, `j44_varios_com_folha.template.dart`, três componentes):

```
final List<Object> styles$J44Primeiro = [import0.styles];
class ViewJ44Primeiro0 ...
const _J44PrimeiroNgFactory = ...
...
final List<Object> styles$J44PrimeiroHost = const [];
class _ViewJ44PrimeiroHost0 ...
import10.HostView<import2.J44Primeiro> viewFactory_J44PrimeiroHost0() {...}
final List<Object> styles$J44Segundo = ['i._ngcontent-%ID%{color:red}'];
...
final List<Object> styles$J44Terceiro = [import11.styles, import0.styles];
```

e (oráculo, `j45_componente_e_ngcd.template.dart`) as classes `J45AtivoNgCd` e
`J45DepoisNgCd` depois da última fábrica da hospedeira.

- **Rust:** coberto — `L:gerar_visoes` (1628-1654) monta os trechos na ordem do fonte com
  uma `Importacoes` compartilhada e põe `V:classe_ngcd` depois; arquivo só com diretivas:
  `V:detector_de_diretivas`.

### R2.3 — Injetores
Os `@GenerateInjector` (`InjectorReader.findInjectors`, `TP:25`) vão para o fim do arquivo
(R1.4), independentes do módulo das visões.

- **Rust:** coberto — `L:gerar_interno`.

---

## 3. Imports e prefixos do emissor de visões

### R3.1 — Tabela `importsWithPrefixes`
`_DartEmitterVisitor.importsWithPrefixes` é um `Map<String,String>` de **`moduleUrl` →
prefixo**, com ordem de inserção (`DE:104`). É preenchido só por `_computeModulePrefix`
(`DE:727-754`), chamado ao **escrever** um `ExternalExpr` ou `ExternalType`
(`DE:127-129`, `DE:635-643`, `DE:703-724`):

```
se moduleUrl != null e moduleUrl != _moduleUrl:
    se já está na tabela: prefixo = tabela[moduleUrl]
    senão:
        prefixo = moduleUrl ∈ _allowListedImports ? '' : 'import${tabela.length}'
        tabela[moduleUrl] = prefixo
```

Consequências:

- O número `N` de `importN` é a **posição** do módulo na tabela; um módulo da lista
  liberada ocupa uma posição mas sai sem prefixo — por isso faltam números (oráculo:
  `import 'package:ngdart/angular.dart';` entre `import8` e `import10` em `j45`).
- Como o emissor escreve o texto em ordem e o formatador não reordena nada, **a numeração é
  a ordem da primeira ocorrência textual de cada `moduleUrl` distinto no arquivo final**
  (sem contar a linha `import '<basename>';` de R1.4 nem a parte dos injetores).
- `moduleUrl == _moduleUrl` (o próprio `.template.dart`): sem prefixo e sem import. Ex.
  (oráculo, `j45`): `late final J45AtivoNgCd _J45Ativo_0_5;` — a classe `XNgCd` mora no
  próprio módulo.
- `moduleUrl == null`: sem prefixo e sem import.
- Classes do usuário têm `moduleUrl` `asset:<pkg>/<dir>/<x>.dart` (`UR:8-24`), diferente do
  módulo `…/<x>.template.dart`: o arquivo fonte é importado **de novo**, agora com prefixo
  (oráculo, `j45`: `import 'j45_componente_e_ngcd.dart' as import1;`, além da linha sem
  prefixo de R1.4).
- A chave é o `moduleUrl`, não o caminho impresso: dois `moduleUrl` diferentes que viram o
  mesmo caminho dão dois imports com prefixos diferentes.

- **Rust:** `V:Importacoes` (25-89): `alias`/`q` alocam na ordem da chamada, com número =
  posição; `SEM_PREFIXO` ocupa número sem prefixo; `escrever` imprime na ordem.
  A chave é o **caminho impresso**, não o `moduleUrl`; o caso de duas chaves para o mesmo
  caminho é tratado à parte por `q_chave` (usado no argumento de tipo de `MultiToken`).
  Como a emissão Rust monta blocos fora da ordem textual, a alocação tardia usa marcas
  `V:tardio`/`tardio_q`/`resolver_tardios`, resolvidas na ordem do texto — o
  que implementa a invariante acima. Coberto nos casos conhecidos.

### R3.2 — Lista liberada (`_allowListedImports`)
`DE:76-100`, comparada com o `moduleUrl`:

```
package:ngdart/angular.dart
dart:core
asset:ngdart/lib/src/core/linker/element_ref.dart        (+ package:ngdart/src/core/linker/element_ref.dart)
asset:ngdart/lib/src/core/linker/view_container.dart     (+ package:)
asset:ngdart/lib/src/core/linker/template_ref.dart       (+ package:)
asset:ngdart/lib/src/core/change_detection/change_detection.dart (+ package:)
asset:ngdart/lib/src/common/directives/ng_if.dart        (+ package:)
asset:ngdart/lib/src/core/linker/app_view.dart           (+ package:)
asset:ngdart/lib/src/core/render/api.dart                (+ package:)
```

Note-se que `asset:ngdart/lib/angular.dart` **não** está na lista, só a forma `package:`.
Os identificadores de `ID` usam `_angularRootUrl = 'package:ngdart/angular.dart'` para
`ComponentFactory` etc. e `asset:ngdart/lib/...` para o resto (`ID:4-5`).

`dart:core` entra na tabela quando algum `ExternalType` vem do `dart:core` (tipos lidos do
analyzer, ex. `unsafeCast<String>`); sai `import 'dart:core';` numerado e sem prefixo
(oráculo, `a10_ng_for.template.dart:23`). Os tipos embutidos do emissor (`o.stringType`,
`o.intType`…) são `BuiltinType` e **não** alocam import (`DE:588-632`).

- **Rust:** coberto — `V:SEM_PREFIXO` usa os caminhos `package:` equivalentes. Diferença
  teórica: um identificador com `moduleUrl` `asset:ngdart/lib/angular.dart` levaria prefixo
  no oficial e sairia sem prefixo no Rust (não observado no corpus).

### R3.3 — Caminho escrito no import: `getImportModulePath`
`DartEmitter.emitStatements` (`DE:51-70`), para cada entrada da tabela, em ordem:

```
importPath = getImportModulePath(moduleUrl_do_módulo, moduleUrl_importado)
prefix.isEmpty ? "import '$importPath';" : "import '$importPath' as $prefix;"
```

As linhas de import são unidas por `\n` e seguidas do texto do contexto (`DE:62-69`).

`getImportModulePath` (`PU:10-30`), com `_assetUrlRe = asset:([^/]+)/([^/]+)/(.+)`
(pacote, pasta de primeiro nível, resto):

1. Importado que **não** casa `asset:` (ex. `dart:html`, `package:…`) → devolvido **tal
   qual**.
2. Importado igual ao módulo → só o último segmento.
3. Mesmo pacote **e** mesma pasta de primeiro nível → caminho relativo
   (`_getRelativePath`, `PU:49-71`): maior prefixo comum de segmentos `k`; sobe
   `segmentos_do_módulo - 1 - k` vezes com `..`; acrescenta os segmentos restantes do
   importado; junta com `/`.
4. Senão, se a pasta do importado é `lib` → `package:<pkg>/<resto>`.
5. Senão → `StateError("Can't import url ...")` (a build falha).

Exemplos (derivado; os dois primeiros no oráculo):

| módulo | importado | import escrito |
|---|---|---|
| `asset:p/lib/src/a.template.dart` | `asset:p/lib/src/a.dart` | `a.dart` |
| `asset:p/lib/src/a.template.dart` | `asset:ngdart/lib/src/utilities.dart` | `package:ngdart/src/utilities.dart` |
| `asset:p/lib/src/x/a.template.dart` | `asset:p/lib/src/b.dart` | `../b.dart` |
| `asset:p/test/a_test.template.dart` | `asset:p/lib/b.dart` | `package:p/b.dart` |
| `asset:p/lib/a.template.dart` | `asset:p/test/b.dart` | erro |
| qualquer | `package:p/src/a.css.shim.dart` | `package:p/src/a.css.shim.dart` (verbatim) |

- **Rust:** coberto — `RS:caminho_do_import` (293-306) e `RS:relativo` (330-341) para
  `asset:`; os chamadores tratam `dart:` e `package:` à parte.

### R3.4 — Prefixo de membro estático (`emitPrefix`)
Se o identificador tem `emitPrefix` e `prefix` não vazio (funções de fábrica/`useFactory`
estáticas: `prefix` = nome da classe, `source_gen/template_compiler/compile_metadata.dart:564-591`),
o prefixo final é `importN.Classe` (ou só `Classe` se o módulo é o próprio) e o nome vem
depois do ponto (`DE:747-752`, `DE:757-772`). Com nome vazio, imprime só o prefixo.

Ex. (derivado): `useFactory: Servico.criar` em `asset:p/lib/s.dart` →
`import3.Servico.criar(...)`.

- **Rust:** não verificado como regra geral; depende dos pontos de `V`/`diretivas.rs` que
  emitem fábricas.

---

## 4. O emissor (`AbstractEmitterVisitor` + `_DartEmitterVisitor`)

O `DartEmitter` é criado com `emitNullSafeSyntax: true`
(`compiler/module/ng_compiler_module.dart:31`) e `_escapeDollarInStrings = true`
(`DE:112-118`). Para o `.template.dart` a saída crua passa depois pelo `DartFormatter` (R5);
para os `.css.dart`/`.css.shim.dart` **não** (R6.5) — lá os bytes crus do emissor valem.

### R4.1 — Contexto de linhas (`EmitterVisitorContext`)
`AE:17-93`:

- Uma lista de linhas; cada linha guarda `indent` (nível) e `parts`.
- `print(part, newLine)`: acrescenta `part` se não vazio; com `newLine`, abre nova linha
  com o nível atual e zera `_outputPos`; sem, soma `part.length` a `_outputPos`.
- `println(last)`: `print(last, true)` e depois `_outputPos += last.length` — a posição
  fica "suja" com o tamanho da última parte (efeito só no teste de largura de R4.6).
- `incIndent`/`decIndent` mudam o nível e **também o da linha corrente**.
- `currentLineLength = indent + _outputPos` — o nível conta 1 por nível, não 2 espaços.
- `toSource()`: tira a última linha se vazia; cada linha vira `'  ' * indent + parts.join('')`
  (linha sem partes vira `''`); junta com `\n`. **Não há `\n` final.**

### R4.2 — Instruções

| Instrução | Saída crua | Cit. |
|---|---|---|
| expressão | `expr` + (`/* REF:url:ini:fim */` se houver `sourceReference`) + `;` + quebra | `AE:108-126` |
| `return` | `return expr;` / `return;` (+ `inlineComment`, sempre `''` no compilador) | `AE:129-138` |
| `if` sem `else` e ≤1 instrução | `if (c) { stmt; }` numa linha | `AE:147-170` |
| `if` geral | `if (c) {` ↵ corpo ↵ `} else {` ↵ … ↵ `}` | `AE:157-169` |
| `throw` | `throw e;` | `AE:176-180` |
| comentário | `// ` + linha, por linha | `AE:183-189` |
| `try` | `try {` ↵ … ↵ `} catch (error, stack) {` ↵ … ↵ `}` | `DE:447-457` |
| var | `[static ][late [final] |final |const |var ][Tipo ]nome[ = valor];` | `DE:132-175` |

Notas de `visitDeclareVarStmt` (`DE:132-175`): `late`/`late final` saem como tal (null-safe);
`final` ganha de `const`; `var` só quando não há tipo nem modificador; `const` liga o
contexto constante (R4.5) até o fim da declaração.

`REF`: `sourceUrl` é a `Uri` do template (`package:…html` para `templateUrl` em `lib/`, a do
`.dart` para template em linha) e os deslocamentos somam `templateOffset`
(`ir/model.dart:583-589`, `631-634`). Depois do formatador fica
`expr /* REF:… */;` (oráculo, `a10_ng_for.template.dart:115`).

- **Rust:** coberto — `html.rs` (37-58, 165, 194: deslocamentos em unidades UTF-16).

### R4.3 — Expressões: parênteses e formas fixas
O emissor põe parênteses próprios nas formas abaixo; o formatador **não os remove**, então
eles aparecem no arquivo final.

| Nó | Saída | Cit. |
|---|---|---|
| `BinaryOperatorExpr` (exceto identidade) | `(a op b)` com `==`, `!=`, `-`, `+`, `/`, `*`, `%`, `&&`, `||`, `<`, `<=`, `>`, `>=` | `AE:464-473`, `OA:151-165` |
| `identical` / `notIdentical` | `identical(a, b)` / `!identical(a, b)` (sem parênteses externos) | `DE:460-480` |
| `NotExpr` | `(!x)` | `AE:435-439` |
| `NotNullExpr` | `(x!)` | `AE:442-446` |
| `ConditionalExpr` | `(c? a: b)` → formatado `(c ? a : b)` | `AE:409-420` |
| `IfNullExpr` | `(a?? b)` → `(a ?? b)` | `AE:423-432` |
| `CastExpr` | `(x as T)` | `DE:178-184` |
| `Write*Expr` fora do início da linha | `(alvo = valor)`; no início da linha, sem parênteses | `AE:198-283`, `DE:433-444` |
| `ReadClassMemberExpr` | `this.nome` | `DE:427-430` |
| `InvokeMemberMethodExpr` | `this.m(args)` | `AE:311-324` |
| `InvokeMethodExpr` | `r.m(args)` / `r?.m(args)`; `concatArray` sai `r..addAll(args)` | `AE:286-308`, `OA:518-519` |
| `ReadPropExpr` | `r.p` / `r?.p` | `AE:476-486` |
| `InstantiateExpr` | `[const ]Classe<T>(args)` — **sem `new`** | `DE:561-585` |
| `InvokeFunctionExpr` | `f<T>(args)` | `DE:540-558` |
| `FunctionExpr` | `(params) {` ↵ corpo ↵ `}` (sempre corpo em bloco) | `DE:366-374` |
| `SpreadExpr` | `...x` | `AE:449-452` |
| `ReadVarExpr` embutidas | `super`, `this`, `error`, `stack`, `_METADATA` | `AE:327-351`, `DE:418-424` |

Argumentos (`AE:541-575`): posicionais separados por `,`; os nomeados vêm **depois** de todos
os posicionais, cada um como `nome: valor,` — **vírgula depois de cada nomeado**, e uma
vírgula entre o último posicional e o primeiro nomeado. A vírgula final muda a formatação
(R5.3).

Exemplos (oráculo, `limitless_ui`, `breadcrumbs_component.template.dart`):

```
if ((styles == null)) {
  _componentStyles = (styles = import6.ComponentStyles.unscoped(styles$LiBreadcrumbComponent, _debugComponentUrl));
return (import9.isDevMode ? 'asset:limitless_ui/lib/src/components/breadcrumbs/breadcrumbs_component.dart' : null);
```

e `(this.parentView!).injectorGet(import10.SweetAlertService, this.parentIndex)`.

- **Rust:** coberto por construção nas cadeias de `V`/`X` (o porte escreve direto a forma
  já formatada). Não há um emissor genérico de `OutputAst`; cada forma nova precisa ser
  escrita à mão com estes parênteses.

### R4.4 — Literais e `escapeSingleQuoteString`
`visitLiteralExpr` (`AE:392-403`):

- `String` → `escapeSingleQuoteString(v, true)`;
- `EscapedString` → `'${v}'` sem escape nenhum (só mensagens i18n com HTML,
  `view_compiler/compile_view.dart:683-697`);
- `null` → `null`; o resto → `'$value'` do Dart (`true`, `1`, `1.5`, `1e+21`…).

`escapeSingleQuoteString` (`AE:612-628`), regex `'|\\|\n|\r|\$`:

| caractere | saída |
|---|---|
| `'` | `\'` |
| `\` | `\\` |
| LF | `\n` |
| CR | `\r` |
| `$` | `\$` (com `escapeDollar = true`, sempre no `DartEmitter`) |
| qualquer outro (tab, não-ASCII, `"`) | como está |

Envolvido em aspas simples. Ex.: `a'b$c\` + LF → `'a\'b\$c\\\n'`; oráculo
`this.locals['\$implicit']`.

- **Rust:** coberto — `V:literal`, mesma tabela.

### R4.5 — Coleções, `const` e tipos de coleção
- `LiteralArrayExpr` (`DE:495-517` + `AE:497-508`): `const ` se o tipo tem
  `constModifier` e ainda não se está em contexto constante; depois `<dynamic>` se o tipo
  **da expressão** é `dynamicType`, `<Object>` se é `objectType` ou `ArrayType(of: Object)`;
  depois `[` itens `]`. Com **mais de um item**: `[`, quebra, itens com `keepOnSameLine`
  (R4.6), quebra, `]` e quebra.
- `LiteralMapExpr` (`DE:519-537` + `AE:510-539`): `const ` idem; `<String, V>` se há
  `valueType`; chaves `String` passam por `escapeSingleQuoteString`; `chave: valor`; com
  mais de uma entrada, uma por linha (`keepOnSameLine: false`).
- `_inConstContext` (`DE:106-107`): dentro de `const x = …` ou de coleção/`new` constante
  nenhum `const` interno é repetido. Ex. (oráculo):
  `const _XNgFactory = ComponentFactory<import2.X>('x', viewFactory_XHost0);`.
- Tipos (`DE:587-690`), com null-safety: `bool`, `dynamic`, `Object`, `Function`, `num`,
  `int`, `double`, `String`, `Null`, `Never`, `void`; `?` quando anulável; `List<T>`
  (`List<dynamic>` sem `of`), `Map<String, V>`, `R Function(P,…)`.

### R4.6 — Quebra de linha de listas (`visitAllObjects`)
`AE:577-600`: depois de cada item exceto o último, imprime o separador; com
`keepOnSameLine`, quebra **depois** do separador só se `currentLineLength > 80` (medido
antes de imprimir o separador); sem `keepOnSameLine`, quebra se `newLine`. Com `newLine`,
`println()` no fim. `length` é o do `String` do Dart (unidades UTF-16).

No `.template.dart` isso é apagado pelo formatador (R5.4). Nos `.css*.dart` é o que fixa os
bytes (R6.5).

### R4.7 — Classes e funções de topo
`visitDeclareClassStmt` (`DE:187-218`):

```
class Nome<T extends B> extends Pai {
  <campos COM inicializador, na ordem>
  <campos SEM inicializador, na ordem>
  <construtor>
  <getters>
  <métodos>
}
```

- Campo: `[static ][late [final] |final |var ][Tipo ]nome[ = init];` (`DE:234-266`).
- Construtor: anotações (`@x` + quebra cada), `Nome(params)`, `: super(...)` se a
  primeira instrução de inicialização é `super(...)`, e `;` se o corpo é vazio ou
  ` {` ↵ corpo ↵ `}` (`DE:284-309`).
- Getter: anotações, `[static ][Tipo ]get nome {` (`DE:268-282`).
- Método: anotações, `[static ]Tipo|void nome(params) {` (`DE:311-328`).
- Função de topo: anotações, `Tipo|void nome<T>(params) {`; getter de topo:
  `Tipo get nome {` (`DE:376-404`).
- Parâmetros: `Tipo nome` separados por `,` (`DE:692-701`); parâmetros de tipo separados
  por `, `, sem `extends dynamic` (`DE:344-363`).

- **Rust:** coberto por construção (cadeias de `V`).

---

## 5. O que o `DartFormatter` faz com a saída (só `.template.dart`)

`DartFormatter(pageWidth: 1000000)` do `dart_style 2.3.8`, sem `languageVersion` e sem o
experimento `tall-style` → **estilo curto** (`DS:dart_formatter.dart:83-94`, `220`). Com a
página de um milhão de colunas **nada quebra por largura**; só quebras obrigatórias.

### R5.1 — Espaçamento
Espaços normalizados em volta de operadores, depois de vírgulas (`f(a,b)` → `f(a, b)`),
`c? a: b` → `c ? a : b`, `a?? b` → `a ?? b`; recuo de 2 por bloco; `@override` em linha
própria; comentário de bloco ganha um espaço antes (`x/* REF */;` → `x /* REF */;`).
Conteúdo de strings nunca muda. Parênteses nunca são removidos nem acrescentados.

### R5.2 — Linhas em branco
- Diretivas: separadas por uma quebra (as cruas não têm linha vazia entre si)
  (`DS:short/source_visitor.dart:657`).
- Declarações de topo (`DS:short/source_visitor.dart:659-689`): linha vazia antes da
  primeira declaração; linha vazia **antes e depois** de toda `class`; linha vazia **depois**
  de função/getter de topo com corpo em bloco não vazio; entre as demais (variáveis,
  `const`), uma quebra só. Ex. (oráculo):

  ```
  }

  const _XNgFactory = ComponentFactory<import2.X>('x', viewFactory_XHost0);
  ComponentFactory<import2.X> get XNgFactory {
    return _XNgFactory;
  }

  ComponentFactory<import2.X> createXFactory() {
  ```

- Membros de classe e instruções de bloco (`DS:short/source_visitor.dart:3430-3444`,
  `DS:ast_extensions.dart:59-71`): linha vazia depois de todo **`MethodDeclaration`**
  (método ou getter) com corpo em bloco não vazio, exceto o último; **construtor e campos
  não** contam. Por isso o construtor da visão é seguido direto de
  `static String? get _debugComponentUrl {`, e há linha vazia antes de cada `@override`.

### R5.3 — Vírgula final: argumentos nomeados um por linha
Lista de argumentos com vírgula final é formatada como coleção: cada argumento numa linha,
+2 de recuo, `)` em linha própria (`DS:short/source_visitor.dart:252-259`). Como o emissor
põe vírgula depois de todo nomeado (R4.3), **toda chamada com argumento nomeado quebra**.
Ex. (oráculo, `li_multi_select.template.dart:393`):

```
    _ctx.closeDropdown(
      restoreFocus: false,
      preserveFocusTarget: $event.target,
    );
```

- **Rust:** coberto — `X:QUEBRA`/`RECUO` (98-140) e `X` 797-813; recusa argumento que
  quebra dentro de lista sem nomeado (forma não vista).

### R5.4 — Coleções
Listas/mapas crus com quebras (R4.5/R4.6) **voltam a uma linha**: o estilo curto só preserva
quebras de coleção com comentário de linha dentro (`DS:short/source_visitor.dart:3689-3721`).
Força a quebra: vírgula final (não emitida pelo compilador em coleções) ou **coleção
não vazia aninhada em outra** (`DS:short/source_visitor.dart:3675-3741`, `_collectionSplits`)
**(derivado; sem caso no oráculo)**.

### R5.5 — Condicional com corpo de função em bloco
Um operando que contém função com corpo em bloco força a regra do condicional a quebrar
(`DS:short/source_visitor.dart:691-742`): `?` e `:` vão para linhas próprias, +4 em relação à
instrução. Ex. (oráculo, `li_sweet_alert_directive_test.template.dart:46-50`):

```
    this._LiSweetAlertDirective_0_5 = (import6.isDevMode
        ? import9.debugInjectorWrap(import2.LiSweetAlertDirective, () {
            return import2.LiSweetAlertDirective((this.parentView!).injectorGet(import10.SweetAlertService, this.parentIndex));
          })
        : import2.LiSweetAlertDirective((this.parentView!).injectorGet(import10.SweetAlertService, this.parentIndex)));
```

Sem bloco, o condicional fica numa linha (ex. `_debugComponentUrl`).

- **Rust:** coberto só para as formas conhecidas, com as cadeias fixas `({util}.isDevMode\n        ? …` de `V` (criação de diretiva/componente com injeção e provedores da hospedeira); não
  há um formatador geral.

### R5.6 — Fim do arquivo
O formatador remove a linha vazia inicial de R1.3 e termina o arquivo com exatamente um
`\n`.

- **Rust:** coberto — cada trecho de `V:montar_arquivo` começa com `\n` e termina em
  `}\n`; `X:resolver_quebras` aplica o recuo das quebras.

---

## 6. Estilos

### R6.1 — Normalização do componente
`_normalizeLoadedTemplate` (`DN:135-172`):

- `styleUrls` resolvidos contra o `moduleUrl` do componente por `NgAssetReader.resolveUrl`
  (`DN:174-185`, `AS:18-33`): `AssetId.resolve(url, from: …)`, que normaliza `.`/`..`, e
  `asset.uri.toString()` — **`package:<pkg>/<resto>` para arquivos em `lib/`** e
  `asset:<pkg>/<dir>/<resto>` fora de `lib/` **(derivado do `AssetId.uri` do `build`)**.
  URL não resolúvel (`/abs`, `http:`…) → `BuildError "Invalid Style URL"`.
- `emulated` sem `styles` e sem `styleUrls` vira `none` (`DN:154-160`).
- `styles` (texto em linha) **não** passa por `extractStyleUrls`: um `@import` nele fica no
  texto e vai para o shim.

### R6.2 — `styles$X` e `styles$XHost`
`StyleCompiler` (`SC:38-109`):

- Visão do componente: `_compileStyles('styles$X', component.styles, component.styleUrls,
  shim = encapsulation == emulated)`.
- Visão hospedeira: `_compileStyles('styles$XHost', [], [], true)`.
- `_compileStyles` (`SC:56-95`): primeiro, para cada URL de `styleUrls`, um
  `ExternalExpr(name: 'styles', moduleUrl: stylesModuleUrl(url, shim))`; depois, para cada
  texto de `styles`, `o.literal(shim ? shimShadowCss(texto, '_ngcontent-%ID%',
  '_nghost-%ID%') : texto)`. Lista **`const` só quando vazia**. Declaração
  `final List<Object> styles$X = [...]`.
- `stylesModuleUrl(url, shim)` = `url + (shim ? '.shim.dart' : '.dart')` (`CU:12-16`).

Consequências (oráculo, `j44`):

```
final List<Object> styles$J44Primeiro = [import0.styles];              // styleUrls
final List<Object> styles$J44Segundo = ['i._ngcontent-%ID%{color:red}']; // styles, emulated
final List<Object> styles$J44Terceiro = [import11.styles, import0.styles]; // import reaproveitado
final List<Object> styles$J44PrimeiroHost = const [];
```

- O import da folha é alocado quando `styles$X` é escrito (primeira instrução do
  componente): no primeiro componente, é o `import0`; nos seguintes, entra no meio da
  tabela (`import11` acima). URL repetida reaproveita o prefixo (mesmo `moduleUrl`).
- O import sai com o `package:` **verbatim** (R3.3 regra 1), mesmo com a folha na mesma
  pasta: `import 'package:corpus_ngdart/src/j44_primeiro.css.shim.dart' as import0;`.
  Fora de `lib/` o `moduleUrl` é `asset:` e o import sai relativo.
- `encapsulation: none` → `.css.dart` e texto sem shim.

- **Rust:** coberto para `lib/` e URL simples — em `V:gerar_componente_com`: o laço de
  `c.styles` (texto em linha, com `css::shim` no emulado), o laço de `c.style_urls` que aloca
  o import na escrita, o `match (itens.is_empty(), c.sem_encapsulamento)` que escolhe
  `const []`/`scoped`/`unscoped`, e o modelo com `styles${x}Host = const []`;
  `V:Local::uri_do_estilo`. Faltas: ver Lacunas (URL com
  `..`, URL `package:`, componente fora de `lib/`, `@import` em `styles:`).

### R6.3 — O builder de folhas
`StylesheetCompiler` (`SB:11-32`) roda para **todo `.css`** do pacote (inclusive os gerados
pelo `sass_builder`): `processStylesheet` (`SP:7-20`) com `stylesheetUrl = toAssetUri(id)` =
`asset:<pkg>/<caminho>` (`UR:26-28`), `TemplateCompiler.compileStylesheet` (`TC:107-123`):

- `.css.dart` = `_compileStyles('styles', [estilo], urls, false)` com módulo
  `asset:<pkg>/<caminho>.css.dart`;
- `.css.shim.dart` = o mesmo com `shim = true` e módulo `…css.shim.dart`.

Cada um passa por `emitStatements` e é escrito com `buildStep.writeAsString` **sem
formatador e sem cabeçalho** (`SB:24-32`, `SP:22-28`).

### R6.4 — `extractStyleUrls`
`SU:18-39`. Regex:

```
@import\s+(?:url\()?\s*(?:(?:['"]([^'"]*))|([^;\)\s]*))[^;]*;?
```

Para cada casamento, `url = grupo1 ?? grupo2`; se `isStyleUrlResolvable(url)` (não vazia,
não começa com `/`, e sem esquema ou esquema `package`/`asset` — `SU:8-14`), o casamento
inteiro é **removido** do texto e `Uri.parse(stylesheetUrl).resolve(url)` entra na lista;
senão o texto fica. Ordem: a de aparição. O texto restante (com as quebras de linha que
sobram) é o estilo.

Ex. (oráculo, `j36_estilo_importa.css`):

```
@import 'j36_base.css';
@import url(https://fonts.example.com/x.css);
.a {
  color: blue;
}
```

→ URLs `[asset:corpus_ngdart/lib/src/j36_base.css]`, texto
`\n@import url(https://fonts.example.com/x.css);\n.a {\n  color: blue;\n}\n`.

- **Rust:** coberto — `L:extrair_imports` (1901-1949), `L:resolvivel` (1952),
  `L:resolver_url` (1966). Diferença: `@import 'package:…'` — ver R6.5.

### R6.5 — Bytes exatos de `.css.dart` / `.css.shim.dart`
Saída crua do emissor (R4.1, R4.5, R4.6):

```
[import '<caminho>' as importK;\n]*      // um por módulo distinto, na ordem de aparição
final List<Object> styles = <lista>;     // sem \n final
```

- Um item só: `final List<Object> styles = ['…'];` numa linha.
- Mais de um item (tem `@import`):
  ```
  final List<Object> styles = [
    import0.styles,import1.styles,'…'
  ]
  ;
  ```
  Itens separados por `,` **sem espaço**, na mesma linha até `1 + (caracteres UTF-16 da
  linha sem o recuo) > 80` antes do separador — aí a `,` fica na linha e o item seguinte vai
  para a próxima, com dois espaços; `]` e `;` em linhas próprias; **sem quebra final**.
- Os `importK.styles` vêm antes do texto; o texto é o literal escapado (R4.4) — no
  `.css.shim.dart`, depois do shim (R6.6).
- Caminho do import: `getImportModulePath('asset:…/x.css.shim.dart', url + sufixo)`. Com
  `@import` relativo ou `asset:`, relativo; com `@import 'package:…'`, o `package:`
  **verbatim** (o `Uri.resolve` mantém o esquema e R3.3 regra 1 o devolve tal qual). URL
  repetida reaproveita o mesmo `importK`.
- Nunca é `const`: há sempre o literal do texto (mesmo `''`).

Ex. (oráculo, `j36_estilo_importa.css.shim.dart`, bytes completos):

```
import 'j36_base.css.shim.dart' as import0;
final List<Object> styles = [
  import0.styles,'@import url(https://fonts.example.com/x.css);\n.a._ngcontent-%ID%{color:blue}'
]
;
```

- **Rust:** `L:gerar_folha` (1812-1864) + `L:lista_de_estilos` (1871-1893): coberto, inclusive
  a conta de largura em UTF-16. Diferenças: (a) `@import` repetido ganha `import0`,
  `import1`… com a mesma URL (o oficial reaproveita); (b) `@import 'package:<mesmo pacote>/…'`
  vira caminho relativo (`RS:asset_de_uri` + `caminho_do_import`) em vez do `package:`
  verbatim.

### R6.6 — `shimShadowCss`
`SH:80-100`: `parse(css, errors)` do `csslib` (erros só viram `logWarning`, a saída sai assim
mesmo); `_ShadowTransformer(contentClass = '_ngcontent-%ID%', hostClass = '_nghost-%ID%')`
(`SC:12-19`) ou, com `use_legacy_style_encapsulation`, `_LegacyShadowTransformer`;
impressão com `CssPrinter` compacto (formato do `csslib`: sem espaços supérfluos,
`>`/`+`/`~` com espaço em volta, cores nomeadas… — seção do csslib).

`_ShadowTransformer.visitSelectorGroup` (`SH:549-572`) — para cada seletor do grupo:

1. Converte em `_ComplexSelector`: sequências agrupadas em compostos, cortando onde a
   sequência tem combinador diferente de "nenhum" (`SH:226-238`).
2. Se algum composto tem `:host-context(...)`, acrescenta **logo depois** um segundo
   seletor (`_createDescendantHostSelectorFor`, `SH:450-481`): cada composto com
   `:host-context(A)` vira dois — ancestral com o combinador original contendo `A`, e
   descendente (combinador descendente) com o resto do composto + `:host`.
3. `shimSelectors` (`SH:498-547`):
   - `deepIndex` = índice do **primeiro** composto que é `::ng-deep` (varrendo de trás para
     frente); esse composto vira um seletor de tipo vazio com o mesmo combinador
     (`SH:305-321`, `485-492`) — imprime só o espaço do combinador.
   - Do composto `deepIndex-1` para trás até achar um com `:host`, `:host-context()` ou
     `:global-context()` (que vira `hostIndex`): acrescenta `._ngcontent-%ID%` com `add`
     (`SH:507-516`).
   - Do `hostIndex` para trás até 0: `:host(A)`/`:host-context(A)` viram `._nghost-%ID%` e
     os simples de `A` entram no composto com `addAll`; `:host` vira `._nghost-%ID%`;
     `:global-context(A)` sai e `A` entra com `addAll` (`SH:519-546`).
4. Ordem dentro do composto (`_compare`, `SH:329-356`): seletor de tipo/namespace primeiro,
   pseudo-elemento por último, o resto na ordem de chegada; `add` insere antes do primeiro
   com que compara `< 0` (antes de um pseudo-elemento); `addAll` faz *merge* estável
   (`SH:360-399`). Tipo duplicado ou pseudo-elemento duplicado só avisam.
5. `toSequences` zera o combinador de todas as sequências do composto e põe o do composto
   na primeira (`SH:409-417`).

Exemplos (oráculo: testes de `crates/gerador_ng/src/css.rs`, conferidos com o oficial):

| entrada | saída |
|---|---|
| `:host { display: block; }` | `._nghost-%ID%{display:block}` |
| `.a .b {…}` | `.a._ngcontent-%ID% .b._ngcontent-%ID%{…}` |
| `a:hover {…}` | `a:hover._ngcontent-%ID%{…}` |
| `.a::before {…}` | `.a._ngcontent-%ID%::before{…}` |
| `:host(.tema-escuro) .b` | `._nghost-%ID%.tema-escuro .b._ngcontent-%ID%` |
| `:host-context(.pai) .c` | `._nghost-%ID%.pai .c._ngcontent-%ID%,.pai ._nghost-%ID% .c._ngcontent-%ID%` |
| `::ng-deep .d` | ` .d` (espaço inicial) |
| `:host(span.x) > .d::before` | `span._nghost-%ID%.x > .d._ngcontent-%ID%::before` |
| `@media (max-width: 600px) { .a {…} }` | `@media (max-width:600px){.a._ngcontent-%ID%{…}}` |
| `@keyframes girar { from {…} to {…} }` | `@keyframes girar{from{…}to{…}}` (sem shim) |

O `_LegacyShadowTransformer` (`SH:575-691`) só com a opção `use_legacy_style_encapsulation`
(padrão `false`, `FL:114`): remove `::content`/`::shadow`, não escopa depois de `:host`,
aplica `polyfill-next-selector` e `polyfill-unscoped-rule`.

- **Rust:** coberto — `shadow_css.rs` (porte de `_ShadowTransformer`), `css.rs:shim`, com o
  porte do `csslib` em `csslib/`. O transformador legado **não** está portado
  (`shadow_css.rs:11-12`).

---

## 7. `deferred`

### R7.1 — Não há suporte a carregamento adiado no `v1`
Nenhum arquivo de `compiler/` nem de `source_gen/` trata `deferred`; o único uso no
`ngcompiler` é o *outliner* (`.outline.template.dart`), que pula imports `deferred as` ao
copiar os imports do usuário (`angular_compiler/outliner.dart:111-112`). O `ngast` não tem
tratamento especial para `@deferred`.

- `@deferred` num elemento do template é uma anotação comum: `AstTemplateParser.visitElement`
  só reage a `skipOnPushValidation` e `skipSchemaValidationFor` (`ATP:265-275`) e
  `parseI18nMetadata` ignora nomes que não casam `i18n…` (`i18n/metadata.dart:37-41`). A
  saída é **a mesma sem a anotação** (derivado).
- Um componente importado com `import '…' deferred as p;` no código do usuário é tratado
  como qualquer outro: o `moduleUrl` vem da biblioteca que declara a classe (`UR:8-24`) e o
  `.template.dart` a importa **sem** `deferred` (derivado).

- **Rust:** o porte **recusa** `@deferred` (qualquer anotação que não seja `@i18n…`,
  `V:metadados_i18n` 377-398, `Motivo::I18n`) — conservador: o arquivo fica com o
  `build_runner`, sem bytes errados.

---

## Lacunas do porte

Em ordem de prioridade (bytes errados primeiro, depois recusas que deixam arquivos para o
`build_runner`).

1. **`styleUrls` com `..` ou `./` gera import errado.** `V:uri_do_estilo` concatena
   `dir/url` sem normalizar (`package:p/src/x/../a.css.shim.dart`); o oficial normaliza via
   `AssetId.resolve` (`AS:18-24`) e escreve `package:p/src/a.css.shim.dart`. O arquivo existe
   (`L` 1741-1749 só confere a existência), então sai **gerado e diferente**. Normalizar os
   segmentos como `resolver_url` já faz.
2. **Componentes em arquivos `part`.** `L:achar` só olha a unidade definidora; o oficial
   visita as partes (R2.1). Uma biblioteca com componente numa parte sai como trivial ou sem
   aquele componente. Verificar se o motor recusa esse caso; se não, é saída errada.
3. **`@import 'package:<mesmo pacote>/…'` numa folha** vira caminho relativo em
   `L:gerar_folha`; o oficial escreve o `package:` verbatim (R6.5).
4. **`@import` repetido na mesma folha** ganha `import0`, `import1`… no Rust; o oficial
   reaproveita o prefixo (tabela por `moduleUrl`, R3.1) — `import0.styles` duas vezes e uma
   linha de import só.
5. **`styleUrls` com URL `package:`** (`'package:p/src/a.css'`): `uri_do_estilo` monta
   `package:p/<dir>/package:…`; hoje a checagem de existência (`L` 1741-1749) recusa antes,
   mas a forma não é suportada.
6. **Componentes fora de `lib/`** (`test/`, `web/`) com `styleUrls`: recusados
   (`uri_do_estilo` exige `lib/`). O oficial usa `asset:` e import relativo (R3.3, R6.2).
7. **`@import` dentro de `styles: [...]`**: recusado; o oficial deixa o texto como está e o
   passa pelo shim (R6.1).
8. **`use_legacy_style_encapsulation`**: `_LegacyShadowTransformer` não portado; a opção,
   se ligada no `build.yaml`, mudaria todas as folhas.
9. **Chave da tabela de imports por caminho impresso** (R3.1/R3.2): diverge do oficial só
   quando dois `moduleUrl` imprimem o mesmo caminho (tratado caso a caso por `q_chave`) ou
   para `asset:ngdart/lib/angular.dart`. Sem caso conhecido fora do `MultiToken`.
10. **Formatação sem formatador geral**: as regras de R5 (vírgula final, condicional com
    bloco, coleções aninhadas, linhas em branco) estão embutidas em cadeias fixas de `V`/`X`.
    Forma nova de expressão que combine essas regras (ex. coleção aninhada em coleção, lista
    posicional com argumento que quebra — hoje recusada em `X` 803-805) precisa de regra
    nova; R5.4 (coleção aninhada força quebra) não tem caso no oráculo.
11. **`@deferred`** e outras anotações de elemento que o oficial ignora: recusadas
    (conservador, R7.1).
