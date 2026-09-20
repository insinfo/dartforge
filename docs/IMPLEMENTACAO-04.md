# Quarto incremento — Dart 3.6.2

Implementado por três subagentes com integração, revisão cruzada e testes diferenciais.
O projeto permanece experimental: não compila aplicações completas Dart/ngdart.

## Entregas

- Tipos anuláveis, ??, !, promoções por fluxo, atribuições e junções conservadoras.
- Classes nominais, campos, métodos, herança única e verificação de sobrescritas.
- Ordem de inicialização derivada/base corrigida com confirmação no Dart 3.6.2.
- Grafo de imports relativos com ciclos, cache de caminhos e diagnósticos por arquivo.
- Modo --optimize para simplificação de constantes após validação semântica.
- Benchmarks reproduzíveis por fase e por processo, com mediana/p95 e dados brutos.
- Documentação /// e //! em português nas novas APIs.

O grafo de imports ainda não fornece linkagem/resolução de símbolos entre bibliotecas.
Extensions exigem membros resolvidos na HIR e continuam no roteiro. Construtores explícitos,
super, generics, interfaces, runtime completo e otimizações globais ainda não existem.

## Validação

- 110 testes Rust passaram, incluindo os testes Node e exemplos executáveis.
- 18 casos passaram contra Dart 3.6.2/dart2js -O2 no modo direto.
- Os mesmos 18 casos passaram com a avaliação de constantes ativada.
- Formatação, Clippy, build release e Rustdoc sem avisos passaram.
- O CI executa Windows/Linux e os dois modos da comparação diferencial.

Relatórios: [direto](conformance-increment-04.json) e
[constantes](conformance-increment-04-opt.json). A divergência histórica conhecida de -O0
não foi usada como baseline nem escondida; a política de referência permanece em vigor.

## Primeira melhoria guiada por medição

A expressão binária copiava todo o estado do analisador mesmo sem promoção de fluxo.
Agora a cópia ocorre somente nos operadores && e ||. O lado direito dos demais operadores
usa o estado imutável existente; os testes semânticos de null safety continuam passando.

No corpus sintético de 100 funções, com 15 amostras de 40 iterações e aquecimento:

| Fase | Mediana antes | Mediana depois |
|---|---:|---:|
| Análise semântica | 1,818 ms | 0,791 ms |
| Pipeline direto | 2,095 ms | 1,103 ms |

Dados brutos: [antes](benchmarks/increment-04-before.json) e
[depois](benchmarks/increment-04-after.json). As medições foram feitas sequencialmente,
sem Cargo/subagentes executando em paralelo, mas sem isolamento do sistema operacional.
São observações locais de médias por lote, não garantia geral nem p95 por chamada.
Ambos os relatórios identificam árvore de trabalho modificada, hardware e toolchain.
A diferença de código medida é exclusivamente clone condicional em expression/Binary;
reintroduzir o clone incondicional reproduz a variante anterior no código deste incremento.

Não há afirmação de fator de aceleração sobre DDC/dart2js. A medição por processos novos
não é equivalente a sessões incrementais DDC, e a saída DDC ainda não é executada.

## Medição exploratória por processos novos

5 amostras após 1 aquecimento, corpus de 100 funções alcançáveis:

| Compilador/modo | Mediana | p95 |
|---|---:|---:|
| DartForge direto | 35,36 ms | 45,27 ms |
| DartForge constantes | 27,67 ms | 237,63 ms |
| DDC | 413,08 ms | 433,93 ms |
| dart2js -O2 | 1.514,49 ms | 1.706,63 ms |

[Relatório completo](benchmarks/increment-04-process.json). O p95 com 5 amostras é o
máximo observado; a dispersão do modo constantes impede concluir que ele compila mais
rápido que o modo direto. Inclui startup, disco e invocação PowerShell. Não há equivalência
de recursos/otimizações e não é uma sessão incremental DDC; portanto esta tabela não
sustenta a meta geral de ser várias vezes mais rápido. Saídas DartForge e dart2js foram
executadas e comparadas com a soma esperada; DDC foi somente compilado.
