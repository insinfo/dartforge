# Macros e a toolchain oficial: compatibilidade por materialização

Decisão de 2026-09-23 (proprietário + coordenação, medida nesta rodada). Complementa
[`MACROS-PROTOCOLO.md`](MACROS-PROTOCOLO.md) e [`AUGMENTATIONS.md`](AUGMENTATIONS.md).

## 1. O que foi medido

* O SDK **3.13.4** aceita augmentations com
  `--enable-experiment=augmentations,enhanced-parts` (a forma `part of` com
  imports próprios) e recusa sem a flag. As **macros** saíram do SDK oficial
  novo: `macro class` não é mais linguagem lá.
* O SDK **3.6.2** executa macros com `--enable-experiment=macros` e aceita
  augmentation escrita à mão só na forma de **biblioteca de augmentation**
  (`import augment 'x.dart';` + `augment library 'y.dart';`); augmentation
  na mesma biblioteca ele recusa, com qualquer flag.
* Sem flag experimental nenhum SDK oficial aceita augmentation: aí só o
  estilo clássico (`json_serializable`, código gerado em `part` sem
  `augment`) serve.

## 2. O princípio

A saída de uma macro é uma **augmentation em Dart comum**, gravável em
arquivo — o `.g.dart` das macros —, na forma que cada versão do SDK aceita.
Dois caminhos produzem **o mesmo texto, byte a byte**:

* **no DartForge**, o executor nativo (D4) roda a macro em memória, no
  hospedeiro (`crates/macros_host`), e o texto vive em
  `elements/src/gerado.rs` como `<biblioteca>.macro.dart`;
* **na toolchain oficial**, um *builder* do `build_runner` roda **a mesma
  macro** na VM oficial, com a **nossa** API de macros (`pacotes/macros`, Dart
  puro, sem nada que só o nosso compilador tenha), e grava o arquivo.

O critério é conferido no CI sobre o mesmo corpus (`corpus/macros/*/esperado`):
o texto do hospedeiro tem de ser o que o CFE 3.6.2 gera, e o que a VM produz
pela nossa API também.

## 3. A materialização (implementada)

```
dartforge macros <entrada.dart> --materializar [--forma 3.6|atual] --dart <dart> [--api pacotes/macros]
```

* roda o hospedeiro inteiro (detecção, as três fases com o programa
  recarregado, a montagem) com o executor de materialização — a nossa API
  numa VM Dart (`macros_host::vm`, o mesmo protocolo `dfexec/1` do executor
  nativo; o *bootstrap* e o `package_config.json` com `macros` apontando para
  a nossa API são gerados em `.dart_tool/dartforge/macros/`);
* grava, para cada biblioteca com aplicação, `x.macro.dart` ao lado de
  `x.dart`, e diz a diretiva que a biblioteca precisa;
* sem `--materializar`, imprime o texto.

Formato do arquivo por versão de SDK (o corpo é o mesmo; só muda o
cabeçalho):

| SDK oficial | cabeçalho de `x.macro.dart` | diretiva em `x.dart` | flag |
|---|---|---|---|
| 3.6.2 | `augment library 'x.dart';` | `import augment 'x.macro.dart';` | `--enable-experiment=macros` |
| 3.13.4 | `part of 'x.dart';` | `part 'x.macro.dart';` | `--enable-experiment=augmentations,enhanced-parts` |

Medido: `dartforge macros --materializar` no `corpus/macros/410_json_codable`
grava `lib/modelos.macro.dart` com o corpo **idêntico** ao texto do CFE 3.6.2
(só o cabeçalho troca a URI `package:` pelo nome do arquivo); com
`import augment 'modelos.macro.dart';` na biblioteca, o `dartforge
compile-js` compila o programa **sem executor de macros** e a saída no Node é
a mesma da VM 3.6.2 rodando as macros (7 linhas, byte a byte).

Uma biblioteca que já inclui o seu `x.macro.dart` (do disco) **não roda as
macros de novo** no DartForge: o arquivo materializado vale como o `.g.dart`
de um builder (`sessao::ja_materializada`).

Limites registrados:

* O texto fundido de todas as fases **numa** biblioteca de augmentation
  (declaração `external` da fase 2 e o `augment` da fase 3 no mesmo corpo)
  roda no 3.13.4 e no DartForge, mas **não** no 3.6.2, que aplica cada fase
  numa biblioteca própria (`corpus/macros/402`). A forma 3.6 materializada
  serve ao DartForge e ao analyzer; para o CFE 3.6.2, a macro roda nele
  mesmo (o caminho oficial daquela versão).
* No 3.6.2 com a flag de macros, a aplicação **e** o arquivo materializado
  juntos declaram tudo duas vezes: materializar é para quem compila **sem** o
  executor de macros da VM.
* Um pacote de macros da 1ª geração (`macro class`, como o `package:json`
  0.20.4 publicado) não compila no 3.13.4: as anotações precisam vir de um
  pacote sem `macro` para o arquivo materializado servir lá.

## 4. O builder (em andamento)

