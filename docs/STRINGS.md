# Literais de string — contrato, semântica e limites

Este documento descreve o que o DartForge aceita em literais de string do
subconjunto Dart 3.6.2 e o que continua fora. Toda regra registrada aqui foi
conferida contra o **Dart SDK 3.6.2 instalado na máquina de desenvolvimento**
(`dart --version` → `3.6.2 (stable)`), usado como oráculo real: os programas de
`tests/conformance/modules/strings25/` foram executados com `dart run` e a saída
gravada em `main.stdout` é o texto que o JavaScript emitido precisa reproduzir
byte a byte. O teste `strings_match_dart_in_all_modes`
(`crates/compiler/tests/strings.rs`) faz essa comparação no Node.

## O que passou a ser aceito

| Forma | Exemplo | Resultado |
| --- | --- | --- |
| Interpolação simples | `'$nome'` | valor de `nome` convertido com `toString` |
| Interpolação de expressão | `'${a + b}'` | expressão inteira avaliada e convertida |
| Nome seguido de ponto | `'$obj.campo'` | interpola **só** `obj`; `.campo` é texto |
| Cifrão escapado | `'\$nome'` | `$nome` literal |
| String raw | `r'$nome'` | `$nome` literal |
| Aspas triplas | `'''...'''`, `"""..."""` | corpo multilinha, com interpolação |
| Aspas triplas raw | `r'''...'''` | corpo multilinha sem escapes nem interpolação |
| Literais adjacentes | `'a' 'b' '$c'` | concatenação em tempo de compilação |

### `$nome` versus `${expressão}`

A distinção é léxica, não sintática. O lexer reconhece `$` seguido de um
identificador como uma interpolação que termina no último caractere do
identificador — é a produção `IDENTIFIER_NO_DOLLAR` do Dart. Duas consequências
diretas, ambas conferidas no SDK:

- `'$obj.campo'` interpola `obj` e concatena `.campo` como texto. Com
  `var nome = 'Ana'`, `'$nome.nome'` imprime `Ana.nome`.
- `'$a$b'` são **duas** interpolações; com `a = 1` e `b = 2` imprime `12`.

Para interpolar o membro é preciso escrever `'${obj.campo}'`.

O identificador precisa ser um identificador de verdade: `'$this'`, `'$true'` e
`'$null'` são erros no Dart e também aqui.

### Aspas triplas

Três regras léxicas, todas verificadas com `dart run`:

1. **A primeira linha em branco some.** Se o que vem depois da abertura for
   apenas espaços e tabulações seguidos de um terminador de linha, esse trecho
   inteiro é descartado, terminador incluído. `'''\nabc'''` e `'''   \nabc'''`
   valem `abc`; `'''  x\nabc'''` vale `  x\nabc`, porque a primeira linha tem
   conteúdo. Só a primeira linha é afetada: `'''\n\nabc'''` vale `\nabc`.
2. **A regra é sobre a fonte, não sobre o valor.** Um `\n` escrito como escape
   não conta: `'''\tX'''` (com `\t` e `\n` escapados) preserva tudo.
3. **CR e CRLF viram LF.** Uma string tripla escrita num arquivo com fim de
   linha do Windows produz exatamente as mesmas unidades de código que a mesma
   string num arquivo com LF. Vale também para `r'''...'''`.

O literal termina na primeira ocorrência das três aspas. Uma aspa isolada ou um
par de aspas dentro do corpo é texto comum: `'''a'b"c'''` vale `a'b"c`.

### Literais adjacentes

`'a' 'b'` concatena no parser, antes de qualquer análise. Qualquer combinação de
formas participa, inclusive raw e triplas: `'a' 'b' r'$c' '''d'''` vale
`ab$cd`. Se alguma parte tiver interpolação, o resultado é uma interpolação
única cujas partes literais vizinhas já vêm juntas.

## Semântica da conversão

O resultado de uma interpolação é sempre `String`.

- Cada expressão interpolada é avaliada **exatamente uma vez**, na ordem em que
  foi escrita. A emissão é uma única concatenação JavaScript da esquerda para a
  direita, sem variável temporária e sem reavaliação.
