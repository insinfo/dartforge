# Keepalive de campo Ref — CI Windows/Linux/macOS, 2026-10-10

Fonte `ed5897e95836972de0c9091bd23a0ad5bd4662f0`,
[CI 38034929874](https://github.com/insinfo/dartforge/actions/runs/38034929874).
As etapas de campos e upload terminaram com sucesso nas três plataformas.
Ao preservar esta evidência, os jobs Windows/Linux estavam completos com
sucesso e macOS continuava em execução. Isso não é aprovação do workflow
inteiro nem das mudanças locais posteriores.

Cada plataforma executou ARC/tracing × O0/O2: quatro provas positivas e
oito controles negativos. As doze saídas positivas são exatamente `1` LF,
com código 0. Os 24 controles não têm stdout e retornam trap: Windows
`-1073741795` (instrução ilegal), Linux `132` (SIGILL) e macOS `133`
(SIGTRAP). A matriz antiga aceitava qualquer erro dos controles; a inspeção
dos artefatos confirma os traps concretos, sem ampliar essa regra antiga.

A HIR lê um campo Ref, retém antes de substituir a aresta e retorna Owned.
O harness LLVM usa instância de 40 campos e Mint mortal; verifica identidade,
owner do retorno, morte do receiver e morte do filho após o release final.
O gerador foi compilado em release com `llvm-embutido`; O0/O2 selecionam a
otimização do executável gerado. Runtime e compilador são Rust.

`evidencia.json` conserva fonte, etapas, jobs, IDs/digests dos artefatos,
códigos de saída e SHA-256 dos 144 arquivos brutos. Os arquivos preservam
bytes originais, inclusive CRLF dos códigos de saída Windows. Não há
executáveis ou logs de geração nesses artefatos da matriz antiga.

Limites: HIR explícita, não lowering de fonte Dart. Não prova pending,
cleanup de laços, unwind, suspensão, Finalizable ou desempenho ARC≥A0.
Tracing compara tokens explícitos somente no harness; preparação ARC no
produto continua exclusiva de `--memoria=arc`. O script compartilhado novo
não foi executado por esta revisão.
