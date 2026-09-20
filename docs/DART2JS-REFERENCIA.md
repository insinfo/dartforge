# Dart2js como referência: níveis e bugs

Não tratar a saída de uma única versão/flag como oráculo absoluto. Uma divergência deve ser
investigada contra especificação, testes, regras do backend web e outros compiladores.

Referências verificadas:
- https://github.com/dart-lang/sdk/issues/63337
- https://github.com/dart-lang/sdk/issues/59790
- https://dart.dev/tools/dart-compile#javascript-js

Na issue #63337, mantenedores confirmaram reprodução de um problema em Dart 3.6.2 e o
identificaram como problema já corrigido (#59790). O comentário também explica que -O0
serve à depuração do compilador e historicamente não funciona bem. Não generalizar isso
para afirmar que -O2/-O3 eliminam todos os bugs: a própria issue envolve níveis otimizados.

Política do projeto:
- Baseline principal de produção: -O2.
- Matriz de investigação: -O0, -O1, -O2 e -O3, identificados separadamente.
- -O1 já aplica otimizações padrão; -O0 é o nível que desativa muitas otimizações.
- -O3 omite checks implícitos de tipos: não é equivalente a -O2 quanto às garantias.
- Resultado diferente é rotulado divergence e retorna falha no runner; nunca é ignorado.
- Regressões reais devem ter caso mínimo, versão/flags e link da issue quando existente.
- Alvo corrigido pelo proprietário para Dart 3.6.2, igual ao executável instalado. Novos relatórios devem registrar targetVersionMatched=true.

Execução exploratória de 20/09/2026: cinco fixtures x quatro níveis = 20 comparações.
19 concordaram. numeric_edges em -O0 imprimiu -0.0 nos dois primeiros valores enquanto
DartForge imprimiu 0. Em -O1/-O2/-O3, esse caso imprimiu 0, concordando com DartForge.
A VM também confirmou zero inteiro nos exemplos de investigação; produtos grandes da VM
não servem de oráculo web por terem diferenças de representação.

Essa divergência fica registrada em docs/conformance-optimization-matrix.json. Não prova
correção completa do DartForge nem é apagada por adotar -O2 como baseline principal.
