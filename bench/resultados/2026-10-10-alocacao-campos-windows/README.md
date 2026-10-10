# Alocação e campos Owned — AOT Windows, 2026-10-10

Fontes preservadas no commit `bee4dceb830eec429353f11ff4468fa11e10a35d`.
Execução local sobre a base `b198aadd`, com alterações registradas no
manifesto. Gerador e script têm os mesmos bytes da execução. Um teste
unitário de keepalive foi acrescentado depois da matriz AOT, sem alterar
a implementação de produção; a suíte final incluiu esse teste.

| Variante | ARC O0/O2 | tracing O0/O2 |
| --- | --- | --- |
| normal | código 0, stdout `1` LF | código 0, stdout `1` LF |
| sem release do filho | trap | trap |
| alocador sem owner do objeto | trap | trap |

Os oito controles retornaram `3221225501` (`STATUS_ILLEGAL_INSTRUCTION`),
sem stdout. Alteram o LLVM depois da preparação/verificação, removendo o
release do filho ou trocando a fábrica Owned pela fábrica legada. Falha
de geração, abort ou erro de carregamento não conta como controle aprovado.

A função HIR cria Mint de `i64::MAX` e AllocObject de 40 campos. A preparação
do módulo escolhe fábrica Owned e setters auditados, depois libera o token
original do filho e transfere o objeto pelo retorno Owned. O harness LLVM
confere owner do objeto, bits escalares `85`, `-0.0`, `true` e byte `255`,
campo Ref no índice 39, valor/ausência de owner do filho e morte de ambos
após release do retorno e coleta. Não aloca entre morte e observação.
Compilador/runtime são Rust; não há fixture C++.

Executado com auditoria ARC, berçário desligado e GC stress. O gerador usa
perfil debug e `llvm-embutido`; O0/O2 são do executável gerado. Reprodução:
`python scripts/provar-arc-aot.py arc_alocacao_campos --debug --features llvm-embutido`.

`evidencia.json` registra comandos, versões, hashes/blobs das fontes, saídas
e SHA-256 dos 60 arquivos AOT e três logs de verificação. Suíte final:
242 testes do emissor aprovados, 7 ignorados, 52 doctests e 1 teste do exemplo.
Os bytes originais são conservados; executáveis não foram incluídos.

Limites: HIR explícita e harness LLVM, não lowering completo de fonte Dart,
origem de receivers para getters, guardas late/tipo, registro de métodos,
construtores, versões/pins, suspensão ou desempenho ARC≥A0. Tracing usa
tokens explícitos somente para comparação; no produto, ARC continua
exclusivo de `--memoria=arc`. O passe ainda não foi integrado ao pipeline
padrão. A CI desta revisão permanece pendente.
