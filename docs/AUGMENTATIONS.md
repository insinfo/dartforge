# Augmentations: a cadeia e a fusão no `elements`

Contrato (P0) do passo P7 do plano "Dart 3.7–3.13 e macros"; não implementado
nesta rodada. Fonte normativa:
`references/dart-language/working/augmentations/feature-specification.md`
**v1.46** (a de `working/augmentation-libraries` é só um redirecionamento) e
`accepted/future-releases/parts-with-imports`. Oráculo: o 3.13.4 com
`--enable-experiment=augmentations,enhanced-parts` (D5). Sem `augmented()`
nem embrulho de corpo (D9; a spec o removeu na 1.35 e o CFE 3.6.2 o recusa).

## 1. Onde entra

* **Parser**: o modificador `augment` em declarações de topo e membros, e
  diretivas (`import`, `export`, `part`) dentro de *part* quando
  `enhanced-parts` está ligado; `part of 'uri'` continua sendo o cabeçalho.
  Tudo atrás de `Parser::exigir(Feature::Augmentations | EnhancedParts, …)`
  ([`VERSOES-LINGUAGEM.md`](VERSOES-LINGUAGEM.md) §2).
* **`elements`**: o `UnitRole::Patch` + `patched_by` já fazem o que o CFE fazia
  para augmentations (o CFE reusou a infraestrutura de patch). Generaliza-se
  para `UnitRole::Augmentation` e uma **cadeia de augmentation por elemento**:
  a declaração introdutória seguida das augmentations **na ordem da spec**
  (`:603-753`: pós-ordem das *parts*, e dentro de cada unidade a ordem do
  texto).
* Texto gerado por macro entra na mesma cadeia, como unidade de
  augmentation em memória (`elements/src/gerado.rs`), sem arquivo.

## 2. Regras da cadeia

* **Declaração incompleta** (`:541-602`): `external` ou abstrata, completada
  por uma augmentation posterior; é erro completar duas vezes ou deixar
  incompleta uma declaração que precisa de corpo.
* **Fusão de membros** (`:754-836`): uma `augment class` acrescenta membros e
  cláusulas (`implements`, `with`) e pode aumentar membros existentes; os
  membros acrescentados entram na ordem em que aparecem na cadeia.
* **Casamento de assinatura** (`:837-924`): a augmentation repete a
  assinatura (tipos podem ser omitidos e são herdados da declaração
  anterior); divergência é erro.
* **Variáveis**: augmentation de variável pode trocar o inicializador;
  getter/setter implícitos seguem a declaração introdutória.
* **Escopo**: cada unidade resolve nomes no seu próprio escopo de imports
  (`parts-with-imports`), mas os membros fundidos pertencem à biblioteca.

## 3. O que os emissores veem

Nada novo: depois da fusão, `emit_js`, `emit_native` e `mundo` veem uma classe
comum com os membros finais (a mesma estratégia dos patches do SDK). O DDC
3.13.4 faz o mesmo (`Usuario.fromJson = function(json) { … }` na classe
fundida).

## 4. Aceite

`corpus/augmentations` e `front_end/testcases/{augmentations, enhanced_parts}`
(8 + 82) contra o 3.13.4; o texto do `@JsonCodable` gerado pelo 3.6.2,
colocado como *part* escrita à mão, roda igual (`scratchpad/aug2` do plano).
