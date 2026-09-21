# Grafo de imports relativos

A crate `dartforge-packages` carrega arquivos UTF-8 e constrói um grafo de dependências.
Esta crate cuida do carregamento; **não resolve nomes nem compila múltiplas unidades**.
A compilação conjunta já é oferecida por `dartforge-linker` e pelo CLI `compile`,
conforme [a documentação de bibliotecas](MODULES.md).

A API `load(&Path) -> Result<SourceGraph, GraphError>` entrega `units` e `entry`. Cada
unidade contém caminho canônico, fonte original e imports. Cada import registra URI,
ID do destino e span da diretiva completa, em bytes da fonte importadora. `GraphError`
fornece caminho, mensagem e span opcional; erros de acesso à entrada não têm span.

## Subconjunto

São aceitas diretivas `import 'relativo.dart';` ou strings raw equivalentes antes de
qualquer declaração. Comentários e espaços podem separar diretivas. O caminho usa `/`,
é relativo ao diretório do importador e pode conter `..`. Não há confinamento ao
diretório da entrada: o carregador acessa as dependências relativas declaradas.

Caminhos canônicos são deduplicados, inclusive aliases como `./a.dart` e `sub/../a.dart`.
A ordem de descoberta em largura e a ordem textual dos imports determinam IDs estáveis.
Imports repetidos mantêm arestas distintas para a mesma unidade. Ciclos são aceitos pelo
carregador; isso não significa que namespaces ou inicialização cíclica estejam resolvidos.

Não são suportados caminhos absolutos, escapes em URIs, query, fragmento e `deferred`.
`package:`, `dart:core`, `show`, `hide`, `export`, prefixos `as`, imports condicionais,
metadados antes da primeira diretiva, as três formas de `library`, `part` e `part of`
são suportados; as partes seguem
[a documentação de partes](PARTS.md). Diretivas import depois de declarações são
rejeitadas. Todos os casos fora do subconjunto produzem erro explícito, sem tentar
resolver parcialmente a diretiva.

O lexer atual tokeniza o arquivo inteiro. Assim, a descoberta de imports ainda exige
que as fontes pertençam ao subconjunto léxico do protótipo, mesmo que os corpos não
sejam analisados semanticamente nesta etapa. Não existe cache persistente: a deduplicação
vale por chamada de `load`. A resolução de `package:` segue package_config v2, descrita
em [PACKAGES.md](PACKAGES.md); não há leitura de `pubspec.yaml`.

## A diretiva `library` e metadados de diretiva

As **três** formas de `library` são válidas no Dart atual, e todas são aceitas:

```dart
library a.b.c;   // nome pontuado
library foo;     // nome simples
library;         // sem nome
```

Nomear biblioteca é sintaxe legada das primeiras versões do Dart e **continua
aceita**; o que caiu em desuso foi o costume, a ponto de o linter oficial ter a
regra `unnecessary_library_name`. A forma sem nome é a recomendada hoje, e existe
justamente para receber metadados:

```dart
@TestOn('browser')
library;
```

Só a forma **com nome** pode ser alvo de um `part of nome;`. Uma `library;` sem nome
não registra nome nenhum, e um `part of app.exemplo;` apontado para ela é recusado
com `part of app.exemplo não corresponde à biblioteca …` — não porque a forma seja
inferior, mas porque não há texto com que casar.

O nome pontuado é **gramática própria de diretiva**: uma sequência de identificadores
separados por ponto, que produz uma única string. Ele não passa pelo caminho de
expressão e não vira `Member`/`Identifier` aninhados, então `a`, `b` e `c` nunca são
procurados como declarações. O mesmo vale para `part of a.b.c;`, que usa a mesma
gramática. As recusas nomeiam a forma aceita em vez de só constatar a falha:

    diretiva library exige ; ou um nome pontuado como a.b.c
    part of exige uma URI entre aspas ou um nome pontuado como a.b.c

Metadados podem preceder a primeira diretiva, e o código publicado usa isso —
`@TestOn('vm')`, `@Skip()`, `@Tags(['golden'])`, `@Timeout(Duration(seconds: 60))`.
Eles são consumidos junto do prefixo de diretivas, com os argumentos saltados por
parênteses equilibrados, então atravessam aninhamento sem serem interpretados. Como
os tokens ficam antes do fim do prefixo, o front-end nunca os recebe: metadados de
biblioteca não dirigem geração de código. Metadados antes de `import`/`export`/`part`
continuam fora do subconjunto.

