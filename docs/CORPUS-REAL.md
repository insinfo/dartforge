# Aferição contra código Dart de produção

Uma sondagem de construções isoladas diz **quais recursos existem**. Ela não diz
se o compilador aguenta um arquivo real, nem — o que importa mais — **quais
recursos o código de verdade usa e com que frequência**. Este documento registra
a aferição contra um pacote real do pub.dev e o que ela mudou nas prioridades.

## Corpus

`pdf` 3.13.1, do pub.dev: **179 arquivos Dart, 1,1 MB**. Baixado por
`scripts/corpus.sh pdf`, para `references/pub/`, que o Git ignora — o corpus é
grande e cada pacote tem licença própria.

O teste `crates/compiler/tests/corpus_real.rs` roda o front-end sobre cada
arquivo e agrupa os diagnósticos por **forma**, descartando nomes e números
concretos: sem isso, `Unknown identifier 'a'` e `Unknown identifier 'b'`
contariam como lacunas diferentes e o ranking não significaria nada.

O teste **não exige sucesso**. Um compilador em construção falha em código de
produção, e isso não é notícia. O que ele afirma é que todo arquivo **termina**,
com sucesso ou com diagnóstico: sem pânico e sem laço infinito. Essa é a
propriedade que pode regredir sem ninguém notar.

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
scripts/corpus.sh pdf
cargo test -p dartforge-compiler --test corpus_real -- --ignored --nocapture
```

Outros pacotes podem ser acrescentados como argumentos. Um corpus com pacotes de
domínios diferentes — rede, serialização, interface — daria um retrato menos
enviesado do que um único pacote de geração de PDF.
