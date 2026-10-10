# Keepalive de campo Ref — AOT Windows, 2026-10-10

Fonte `663fe2ab77c64c9d87d85fe5e7b07d491986d62e`. Prova local, não resultado
da CI: `arc_keepalive_campos` usa a preparação ARC do módulo para transformar
a leitura emprestada em um token Owned antes de substituir a aresta do campo.
O driver normal compila o LLVM e liga o runtime Rust; não há fixture C++.

| Variante | ARC O0 | tracing O0 | ARC O2 | tracing O2 |
| --- | --- | --- | --- | --- |
| normal | 0, stdout `1` | 0, stdout `1` | 0, stdout `1` | 0, stdout `1` |
| sem retenção | trap | trap | trap | trap |
| sem liberação final | trap | trap | trap | trap |

Os oito traps retornaram `-1073741795` (`STATUS_ILLEGAL_INSTRUCTION`).
Os controles alteram o IR depois da conferência, omitindo respectivamente a
retenção produzida ou a liberação do retorno no harness. Uma execução normal
precisa passar antes de interpretar seus controles negativos.

O harness aloca instância de 40 campos e caixa Mint de `i64::MAX`, grava o
filho no campo 39 e libera o owner original do filho. A observação inicial
é `1`: bloco vivo, sem owner do código. A função HIR lê o campo, retém antes
de substituí-lo por null e devolve o mesmo handle Owned. Depois de a função
sair, o harness libera o receiver e coleta: receiver `0`, filho `3` (bloco
vivo com owner do código). Libera o retorno e coleta novamente: filho `0`.
Não há alocação entre morte e observação para evitar reutilização do endereço.

Executado com `DARTFORGE_ARC_CONFERIR=1`, `DARTFORGE_ARC_BERCARIO=0` e
`DARTFORGE_GC_STRESS=1`. Comandos, revisões, versões das ferramentas, códigos
de saída e SHA-256 dos 50 arquivos brutos estão em `evidencia.json`.
`.gitattributes` conserva os bytes originais, inclusive stdout e arquivos vazios.
Executáveis não foram incluídos.

Limites: HIR explícita e harness LLVM, não compilação de um programa Dart
pela classificação semântica completa. Tracing usa o mesmo corpo com tokens
explícitos para conferir compatibilidade runtime; seu fluxo normal continua
sem a preparação ARC. Não prova chamadas pending, origem/layout no lowering,
recarga, suspensão, Finalizable, cleanup geral de laços ou desempenho ARC≥A0.
A CI da fonte e a execução Unix da nova matriz ainda estavam pendentes ao
registrar esta evidência. Não substituir esses limites por uma aprovação geral.