O pacote `dartforge_macros_builder` já oferece uma primeira etapa opt-in:
o `build_runner` resolve as anotações pelo element model do analyzer e grava
`x.macro_uses.json`. Um caso derivado de `410_json_codable` identifica 4 aplicações.
A montagem dos resultados estruturados também existe em Dart
(`pacotes/macros/lib/src/executor/montagem.dart`): a reprodução dos 8
`macro.resultado` de `sessao.dfexec` produz os mesmos 3266 caracteres da
augmentation do CFE 3.6.2. Essa verificação cobre a montagem, não a
execução da macro pelo builder.
Um ensaio no mesmo isolate, usando `package:json` 0.20.4 com a API de
`pacotes/macros`, reproduz os 8 resultados da sessão CFE. O builder ainda
precisa completar o modelo semântico a partir do `Resolver` para as fases
restantes.
O adaptador do builder já serializa classes simples e campos no formato de
`macro.executar`; o modelo de `Endereco` bate estruturalmente com o pedido
gravado do CFE, depois de trocar apenas a URI do pacote do fixture.
Com esse modelo, `@JsonCodable` executa a fase de declarações no mesmo isolate
e devolve o resultado estruturado idêntico ao da sessão CFE (3 consultas ao
hospedeiro). As respostas dessas consultas ainda vêm da sessão gravada no
primeiro teste. O caso de integração resolve as 3 consultas
`resolverIdentificador` pela sessão do analyzer e reproduz a mesma fase sem
respostas gravadas. As demais consultas da fase de definições ainda precisam
de implementação antes de gerar o arquivo.

O builder aceita um registro explícito de fábricas Dart do projeto
(`macroDeclarationsBuilder`). O fixture `corpus/builders/macros_registry`
registra as três macros de `package:json` no bootstrap do `build_runner`; o resultado da fase de
declarações de `Endereco` é emitido em `modelos.macro_declarations.json` e
comparado integralmente com o `macro.resultado` do CFE. Quatro aplicações são
executadas no mesmo isolate do builder. A fábrica recebe a anotação resolvida
para poder ler argumentos; o registro distingue construtores nomeados. Esta
etapa ainda é opt-in e não materializa `.macro.dart`.
As aplicações da mesma biblioteca agora compartilham uma tabela de IDs
semânticos: classes distintas não colidem e `Map`, `String` e `Object` mantêm
o ID entre as quatro execuções. Os IDs das duas primeiras classes seguem o
oráculo CFE (1 para `Endereco`, 8 para `Usuario` neste corpus). A tabela é
pré-requisito para fundir os resultados na augmentation parcial.
O adaptador de montagem do analyzer já funde essas quatro declarações em
`modelos.macro_declarations.txt`: os quatro blocos `augment class` são
comparados byte a byte com a parte correspondente da augmentation do CFE.
O `.txt` é uma saída intermediária, pois ainda faltam a recarga do analyzer
com essas declarações e a fase de definições para formar `.macro.dart`.
O analyzer 7.3 não associa `import augment` à classe original neste ensaio;
por isso o builder lê a AST da saída parcial e serializa os métodos e
construtores gerados para `modelos.macro_definitions_model.json`. O modelo
completo de membros de `Endereco` corresponde ao pedido da fase de definições
do CFE depois de desconsiderar os IDs locais da sessão e trocar a URI do
fixture.
O hospedeiro Dart já atende `resolverIdentificador`, `resolver`,
`ehExatamente` e `declaracao`: as 14 consultas da fase de definições de
`Endereco` correspondem ao CFE depois de desconsiderar IDs de sessão.
O `build_runner` também executa a fase de definições de `Endereco` no mesmo
isolate; seu `macro.resultado` corresponde ao CFE depois da mesma
normalização. O hospedeiro responde `membros` de classes já processadas; as
quatro aplicações agora terminam a fase de definições sem diagnósticos no
fixture reduzido. Os resultados de `Endereco`, `SoSaida` e `SoEntrada`
coincidem estruturalmente com o CFE depois de remover IDs de sessão;
`Usuario` tem menos campos no fixture e requer o corpus completo para essa
comparação.
O builder também monta `modelos.macro_complete.txt` com as duas fases. Os
blocos completos de `Endereco`, `SoSaida` e `SoEntrada` são comparados byte a
byte com o CFE. A saída continua `.txt` até a validação integral de `Usuario`
com o corpus completo e da URI da biblioteca materializada.

Um pacote `dartforge_macros_builder` para o `build_runner`:

* `build.yaml` com um `Builder` por biblioteca (`.dart` → `.macro.dart`),
  `auto_apply: dependents`, `build_to: source`;
* o builder usa o `Resolver` do `package:build` para achar as anotações que
  resolvem para uma `macro class` e monta o modelo com o mesmo JSON do
  `dfexec/1` (`MACROS-PROTOCOLO.md` §5), a partir do *element model* do
  analyzer;
* executa a macro **no mesmo isolate**, com a nossa API (`pacotes/macros`:
  `Modelo`, `Introspector`, `executarFase`) e um `Hospedeiro` que responde do
  analyzer — sem processo, sem VM de macros do SDK;
* monta o texto com a mesma montagem do hospedeiro (a ser portada do
  `macros_host::montagem` para Dart, com o mesmo teste byte a byte contra o
  CFE) e grava na forma da versão do SDK do projeto.

O teste de aceite do builder é o mesmo corpus: o `.macro.dart` gravado pelo
`build_runner` tem de ser igual ao que `dartforge macros --materializar`
grava, e ao `esperado/` do CFE.
