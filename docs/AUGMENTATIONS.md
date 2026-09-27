# Augmentations: a cadeia e a fusão no `elements`

Contrato do passo P7 do plano "Dart 3.7–3.13 e macros", **implementado** na
rodada de macros (2026-09-23; placar em §5). Fonte normativa:
`references/dart-language/working/augmentations/feature-specification.md`
**v1.46** (a de `working/augmentation-libraries` é só um redirecionamento) e
`accepted/future-releases/parts-with-imports`. Oráculos (D5), um por forma:

* **forma 3.6** — `import augment 'x.dart';` na biblioteca e
  `augment library 'y.dart';` no arquivo de augmentation: o CFE **3.6.2** com
  `--enable-experiment=macros` (é a forma em que ele aplica a saída das
  macros; `augmentations` sozinho não liga nada no 3.6.2);
* **forma atual** — `part of 'y.dart';` com imports próprios e declarações
  `augment` em qualquer unidade: o CFE **3.13.4** com
  `--enable-experiment=augmentations,enhanced-parts`.

Sem `augmented()` nem embrulho de corpo (D9; a spec o removeu na 1.35 e o CFE
3.6.2 o recusa). Sem a flag experimental, nenhum SDK oficial aceita
augmentation: o estilo clássico (`json_serializable`) é o que serve aí
(docs/MACROS-COMPATIBILIDADE.md).

## 1. Onde entra

* **Parser** (`crates/frontend/src/parser/declarations.rs`):
  * o modificador `augment` em declarações de topo e membros
    (`Decl::augment`, `Member::augment`). `augment` é identificador embutido
    só onde o recurso existe: com `augmentations` ou `macros` ligados é
    modificador antes de outra palavra; sem eles, só as formas que não podem
    ser outra coisa (`augment class`, `augment mixin`…) são lidas como
    modificador, com o diagnóstico de recurso desligado — código 3.6 que usa
    `augment` como nome não muda de sentido;
  * `import augment 'uri';` (`DirectiveKind::ImportAugment`) e
    `augment library 'uri';` (`DirectiveKind::AugmentLibrary`), exigindo
    `macros`;
  * `macro class` (`ClassModifiers::macro_`), exigindo `macros`;
  * diretivas em *part* já eram aceitas (o parser não impõe ordem).
* **Carregador** (`crates/elements/src/load.rs`): `import augment` carrega a
  unidade como `UnitRole::Augmentation` da mesma biblioteca (confere o
  `augment library` de volta, como `part of`); os imports de qualquer unidade
  entram no escopo da biblioteca. `Library::units` fica na **pré-ordem da
  árvore de partes** (as unidades incluídas por uma entram logo depois dela,
  na ordem das diretivas), que é a ordem de aplicação da spec
  (`:603-715`); o SDK fica na ordem de sempre.
* **Outline** (`crates/elements/src/augmentation.rs`, chamado da fase 1 do
  `outline.rs`): a generalização do mecanismo de `@patch` do SDK (o CFE reusou
  a mesma infraestrutura). Uma declaração `augment` não introduz nome: liga-se
  à declaração de mesmo nome que veio antes na ordem de aplicação.
  `Program::augmentacoes` guarda, por classe, as declarações `augment class`
  na ordem; `Program::membros_da_classe` devolve os membros da cadeia inteira,
  cada um com a sua unidade.
* Texto gerado por macro entra na mesma cadeia, como unidade de
  augmentation em memória (`elements/src/gerado.rs`), sem arquivo
  (docs/MACROS-PROTOCOLO.md §4).

## 2. Regras da cadeia

* **Invariante** (a mesma dos patches): a entrada do mapa de membros — e do
  `declared` da biblioteca — aponta sempre para o elemento **efetivo**, a
  última declaração completa da cadeia (ou a introdutória, se nenhuma é
  completa); todo outro elemento da cadeia tem `patched_by` apontando para
  ele. `types` infere todos; os emissores emitem só o efetivo.
