# Aferição contra código Dart de produção

Uma sondagem de construções isoladas diz **quais recursos existem**. Ela não diz
se o compilador aguenta um arquivo real, nem — o que importa mais — **quais
recursos o código de verdade usa e com que frequência**. Este documento registra
a aferição contra pacotes reais do pub.dev e o que ela mudou nas prioridades.

A primeira versão desta aferição mediu a coisa errada: ela rodava o front-end
arquivo a arquivo, **sem resolver imports**, e chamava o resultado de "arquivos
que compilam". Num pacote de verdade, cada arquivo menciona nomes declarados nos
outros, então o número media o recorte da medição, não o compilador. O documento
anterior já registrava a suspeita — 73 das 80 ocorrências da linha mais alta da
tabela eram tipos de outro arquivo — e concluía que "uma correção real pode não
mover o total de aceitos". Um número com essa propriedade não serve para
priorizar nada. O que segue é a medição refeita.

## Corpus

Quatro pacotes do pub.dev, de domínios diferentes, porque um corpus de um único
pacote de geração de PDF mede o estilo daquele pacote e não a linguagem:

| Pacote | Versão | Domínio | `.dart` | em `lib/` | pontos de entrada com `main` |
| --- | --- | --- | --- | --- | --- |
| `pdf` | 3.13.1 | geração de documentos | 179 | 136 | 1 em `example/`, 41 em `test/` |
| `intl` | 0.20.3 | internacionalização | 91 | 47 | 1 em `example/`, 31 em `test/`, 1 em `tool/` |
| `collection` | 1.19.1 | estruturas de dados | 49 | 29 | 20 em `test/`, e **nenhum** `example/` |
| `http` | 1.6.0 | rede | 48 | 27 | 2 em `example/`, 16 em `test/` |

Baixados por `scripts/corpus.sh --com-dependencias` para `references/pub/`, que o
Git ignora — o corpus é grande e cada pacote tem licença própria. As
**dependências transitivas** (22 pacotes: `xml`, `vector_math`, `image`,
`petitparser`, `meta`, `path`, `async`…) também vão para o disco, mas não são
medidas: elas existem para que `package:` resolva.

Baixar as dependências custou pouco — um comando e alguns megabytes — e é a
única forma de compilar pelo grafo sem que a medição se transforme num relatório
de downloads ausentes. A decisão foi baixá-las e não medi-las.

O teste **não exige sucesso**. Um compilador em construção falha em código de
produção, e isso não é notícia. O que ele afirma é que toda unidade medida
**termina**, com sucesso ou com diagnóstico: sem pânico e sem laço infinito. Essa
é a propriedade que pode regredir sem ninguém notar, e a asserção é uma igualdade
— aceitos + diagnósticos classificados + arquivos não-UTF-8 tem de fechar com o
número de unidades. Ela é verificada por ponto de entrada no modo grafo e por
arquivo no modo unidade, que é o modo que cobre **todo** arquivo do pacote,
inclusive os que nenhum programa alcança.

## O alvo é o projeto que consome a biblioteca, não a biblioteca

Uma biblioteca Dart **não declara `main`** — e nem o SDK do Dart compila uma
biblioteca isolada, pelo mesmo motivo. Compilar `lib/**` como se fosse programa
pede algo que não existe. A rodada anterior contornava isso contando
`entrada exige void main()` como sucesso, o que media a chegada ao ligador e não
a compilação de nada; `expected void main() entrypoint` aparecia na lista de
lacunas, onde nunca deveria ter estado.

O que se compila de um pacote é o **projeto que o consome**: os arquivos de
`example/`, `bin/`, `test/` ou `tool/` que declaram `main` e importam a biblioteca
por `package:`. Deles o grafo puxa `lib/` pelos imports, e a biblioteca é
exercitada como um consumidor real a exercita — o que não é alcançável não entra
na conta, que é o comportamento correto e também o mais honesto. Um arquivo é
ponto de entrada porque **declara `main`**, não porque está numa pasta.

`main` ausente passa a ser classificado como **artefato**, com a mensagem própria
"arquivo sem main de topo tomado por ponto de entrada": se ainda aparecer, é
identificação errada da entrada, nunca recurso de linguagem que falta.

## Dois modos, porque um só mentiria

O aferidor `crates/compiler/tests/corpus_real.rs` tem dois modos, ambos
selecionáveis por `DARTFORGE_CORPUS_MODO`:

* **grafo** — cada ponto de entrada real é compilado por
  `compile_path_with_units`, com `package_config.json` resolvendo `package:`. O
  front-end roda sobre o **fechamento transitivo** do programa, então os nomes
  declarados em outros arquivos do pacote resolvem de verdade. Ele responde a
  duas perguntas que são diferentes e por isso saem em números separados:
  **quantos projetos compilam de ponta a ponta** — a métrica que diz se o
  compilador compila código de produção — e **quantos arquivos de `lib/` o grafo
  atravessa sem diagnóstico** — a que mostra a cobertura de linguagem.
  As entradas de `test/` vão em grupo próprio e nunca somadas às de `example/`:
  elas importam `package:test`, dependência de desenvolvimento que o corpus não
  baixa por padrão, e a mistura daria uma média que não descreve nenhuma das
  duas.