Fora de ordem, a diretiva é recusada dizendo qual é a ordem:

    diretiva library fora do prefixo de diretivas: a ordem é library, import/export, part, declarações

### Por que isso valia mais do que parecia

Medido nos quatro pacotes de [CORPUS-REAL.md](CORPUS-REAL.md) (367 arquivos `.dart`,
`pdf`, `intl`, `collection` e `http`):

| Forma | Arquivos | Antes |
| --- | --- | --- |
| `library;` sem nome | 47 | recusada por exigir nome |
| metadados antes de `library` | 26 | `library` caía fora do prefixo |
| `library nome;` ou `library a.b.c;` | 0 | já aceita |
| `part of nome;` | 0 | já aceita |

Os dois primeiros somam **73 arquivos**, e nenhum deles era recurso ausente: a forma
sem nome já existia na gramática, só estava sendo **exigida** com nome. A lição está
no texto do diagnóstico antigo — "diretiva exige nome de biblioteca pontuado" —, que
constatava a falha sem dizer a forma aceita e por isso não se distinguia de uma lacuna
de implementação.

## Prefixos de import

`import 'package:image/image.dart' as im;` **não acrescenta nada** ao namespace sem
qualificação: o namespace da biblioteca importada passa a ser alcançável **apenas**
por `im.nome`. É a forma que o código de produção usa, e por isso ela bloqueava
diretamente [a aferição contra pacotes reais](CORPUS-REAL.md).

O que resolve por prefixo:

| Escrita | Nó resultante |
| --- | --- |
| `p.funcao(...)` | chamada da função de topo importada |
| `p.Tipo` em anotação | classe importada, resolvida já no parser |
| `p.Classe(...)` | construtor sem nome |
| `p.Classe.nomeada(...)` | construtor nomeado ou método estático |
| `p.Classe.CONSTANTE` | campo estático ou valor de enum |
| `p.funcao` sem chamada | referência à função de topo importada |

Regras que o prefixo **não** afasta:

* Privacidade por biblioteca: `_nome` não atravessa a fronteira, nem com prefixo. O
  nome nunca entra no namespace alcançável, então a recusa é a mesma de um nome
  inexistente: `nome não exportado pela biblioteca importada com prefixo p: _oculto`.
* `show` e `hide` compõem com o prefixo e filtram o namespace antes dele.
* `export` transitivo atravessa o prefixo: `p.nome` alcança o que a biblioteca
  importada reexporta, com os combinadores de cada aresta já aplicados.
* Um prefixo é **local ao arquivo que o declara**, mas uma `part` compartilha os
  imports do declarante, então o prefixo vale nela também.
* Dois imports com o mesmo prefixo compõem **um único** namespace. O mesmo nome
  vindo de duas declarações diferentes sob o mesmo prefixo é ambíguo:
  `import ambíguo: p.valor; use show/hide para desambiguar`, no intervalo da segunda
  diretiva.
* Um nome com prefixo **não** é alcançável sem ele, e por isso nunca é ambíguo com um
  nome importado sem prefixo: `import 'a.dart'; import 'b.dart' as p;` deixa `valor()`
  e `p.valor()` como duas coisas distintas, ainda que `a.dart` e `b.dart` declarem o
  mesmo nome.

Um prefixo **não é um identificador comum**. `p` sozinho, sem `.`, é recusado com
`prefixo de import p exige um nome: use p.nome`, no intervalo do identificador e não
no da expressão que o contém. Um nome de topo com o mesmo texto de um prefixo é
recusado já na diretiva, com
`prefixo de import p colide com um nome visível nesta biblioteca`.

`export '...' as p;` não existe na gramática de Dart, e o motivo é de significado:
um reexport reemite nomes no namespace de quem exporta e não cria namespace
qualificado nenhum. A recusa diz isso, no intervalo do prefixo escrito:

    export não aceita prefixo as: um reexport não cria namespace qualificado