* **Declaração incompleta** (`:541-602`): sem corpo e não `external`.
  Divergência seguida (D9, CFE 3.6.2): `external` pode ser completada por uma
  augmentation — é exatamente o que a saída das macros do 3.6.2 faz
  (`external C.fromJson(…)` na fase 2, `augment C.fromJson(…) : …` na 3).
  Completar uma declaração que já tem corpo Dart é erro; uma augmentation
  incompleta (só metadata) não troca o efetivo.
* **Fusão de membros** (`:754-836`): `augment class`/`augment mixin`
  acrescentam membros novos (um construtor novo tira o construtor padrão
  sintético) e cláusulas `implements`/`with`/`on`/`extends`, e aumentam
  membros existentes, na ordem do texto (um membro novo pode ser aumentado
  logo depois, no mesmo corpo).
* **Funções de topo**: `augment` completa uma `external` (ou incompleta).
* **Erros**, com a posição na unidade da augmentation: `augment` sem
  declaração introdutória antes; tipo de declaração diferente; membro novo
  com nome já declarado; `extends` repetido; aplicação de mixin aumentada.
* **Enum** (2026-09-27): `augment enum` segue a regra da classe (cadeia,
  `with`/`implements`, membros novos e `augment` de membros); os valores
  novos entram em `enum_constants` depois dos da declaração, na ordem de
  aplicação, e o `emit_js` os emite (e o `values`) nessa ordem. Valor com
  nome repetido é erro. Oráculo só estático: o `dart analyze` 3.13.4
  (`augmentations`, `enhanced-parts`) aceita e tipa como nós
  (`types/tests/bodies.rs`, `augmentation_de_enum_acrescenta_valores_e_membros`);
  nenhum CFE executa (ver §4).
* **Ainda não suportado** (diagnóstico claro, não silêncio): augmentation de
  `extension` e `extension type` (a augmentation omite o `on` e a
  representação, e o nosso parser ainda os exige; o analyzer 3.13.4 aceita
  as duas formas sem eles, e o CFE 3.13.4 recusa `on`/representação na
  augmentation e quebra sem o `on` — `SimpleIdentifier` não é
  `NominalParameters?`; o CFE 3.6.2 com `macros` quebra nos dois), de enum
  com construtor primário, de
  variável de topo e de campo (o analyzer 3.13.4 relata
  `declaration_already_complete` para `augment int x = 5;` sobre variável
  com inicializador; o CFE 3.13.4 quebra com `LateInitializationError` na
  augmentation de campo).
* **Escopo** (2026-09-27): cada unidade resolve nomes no seu próprio escopo
  de imports (`parts-with-imports`, "Scopes"). Quando alguma parte da
  biblioteca tem imports, `Library::escopos_de_unidade` guarda o escopo de
  cada unidade, calculado pela árvore de partes (`Library::pais`): os imports
  de um arquivo valem nele e nas partes dele e escondem os do arquivo que o
  incluiu (nome solto e prefixo); as declarações da biblioteca escondem todo
  import; o `dart:core` implícito fica por último. Sem imports em partes,
  nada muda. `Program::lookup_na_unidade`/`lookup_prefixed_na_unidade`/
  `prefixos_na_unidade` são usados por `types`, pela resolução de
  supertipos (`elements`), pelo `emit_js` e pelo `analise`; o LSP, o
  gerador ngdart e o hospedeiro de macros ainda consultam o escopo da
  biblioteca (a união dos imports de todas as unidades), assim como a lista
  de extensões aplicáveis de `types`. Aceite: `406_partes_imports_313`.
* **Assinatura** (2026-09-27, `:837-924`): os tipos omitidos numa
  augmentation de função, método, getter ou setter vêm da declaração
  aumentada (posicionais pela posição, nomeados pelo nome, o retorno)
  — `types::resolve`, `herdar_assinatura`. Função genérica (de qualquer
  lado) e construtor ainda ficam com os tipos como escritos. Igual ao
  analyzer 3.13.4 (`augmentation_herda_tipos_omitidos`); o CFE 3.13.4 não
  herda (§4).