* **unidade** — cada arquivo é analisado isoladamente, só até a sintaxe, por
  `compile_unit_diagnostics`. Não afirma nada sobre o programa e responde a uma
  única pergunta: quanto da *sintaxe* de produção o parser cobre. Era o único
  modo até aqui, e é o que produzia números contaminados quando lido como
  "arquivos que compilam". Continua disponível porque é a única medida que
  exercita todo arquivo do pacote.

O modo grafo distingue ainda **onde** a entrada parou: na carga do grafo — uma
diretiva que o carregador recusou, e aí nenhum arquivo de `lib/` foi atravessado —
ou no front-end. Somar as duas sugeriria obstáculo de linguagem onde o obstáculo
é de biblioteca.

## Três categorias, classificadas por span

Cada diagnóstico cai em uma de três categorias, porque elas exigem ações
diferentes:

* **lacuna** — o compilador precisa implementar. Inclui nomes de `dart:core`,
  que é importada implicitamente em todo arquivo Dart: `Stopwatch` ausente é
  trabalho do compilador, não dependência que falta baixar.
* **externa** — biblioteca fora do disco: `package:` que o corpus não baixou, ou
  `dart:` que o carregador ainda não implementa (`dart:math`, `dart:typed_data`,
  `dart:convert` — hoje só `dart:core`, `dart:async` e `dart:ffi` existem).
* **artefato** — consequência do recorte da medição, não do código medido.

A categoria vem do **span** — o texto exato que o diagnóstico aponta — e de onde
aquele nome é declarado no corpus, que o teste indexa de todos os pacotes
presentes. A regra é esta ordem:

1. `main` ausente é artefato: ponto de entrada mal identificado, e o span dessa
   mensagem é `0..0`, de modo que ler o trecho apontado daria o começo do arquivo;
2. uma URI de biblioteca no span identifica a dependência sem ambiguidade;
3. um nome declarado **no próprio arquivo** é lacuna: nada externo o explica;
4. um nome declarado em outro arquivo do mesmo pacote, ou em outro pacote do
   corpus, é artefato;
5. um nome de `dart:core` ou de `dart:async` é lacuna — as duas bibliotecas já
   existem para o carregador, então o que falta ali é membro, não biblioteca;
6. um nome de biblioteca `dart:` ausente é externa;
7. um nome que não existe em arquivo nenhum do disco, num arquivo que importa
   biblioteca ausente, é externa — é a única inferência que não enxerga o nome
   declarado, e ela se apoia nas diretivas do próprio arquivo, que o relatório
   imprime como evidência;
8. o que não é nem URI nem nome procurado é lacuna do compilador.

A mensagem ainda decide se o compilador estava **procurando um nome** — só ela
sabe disso —, mas nunca decide a categoria. Era exatamente esse o erro da rodada
anterior: o desconto de artefato olhava a forma da mensagem, e por isso deixou
passar 73 tipos de outros arquivos contados como lacuna. O relatório imprime a
evidência de cada linha (o nome e onde ele foi achado), para que qualquer
classificação possa ser conferida à mão em vez de aceita por confiança.

## Resultado

**5 de 179 arquivos aceitos — 2,8%.**

| Ocorrências | Diagnóstico |
| --- | --- |
| 80 | `expected declaration delimiter` |
| 37 | `unknown superclass` |
| 23 | `expected an explicitly supported type` |
| 14 | `unsupported annotation` |
| 6 | `expected Symbol(...)` |
| 3 | `mixin on constraints are not supported yet` |
| 2 | `assert in an initializer list is not supported yet` |
| 1 cada | não reservado, expressão, nome de declaração, limite de complexidade, incremento, `late`, construtor redirecionador, estático fora de classe comum, interface desconhecida |

### Uma dessas linhas não é uma lacuna de linguagem

As 37 ocorrências de `unknown superclass` são **artefato da aferição**, não do
compilador: o teste analisa cada arquivo isoladamente, sem resolver imports,
então toda superclasse declarada em outro arquivo aparece como desconhecida.
Contar isso como lacuna inflaria a lista e desviaria o trabalho. O mesmo vale
para `unknown interface`.

Isso é uma limitação do recorte escolhido, e o recorte é deliberado: analisar
com semântica completa reprovaria todos os arquivos por nomes ausentes e
esconderia exatamente as lacunas de sintaxe que interessam aqui.

### Nem `expected an explicitly supported type` é o que parece

A linha mais alta da tabela tem a **mesma** causa, e isso só apareceu ao
classificar as 80 ocorrências uma por uma, pelo texto no span: **73 são o
artefato de unidade isolada** — `Uint8List`, `DeflateCallback`, `Matrix4`,
`XmlElement`, `TtfParser`, `Stopwatch`, `DateTime`, `PdfRect`, `Context`,
`Widget`: tipos declarados em outro arquivo do pacote ou em `dart:typed_data`,
`package:vector_math` e `package:xml`. Restam **7 lacunas reais**:

