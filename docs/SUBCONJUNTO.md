# Subconjunto implementado

Alvo: Dart 3.6.2, igual ao SDK instalado e ao configurado no CI.

## Suporte atual

- Uma entrada `void main()` sem parâmetros.
- Funções de nível superior com retorno explícito `int`, `String`, `bool` ou `void`.
- Parâmetros posicionais obrigatórios e tipados; chamadas antecipadas e recursivas.
- `return` com valor ou vazio, conforme o tipo; `if/else` com blocos obrigatórios.
- Declarações locais inicializadas `var`/`final`/`int`/`String`/`bool`, atribuições e escopos.
- Literais inteiros i32, strings Unicode, booleanos e `print`.
- Operadores `+ - * == != < <= > >= && ||` e unários `- !`; parênteses.
- Comentários de linha e de bloco aninhados.

Validação: nomes, escopos, duplicatas, uso antes da declaração, autorreferência,
mutabilidade, tipos, aridade, argumentos, retorno e condições `bool`. Funções não `void`
precisam de retorno comprovável em todos os caminhos; a análise conservadora pode
rejeitar condições constantes ou retornos em laços que Dart aceitaria.
Parâmetros são mutáveis e podem ser sombreados por locais do corpo.

## Laços e atualizações

`while`, `do { ... } while (...);` e `for` clássico exigem blocos com chaves. As cláusulas
inicialização, condição e atualização do `for` são opcionais, inclusive em `for (;;) {}`.
A inicialização aceita uma declaração inicializada, atribuição, atualização ou chamada;
a atualização aceita atribuição, atualização ou chamada. Não há listas de cláusulas
separadas por vírgulas nem declarações na atualização.

`break` e `continue` atuam sobre o laço mais próximo, sem rótulos, e são rejeitados fora
de laços. `continue` no `for` executa a atualização antes do próximo teste; no `do/while`,
reavalia a condição. A variável declarada no cabeçalho do `for` tem escopo no próprio
laço. Locais declarados no corpo não ficam visíveis na condição ou na atualização.

`++` e `--` prefixados ou pós-fixados e as atribuições compostas `+=`, `-=` e `*=` são
aceitos somente sobre identificadores, como instruções ou cláusulas de `for`. Não
produzem valores utilizáveis em expressões: `print(i++)` e `var x = ++i` são rejeitados.
As atualizações respeitam tipos e `final`; `+=` também permite concatenar duas strings.

```dart
void main() {
  var total = 0;
  for (var i = 0; i < 4; i++) {
    if (i == 1) { continue; }
    total += i;
  }
  print(total);
}
```

## Strings

Strings de uma linha usam aspas simples ou duplas. São aceitos escapes de aspas,
barra invertida e dólar, controles `\b`, `\f`, `\n`, `\r`, `\t`, `\v`, escapes
hexadecimais `\xHH`, Unicode `\uHHHH` e `\u{H...}` com um a seis dígitos hexadecimais.
Pares UTF-16 válidos, como `\uD83D\uDE00`, são decodificados; surrogates isolados são
rejeitados explicitamente. Escapes desconhecidos perdem a barra invertida, conforme
Dart; por exemplo, `\q` produz `q`. Para NUL, use `\x00` ou `\u0000`.

Strings raw `r'...'` e `r"..."` preservam barras invertidas e dólares literalmente.
Não há interpolação, strings de aspas triplas ou strings multilinha. Um escape `\n`
dentro de uma string de uma linha pode produzir uma quebra de linha no valor.

## Tipos anuláveis e objetos

Veja os contratos de [null safety e fluxo](NULL-SAFETY.md) e [classes/herança](CLASSES.md).
A ferramenta também carrega um [grafo de imports](IMPORTS.md), ainda separado da compilação.

## Ainda não suportado

Imports compilados em conjunto, closures/funções locais, funções como valores, parâmetros nomeados ou
opcionais, overloads, `double`, generics, extensions, async, `for-in`, `switch`,
rótulos, interpolação, strings triplas, surrogates isolados, coleções, Dart `const`,
source maps e bibliotecas padrão completas. Atualizações compostas de campos, acesso a índices e atribuições
usadas como expressões também não são suportadas.
Identificadores são ASCII; alguns nomes contextuais válidos em Dart ficam reservados
neste parser. Os casos não implementados retornam erro explícito.

A HIR ainda transporta a AST estrutural; classes possuem IDs locais, mas membros e
símbolos ainda não têm resolução persistida na IR.
Números usam JavaScript Number; limitar literais a i32 não limita a faixa dos resultados.
Não há garantia de equivalência integral à VM nem implementação completa de `int` Dart.
Casos numéricos de borda são testes de regressão, não prova geral de conformidade.

O parser limita aninhamento a 64 níveis e expressões a 128 nós, inclusive argumentos de
chamadas. Esses são limites temporários do protótipo, não da linguagem Dart.
Não existe limite estático para recursão ou iterações executadas pelo programa gerado.

Não há otimização global, cache incremental, servidor LSP, servidor HTTP ou compilador
ngdart completo. Também não há benchmark válido de vantagem sobre DDC/dart2js.

## Documentação e contribuições

[CONTRIBUTING.md](../CONTRIBUTING.md) define as verificações e a documentação Rust em
português: `//!` para módulos, `///` para funções, contratos, limitações e exemplos
executáveis nas APIs públicas. A aceitação de sintaxe deve vir acompanhada de testes
que preservem efeitos, ordem de avaliação e escopos.
