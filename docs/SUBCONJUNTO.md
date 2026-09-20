# Subconjunto implementado

Alvo: Dart 3.6.2, igual ao SDK instalado e ao configurado no CI.

## Suporte atual

- Uma entrada void main() sem parâmetros.
- Funções top-level com retorno explicitamente int, String, bool ou void.
- Parâmetros posicionais obrigatórios e tipados; chamadas antecipadas e recursivas.
- return com valor ou vazio, conforme o tipo; if/else com blocos obrigatórios.
- Declarações locais inicializadas var/final/int/String/bool, atribuições e escopos.
- Literais inteiros i32, strings simples Unicode, booleanos e print.
- Operadores + - * == != < <= > >= && || e unários - !; parênteses.
- Comentários de linha e de bloco aninhados.

Validação: nomes, escopos, duplicatas, uso antes da declaração, auto-referência,
mutabilidade, tipos, aridade, argumentos, retorno e condição bool. Funções não void
precisam de retorno comprovável em todos os caminhos; análise é conservadora e pode
rejeitar condições constantes cuja semântica completa de Dart permitiria inferir retorno.
Parâmetros são mutáveis e podem ser sombreados por locais do corpo.

## Ainda não suportado

Classes, imports, closures/funções locais, funções como valores, parâmetros nomeados ou
opcionais, overloads, métodos, double, null, generics, async, loops, interpolação, escapes,
coleções, Dart const, source maps e bibliotecas padrão completas.
Identificadores são ASCII; alguns nomes contextuais válidos em Dart ficam reservados
neste parser. Os casos não implementados retornam erro explícito.

A HIR ainda transporta a AST estrutural; não tem IDs de símbolo nem tipos resolvidos.
Números usam JavaScript Number; limitar literais a i32 não limita a faixa dos resultados.
Não há garantia de equivalência integral à VM nem implementação completa de int Dart.
Casos numéricos de borda são testes de regressão, não prova geral de conformidade.

O parser limita aninhamento a 64 níveis e expressões a 128 nós, inclusive argumentos de
chamadas. Esses são limites temporários do protótipo, não da linguagem Dart.
Não existe limite estático para recursão executada pelo programa gerado.

Não há otimização global, cache incremental, servidor LSP, servidor HTTP ou compilador
ngdart completo. Também não há benchmark válido de vantagem sobre DDC/dart2js.
