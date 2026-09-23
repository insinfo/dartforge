# Quinto incremento — bibliotecas, extensions e escalabilidade

Alvo: Dart 3.6.2. Três subagentes trabalharam em parsing, ligação/análise e emissão,
com revisão cruzada e integração. Código e APIs novas mantêm documentação em português.

## Implementação

- Compilação real de imports relativos, com namespace por biblioteca, nomes privados,
  funções/classes entre unidades, ciclos declarativos e main independente nas dependências.
- Diagnósticos de dependências preservam arquivo e offsets UTF-8 da fonte original.
- Ambiguidades de imports têm diagnóstico determinístico e não dependem da ordem de HashMap.
- Extensions nomeadas sobre primitivos e classes em uma unidade sem imports.
- Análise escolhe alvos estáticos, respeita precedência de membros e especificidade;
  HIR transporta a resolução até o codegen. Não modifica protótipos JavaScript.
- Correção de fluxo: atribuição de Child a Base mantém Base como tipo estático;
  em Base?, a promoção por atribuição retira null, sem estreitar para Child.
- Correção da otimização de ?? para preservar o span da chamada selecionada e sua resolução.
- Ordenação de classes substituída por algoritmo topológico iterativo O(V+E) esperado,
  com IDs esparsos, bases antecipadas e diagnóstico de contrato para ciclos/IDs inválidos.

Extensions importadas, package:/dart:, prefixos, exports, parts, construtores explícitos,
generics e cache incremental continuam pendentes. O suporte atual não é Dart completo.

## Validação

- 23 programas passaram contra Dart 3.6.2/dart2js -O2 no modo direto.
- Os mesmos 23 passaram com avaliação de constantes: 46 comparações diferenciais.
- Inclui três projetos com múltiplos arquivos, ciclos, funções privadas homônimas,
  herança entre bibliotecas e dispatch de extensions distinto de dispatch virtual.
- 145 testes Rust passaram, incluindo entradas negativas, escopos, privacidade, spans,
  comportamento Node e exemplos de documentação. Formatação, Clippy, Rustdoc sem avisos
  e build release também passaram.

Relatórios: [modo direto](dados/conformance-increment-05.json) e
[constantes](dados/conformance-increment-05-opt.json). CI: Windows/Linux, Rustdoc sem avisos e
ambos os modos da suíte diferencial.

## Gargalo medido e corrigido

O algoritmo anterior percorria repetidamente todas as classes para encontrar bases já
emitidas. Uma cadeia na ordem reversa exigia número quadrático de verificações. O novo
algoritmo constrói arestas e graus de entrada, usa fila e não recorre pela profundidade.

| AST pronta | Mediana antes | Mediana depois |
|---|---:|---:|
| 100 classes | 99,043 µs | 9,627 µs |
| 1.000 classes | 9,002 ms | 0,195 ms |

[Dados antes](dados/benchmarks/increment-05-classes-before.json) e
[dados depois](dados/benchmarks/increment-05-classes-after.json), incluindo amostras e ambiente.
As duas variantes emitiram o mesmo número de bytes no corpus. São medições locais de
emissão de AST sintética, com 15 lotes; não incluem parsing/análise nem demonstram
aceleração geral ou superioridade sobre DDC/dart2js. A mudança isolada foi medida antes
da integração das extensions desta rodada.

O novo benchmark libraries separa carregamento, compilação do grafo já carregado e
pipeline com disco, sem cache incremental. Todos os símbolos públicos do corpus são
alcançáveis. Ele estabelece uma referência para otimizações futuras de bibliotecas.

## Referência de 101 arquivos

Corpus de 100 bibliotecas mais entrada, 11.799 bytes, 15 amostras e filesystem aquecido:

| Operação | Mediana | p95 |
|---|---:|---:|
| Carregamento do grafo | 12,829 ms | 19,720 ms |
| Compilar grafo já carregado | 0,758 ms | 2,053 ms |
| Pipeline com leitura | 11,830 ms | 21,132 ms |
| Pipeline com leitura e constantes | 12,492 ms | 19,142 ms |

[Dados brutos](dados/benchmarks/increment-05-libraries.json). Cada fase foi medida separadamente;
cache e ruído do sistema explicam inclusive a mediana do carregamento superar a do pipeline.
As medianas não são parcelas somáveis. Não houve outros processos Cargo/subagentes durante
a medição. O custo observado de leitura justifica investigar cache por unidade e invalidação
antes de micro-otimizar ainda mais a emissão; não constitui uma medição de build incremental.