## 3. O que os emissores veem

Nada novo: uma classe comum com os membros finais. O `emit_js` percorre
`Program::membros_da_classe` (cada membro com a sua unidade) e pula o membro
cujo elemento não é o efetivo; funções de topo com `patched_by` não são
emitidas. O DDC 3.13.4 faz o mesmo (`Usuario.fromJson = function(json) { … }`
na classe fundida). O `emit_native` não foi tocado nesta rodada (é de outro
agente): ele já segue `patched_by` para os patches do SDK.

## 4. Divergências dos oráculos (medidas)

* O CFE 3.6.2 **e** o 3.13.4 ignoram `implements`/`with` acrescentados por
  `augment class` no `is` e na busca de membros (`c is Descrevivel` imprime
  `false`; `with` não traz os membros do mixin). Seguimos a spec (as cláusulas
  entram); nenhum programa do corpus depende disso.
* O CFE 3.6.2 roda os inicializadores de campo das augmentations **antes**
  dos da classe e em largura (`4 1 3 2` no `403`); a spec e o 3.13.4 usam a
  pré-ordem (`1 2 3`, no `405`). Seguimos a spec.
* O CFE 3.6.2 permite substituir o corpo de função já completa sob o
  experimento `macros`, tanto por `FunctionDefinitionMacro` quanto por
  `import augment` manual. O DartForge segue esse comportamento para
  bibliotecas de augmentation com `macros` (`416`–`417`); fora desse
  experimento preserva a regra que proíbe a substituição.
* O CFE 3.6.2 não aceita `augment factory`, nem o texto fundido que ele
  mesmo mostra para a saída do `@JsonCodable` como **uma** biblioteca de
  augmentation (declarações da fase 2 e definições da fase 3 juntas): ele
  aplica cada fase numa biblioteca própria. O 3.13.4 aceita o texto fundido
  como *part*.
* O 3.13.4 não liga `augment mixin` à declaração de origem.
* Tipos omitidos numa augmentation (`augment f(x) => …` depois de
  `String f(int x);`): o analyzer 3.13.4 os herda da declaração aumentada
  (e relata `int a = C().f(3)` como `invalid_assignment`); o CFE 3.13.4 os
  deixa `dynamic`, compila e falha em execução. Seguimos a spec e o analyzer.
* Augmentation de enum: o analyzer 3.13.4 aceita; o CFE 3.13.4 e o 3.6.2
  (`macros`) recusam ("'E' is already declared", "conflicts with an existing
  class"). A execução segue a spec, sem oráculo.

## 5. Aceite

`corpus/macros/40*` (job `macros` do `pesado.yml`, os dois oráculos):

| programa | forma | o que confere |
|---|---|---|
| `400_aug_membros` | 3.6 | corpo para `external` (método, getter, setter, operador), método e campo novos, função de topo completada |
| `401_aug_construtores` | 3.6 | construtor `external` completado com lista de inicialização, redirecionamento completado, construtor e fábrica novos |
| `402_aug_json_saida_cfe` | 3.6 | o texto que o CFE 3.6.2 gera para o `@JsonCodable`, nas duas bibliotecas em que ele o aplica |
| `403_aug_ordem` | 3.6 | augmentation que importa augmentation; mixin aumentado por duas bibliotecas |
| `404_aug_sem_introdutoria_erro` | 3.6 | negativo: `augment` sem declaração antes |
| `405_aug_partes_313` | atual | *part* com imports, parte dentro de parte (ordem dos inicializadores), o texto fundido do `@JsonCodable` numa *part* |
| `406_partes_imports_313` | atual | escopo de imports por unidade: parte, neta sem imports, neta com import próprio, prefixo repetido, declaração da biblioteca contra import da parte |

Todos batem com o oráculo no perfil de desenvolvimento (conferido à mão com
o binário desta rodada; o CI roda desenvolvimento e produção). Testes do
parser: `parser::declarations::tests::augmentations` (4).
