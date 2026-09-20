# Otimização opcional de constantes

    dartforge compile entrada.dart saida.mjs --optimize

O modo padrão emite diretamente após validação. --optimize ativa o passe de constantes:
int + - * e negação com resultado dentro de i32; booleanos/comparações constantes;
concatenação de strings até 64 KiB e igualdade literal; seleção literal em ??.

A validação semântica ocorre primeiro. Um ramo não executado que contém erro de tipos ou
nome desconhecido continua sendo rejeitado. Chamadas, acesso a campos e construção de
objetos nunca são executados pelo compilador. As otimizações preservam curto-circuito,
efeitos e intervalos de origem, e percorrem funções, classes e laços.

Resultados numéricos fora de i32 continuam como expressões no JavaScript: não há
truncamento ou overflow Rust disfarçado de resultado Dart. Não há DCE, propagação de
variáveis, inlining, minificação ou otimização global ainda.

A suíte diferencial roda os modos none/constants separadamente contra dart2js -O2.
O benchmark registra o custo extra do passe, sem presumir que otimizar reduz tempo de
compilação ou que a saída tem qualidade equivalente a dart2js.