O reexport transitivo **atravessa** o prefixo de quem importa, e é aí que os dois
recursos se combinam: `import 'fachada.dart' as f;` alcança por `f.nome` tudo o que
`fachada.dart` reexporta, direta ou indiretamente, com os combinadores de cada aresta
aplicados no caminho.

`deferred as` tem diagnóstico próprio, no intervalo da palavra `deferred`, porque ele
muda o **momento** do carregamento e não apenas o escopo do nome — tratá-lo como
prefixo comum aceitaria um programa com semântica de carregamento diferente da escrita:

    carregamento diferido não é suportado: remova deferred, a biblioteca importada é ligada estaticamente

`import 'dart:core' as c;` continua recusado: prefixo em biblioteca SDK só vale para
`dart:ffi`, com `prefixo em biblioteca SDK ainda não suportado fora de dart:ffi`.

### Limite: variáveis de topo

Uma variável de topo não entra no índice de declarações da unidade — ela vira campo
estático de uma classe sintética por unidade — então ela não é exportada e
`p.VARIAVEL` é recusado como nome não exportado. Uma constante alcançável por prefixo
precisa estar numa classe: `p.Limites.maximo`. O limite não é do prefixo: ele vale
igual para um import sem prefixo.

## Bibliotecas e pacotes ausentes

Falhar com erro genérico de URI obriga quem usa o compilador a investigar o que o
compilador já sabe. Os dois casos mais comuns nomeiam o que falta.

`dart:typed_data`, `dart:math`, `dart:convert` e `dart:collection` **não existem**
neste subconjunto — implementá-las é outro trabalho. O diagnóstico nomeia a
biblioteca pedida, no intervalo da diretiva, e diz quais são reconhecidas:

    biblioteca dart:math ainda não existe neste subconjunto; só dart:core, dart:async e dart:ffi são reconhecidas

Um pacote ausente nomeia o pacote **e** o caminho consultado, porque sem o caminho não
se distingue "falta a dependência" de "a configuração lida é outra":

    pacote vector_math não encontrado na configuração de pacotes consultada: <raiz>/.dart_tool/package_config.json

Sem nenhum `package_config.json` no caminho ascendente, o diagnóstico diz onde
procurou:

    pacote vector_math não encontrado: nenhuma configuração de pacotes foi encontrada a partir de <raiz>/.dart_tool/package_config.json

## Custo da resolução por prefixo

A resolução de namespaces está no caminho quente ([DESEMPENHO.md](DESEMPENHO.md)), e o
projeto mede **alocações por compilação** porque o tempo tem ruído. Um `HashMap` por
prefixo por arquivo alocaria uma tabela por diretiva; em vez disso os nomes alcançáveis
por prefixo ficam num `Vec` ordenado por `(prefixo, nome)`, consultado por busca
binária, e os prefixos declarados num segundo `Vec` ordenado. As classes alcançáveis por
prefixo vão ao parser como fatia ordenada do mesmo formato.

Um programa **sem prefixo nenhum** não paga nada: os dois vetores externos ficam vazios,
sem alocação, e tanto `prefixed_expression` quanto `prefixed_class` voltam no primeiro
`if`. Medido no corpus de `cargo bench -p dartforge-compiler --bench incremental`, que
não usa prefixos, as alocações por compilação não se movem.

## Verificação

    cargo test -p dartforge-packages
    cargo test -p dartforge-compiler --test prefixos

Os testes usam diretórios temporários exclusivos e cobrem dependências aninhadas,
diamantes, aliases, ciclos, ordem dos IDs, spans por arquivo, arquivos ausentes,
diretivas não suportadas e uma cadeia carregada iterativamente. As três formas de
`library`, os metadados antes da primeira diretiva e as recusas que nomeiam a forma
aceita estão em `library_directive_accepts_named_dotted_and_unnamed_forms` e
`dotted_name_diagnostics_name_the_accepted_forms`.

`crates/compiler/tests/prefixos.rs` fecha o contrato desta página de ponta a ponta,
com mensagem **e** intervalo exatos em cada recusa, e um caso marcado
`#[ignore = "requer Node.js no PATH"]` que executa o módulo emitido para provar que os
namespaces continuam separados na saída, e não apenas na análise.
