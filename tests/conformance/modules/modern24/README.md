# modern24 — recursos posteriores ao Dart 3.6.2

Cada recurso exercitado aqui entrou na linguagem depois do alvo mínimo do projeto:

| Recurso | Versão mínima do Dart |
| --- | --- |
| Variáveis e parâmetros curinga `_` | 3.7 |
| Elementos null-aware `?valor` em coleções | 3.8 |
| Atalhos de ponto `.membro` | 3.10 |
| Construtores primários `class C(...)` | 3.13 |

`main.stdout` foi conferido contra a execução do JavaScript emitido pelo DartForge
em Node.js. O SDK instalado nesta máquina é o 3.6.2 e **não** aceita esta entrada,
portanto ainda não existe oráculo oficial para este módulo: a comparação
diferencial com o SDK exige instalar uma versão que implemente os quatro recursos.