- `null` interpolado vira `"null"`, como em Dart.
- Uma expressão que já é `String` não passa por conversão nenhuma: o emissor usa
  o valor direto, então `'$nome'` custa o mesmo que `nome`.
- Os demais tipos passam por `$dartforgeString`, definida no runtime
  `crates/codegen/src/core.js`. Ela **não** usa `String(x)` do JavaScript para
  inteiros: o `int` do subconjunto tem 32 bits com sinal e o zero negativo do
  JavaScript precisa ser impresso como `0`, que é o único texto que o Dart
  produz para zero. Coleções e records são delegados a `$dartforgeFormat`, o
  mesmo formatador que `print` usa, de modo que `'$lista'` e `print(lista)`
  nunca divergem.
- Um programa que interpola apenas escalares recebe somente `$dartforgeString`,
  recortada do runtime entre marcadores, e não o runtime de coleções inteiro.

Nunca é emitido `[object Object]`: os tipos que produziriam esse texto são
rejeitados antes da emissão.

## Limites que permanecem

| Limite | Diagnóstico exato |
| --- | --- |
| Instância de classe, enum, função, `Future`, `Duration` ou `Timer` interpolados | `String interpolation requires unsupported toString semantics for this value` |
| `$` sem identificador nem `{` em seguida | `a '$' inside a string must be followed by an identifier or '{'` |
| Palavra reservada depois de `$` | `'<nome>' can't be used as an identifier because it's a keyword` |
| Mais de uma expressão dentro de `${...}` | `string interpolation accepts a single expression` |
| Quebra de linha em string de aspas simples | `single-quoted strings must end on the same line` |
| `${` sem a chave correspondente até o fim do arquivo | `unterminated string interpolation` |
| Literal sem a aspa de fechamento | `unterminated string` |
| Interpolação em expressão constante (`const s = 'a${1}b'`) | `Const expression: calls, getters and this expression are unsupported` |

O primeiro limite é o mais relevante e é deliberado. O subconjunto ainda não tem
o protocolo `toString` do Dart: `print` de um objeto já era rejeitado por
`Printing objects or function values requires unsupported toString semantics`, e
a interpolação usa exatamente o mesmo conjunto de tipos representáveis
(`printable_type`, em `crates/semantic/src/collections.rs`). Emitir
`Instance of 'C'` sem o protocolo por trás produziria texto que deixa de bater
com o Dart assim que a classe declarar seu próprio `toString`. Enquanto o
protocolo não existir, a interpolação rejeita com span na expressão ofensora.

Os escapes Unicode, os pares substitutos UTF-16 e a rejeição de surrogates
isolados continuam exatamente como antes; a interpolação apenas passou a aplicar
a mesma decodificação a cada trecho literal.

## Desempenho

As decisões de representação seguem `docs/DESEMPENHO.md`:

- O lexer continua em **passagem única**. Um literal suspenso por `${` vai para
  uma pilha com a aspa, a forma tripla e a profundidade de chaves; a chave
  correspondente devolve o controle ao mesmo literal, sem reler nada. Chaves de
  mapas e literais aninhados dentro da interpolação contam na profundidade e não
  encerram o literal cedo demais.
- Nenhum trecho literal é copiado pelo lexer: todos são `&'a str` emprestados do
  texto original. Uma string sem interpolação, sem escapes e sem CR continua sem
  alocar nada — o caminho `ExprKind::String(&str)` é exatamente o de antes.
- A alocação só aparece quando é inevitável: escapes a decodificar,
  normalização de CR em string tripla, ou junção de literais adjacentes.
- A `Vec<StringPart>` de uma interpolação omite trechos vazios e funde literais
  vizinhos, então não há posições inúteis para o emissor percorrer.

Medição do benchmark `cargo bench -p dartforge-compiler --bench incremental`,
cenário `frio`, antes e depois desta mudança: MEDICAO_PENDENTE. O corpus do
benchmark não usa interpolação, então o número mede o custo que a pilha nova do
lexer impõe a todo programa, mesmo sem nenhuma string interpolada.

## Backends

A interpolação é suportada pelo backend JavaScript. O backend LLVM/AOT a rejeita
com `interpolação de strings não é suportada` até existir uma conversão nativa
de `toString`.