| Ocorrências | Lacuna real |
| --- | --- |
| 4 | `const`/`static const` sem anotação de tipo (`const kIndentSize = 2;`) |
| 1 | anotação em parâmetro (`@Deprecated('…') String? AFRelationship`) |
| 1 | tipo com prefixo de import (`im.Image`) |
| 1 | tipo de outro arquivo que a heurística de classificação não pegou |

A consequência prática é direta: **o topo da tabela não é onde está o trabalho**.
Tipos genéricos escritos, `late` e `typedef` já eram aceitos pelo parser antes
desta rodada, e ainda assim a linha marcava 80 — porque contava nomes que o
recorte não resolve. Ordenar por contagem de diagnóstico só orienta depois de
descontar o artefato, e o desconto precisa ser feito por **inspeção do span**,
não pela forma da mensagem.

Isso também explica por que uma correção real pode não mover o total de arquivos
aceitos: um arquivo com uma lacuna real corrigida continua reprovado pelo
primeiro nome de outro arquivo que ele mencione.

## O que o código real usa, por frequência

A contagem de diagnósticos diz onde o compilador para. Ela não diz o tamanho da
oportunidade, porque o parser para no **primeiro** erro de cada arquivo. Para
isso, a contagem de quantos arquivos usam cada construção:

| Construção | Arquivos (de 179) | Proporção |
| --- | --- | --- |
| Anotações | 116 | 65% |
| `extends` | 99 | 55% |
| Tipos genéricos escritos (`List<T>`, `Map<K,V>`) | 92 | 51% |
| `late` | 50 | 28% |
| `factory` | 30 | 17% |
| `typedef` | 11 | 6% |
| Classes genéricas (`class C<T>`) | 8 | 4% |
| `dynamic` | 5 | 3% |

### Anotações: o item mais barato da lista

Das 587 anotações no corpus, **507 são `@override`**, já suportada. As que
bloqueiam são metadados sem efeito nenhum em geração de código:

| Anotação | Ocorrências |
| --- | --- |
| `@immutable` | 33 |
| `@Deprecated` | 24 (já suportada) |
| `@protected` | 16 |
| `@mustCallSuper` | 4 |
| `@pragma` | 2 |
| `@visibleForTesting` | 1 |
| `@experimental` | 1 |

Hoje o parser rejeita qualquer anotação que não reconheça. Tolerar as que não
alteram semântica destrava 65% dos arquivos com uma mudança pequena. O cuidado
necessário: **tolerar não é ignorar em silêncio**. Uma anotação desconhecida que
o compilador pudesse precisar honrar — `@pragma` dirige o compilador — merece
diagnóstico próprio, e não o mesmo tratamento de `@immutable`.

### `dynamic` importa muito menos do que se supunha

A sondagem sintética listava `dynamic` como lacuna, e era fácil concluir que
fosse urgente. No corpus real ele aparece em **5 de 179 arquivos**. Implementar
despacho dinâmico é caro — desativa tree shaking e obriga o runtime a manter
todo método de mesmo nome — e o retorno medido é pequeno. Rejeitá-lo com um
diagnóstico honesto continua sendo a decisão certa por mais tempo do que
parecia.

## Ordem de trabalho que os números sustentam

1. **Tolerar anotações sem efeito semântico** — 65% dos arquivos, mudança
   pequena e contida.
2. **Tipos genéricos escritos** em anotações de tipo — 51%.
3. **`late`** — 28%.
4. **`factory`** com as formas que faltam, inclusive construtores
   redirecionadores — 17%.
5. **`typedef`** — 6%.
6. **Classes genéricas** — 4%, e é o item mais caro dos seis.
7. **`dynamic`** — 3%, e provavelmente nunca.

Essa ordem vem de medição, não de julgamento sobre o que "parece" importante.
Ela contradiz a ordem que a sondagem sintética sugeria, e é para isso que a
aferição contra código real existe.

## Como repetir

```sh
scripts/corpus.sh --com-dependencias          # corpus padrão: pdf http collection intl
cargo test -p dartforge-compiler --test corpus_real -- --ignored --nocapture
```

O script baixa os pacotes pedidos, segue as dependências declaradas em cada
`pubspec.yaml` e escreve dois arquivos em `references/pub/.dart_tool/`:

* `package_config.json` (formato v2, o mesmo que `crates/packages` lê), mapeando
  cada pacote baixado para o seu nome, de modo que `package:pdf/pdf.dart`
  resolva a partir de qualquer arquivo do corpus;
* `medidos.txt`, com os pacotes pedidos na linha de comando. As dependências
  baixadas por arrasto ficam no disco só para resolver `package:` e **não**
  entram no relatório: contá-las somaria milhares de arquivos alheios ao que se
  quis medir.

Qualquer conjunto de pacotes serve: `scripts/corpus.sh json_serializable` mede
só esse, sem dependências. `DARTFORGE_CORPUS_MODO=grafo|unidade|ambos` escolhe o
modo; sem a variável, o teste roda os dois.
