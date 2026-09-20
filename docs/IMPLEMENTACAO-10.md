# Incremento 10 — raízes reutilizáveis, memória e equivalência alfa

Continuação do backend Dart 3.6.2, com licença MIT e documentação Rust em português.
Os quatro clones Swift solicitados já estavam presentes nas revisões do
[manifesto](references-manifest.json); nenhum clone ou fonte de referência entra
no build. Três subagentes atuaram em LLVM, runtime e otimização, com revisão
independente do protocolo de raízes.

## GC e representação

Antes, cada avaliação de uma expressão acrescentava uma raiz ao frame. Um laço
longo retinha todas essas entradas até retornar. Agora o emissor atribui um slot
estático a cada ponto de expressão e um slot independente a cada local/parâmetro
de referência. Cada ativação reserva a quantidade completa, inicialmente null.
Laços sobrescrevem os slots; chamadas recursivas têm frames separados.

Uma cópia armazenada em um local continua protegida quando o slot temporário que
a originou é sobrescrito. Atribuir null atualiza a raiz local para zero. Referências
de argumentos aninhados, campos e resultados de `??` continuam protegidas durante
alocações; o retorno não dispara coleta entre o pop do callee e o registro no caller.
Funções puramente escalares continuam sem criar frame de GC.

O limite é por quantidade de pontos estáticos e profundidade de chamadas, não
por número de iterações. Isso não limita objetos legitimamente alcançáveis por
campos, nem implementa análise completa de vivacidade. Slots podem reter valores
até sobrescrita/retorno; limpar no fim de escopo e eliminar raízes redundantes são
melhorias futuras.

O coletor também considera bytes estimados dos objetos vivos: cabeçalho Value e
capacidades de strings/vetores de campos. O limiar inicial é 1 MiB e se adapta após
coleta; o gatilho por número de alocações permanece. Não é limite rígido de memória,
não inclui todos os metadados e não equivale ao RSS do processo.

`DARTFORGE_GC_STATS=1` habilita um JSON em stderr após retorno normal do programa,
com alocações, coletas, objetos recolhidos, slots, raízes e bytes estimados. Não
força coleta final nem muda stdout. `DARTFORGE_GC_STRESS=1` continua destinado à
validação de correção por coleta antes de toda alocação.

## Fusão de funções

`--merge-identical-functions` continua opt-in e independente de `--optimize`.
Além do caso original, agora reconhece, por exemplo:

```dart
int soma(int n1, int n2) => n1 + n2;
int soma2(int left, int right) => left + right;
```

A comparação usa IDs lexicais na chave canônica, preservando shadowing,
inicializadores e escopos de `for`. Tipos nominais, membros, operadores e destinos
globais/extension continuam significativos. Inverter operandos de uma subtração
ou chamar outra função não torna os corpos iguais. A transformação não altera
nomes de bindings na AST; remove a definição redundante e redireciona chamadas
posicionais. Limites de métodos, tear-offs e stack traces continuam documentados
no [contrato do passe](MERGE-FUNCOES.md).

## Verificação e medidas

- **225 testes** aprovados com `cargo test --locked --workspace -- --include-ignored`;
  fmt, Clippy all-targets com warnings negados, rustdoc privado e release aprovados.
- Teste nativo específico compara 300 e 6.000 iterações em O0/O2, normal/stress:
  mesmo pico de slots de raiz, stdout preservado, coleta efetiva e menos de 1.024
  slots de objetos reservados em todas as execuções. Ao retornar, zero raízes ativas.
- [Runner nativo](conformance-native-increment-10.json): **14 casos × O0/O2**,
  fusão, GC stress e estatísticas ativados, equivalentes ao Dart VM/AOT 3.6.2.
- [Runner JS](conformance-js-increment-10.json): **27 casos**, fusão ativada,
  equivalentes ao dart2js 3.6.2 O2. O novo caso `merge_bindings` também foi inspecionado:
  `soma2` e `nested2` foram removidas, mantendo resultados `42, 7, 7, -7, 9, 10`.

| Programa nativo, stress | Alocações | Recolhidos | Pico slots de raiz | Slots de objetos reservados |
| --- | ---: | ---: | ---: | ---: |
| gc_root_slots.dart | 20.095 | 20.089 | 46 | 12 |
| root_slots.dart | 20.000 | 19.983 | 34 | 19 |

O0 e O2 produziram os mesmos contadores nesses casos. Objetos ainda presentes no
relatório não significam raízes vazadas: o relatório não força coleta após o
retorno. Metadados, slots vazios e alocador continuam consumindo memória adicional.

[Benchmark da fusão](benchmark-merge-increment-10.json): 200 funções duplicadas,
15 amostras de 10 compilações; mediana 0,42658 ms sem fusão e 0,93210 ms com fusão.
A saída cai de 18.814 para 214 bytes (201 para 2 funções incluindo main).
O passe continua mais caro nesse corpus e desligado por padrão. Os resultados
não demonstram vantagem geral sobre DDC/dart2js ou Dart AOT.

Os relatórios foram coletados no working tree do incremento; o JSON nativo guarda
a revisão base e o status sujo. Tempos de execução do runner com stress e uma
amostra são registro de validação, não benchmark de produção.

## Referências consultadas

- Dart SDK tag 3.6.2: `runtime/vm/heap/heap.h`, `runtime/vm/heap/marker.h`,
  testes de escopo e sombreamento registrados nos comentários do passe.
- Swift revisão `6e75592c4025239130e370877ab0fab4006b675b`:
  `docs/SIL/ARCOptimization.md`, `GenericSpecializer.cpp` e
  `IPO/DeadFunctionElimination.cpp` como referências de arquitetura.

O protocolo de slots é próprio do DartForge; não adota ARC nem copia código Swift.
Generic specialization, escape analysis, GC geracional e compilação de aplicações
ngdart completas permanecem fora deste incremento.
