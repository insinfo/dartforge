# Campos locais — AOT Windows, 2026-10-10

Fontes preservadas em `5d0272e9a2edb6f30f4e679ec6352420ca473aab`.
A execução ocorreu sobre a base `abff112f` com fontes modificadas no
workspace. O manifesto separa essa base do commit que preserva as fontes,
incluindo hashes SHA-256 e blobs Git do gerador, script e implementação.

| Variante | ARC O0/O2 | tracing O0/O2 |
| --- | --- | --- |
| normal | código 0, stdout `1` LF | código 0, stdout `1` LF |
| sem retenção produzida | trap | trap |
| sem cleanup do objeto | trap | trap |
| sem liberação final do retorno | trap | trap |

Os doze controles retornaram `3221225501` (`STATUS_ILLEGAL_INSTRUCTION`),
sem stdout. Alteram o LLVM depois de preparar/verificar a HIR. Falha de
geração, abort ou erro de carregamento não conta como controle aprovado.

A função HIR aloca Mint de `i64::MAX` e instância de 40 campos. A preparação
confere a origem/layout local e escolhe as ABIs auditadas para GetField e
SetField. Depois da inicialização, muda os escalares e confere os valores
relidos: int `85`, bits de `-0.0`, bool `true` e byte `255`. Lê o campo Ref
39 antes das escritas, recebe keepalive antes da primeira invalidação,
substitui o campo por null e devolve o filho Owned.

O harness verifica valor/owner do retorno, morte do receiver após sair da
função/coletar e morte do retorno após liberação/coleta. Endereço diagnóstico
guardado no LLVM não é raiz Ref; não há alocação entre morte e observação.
Compilador/runtime são Rust; não há fixture C++.

Gerador em debug com `llvm-embutido`; O0/O2 são do executável gerado.
Ambiente: auditoria ARC, berçário desligado e GC stress. Reprodução:
`python scripts/provar-arc-aot.py arc_campos_locais --debug --features llvm-embutido`.

`evidencia.json` conserva comandos, ferramentas, saídas e SHA-256 dos 80
arquivos AOT e quatro logs. Suíte: 248 testes do emissor aprovados, 7
ignorados, 52 doctests, 6 testes focados e 1 teste do exemplo. Os bytes
originais são conservados; executáveis não foram incluídos.

Limites: HIR explícita, não lowering completo de fonte Dart. Não prova
parâmetros/receivers publicados, resumos de heap do SDK, guardas late/tipo,
certificados contra adulteração, versões/pins, suspensão, Finalizable,
unwind ou desempenho ARC≥A0. Tracing compara tokens explícitos somente no
harness. Integração ao pipeline padrão permanece pendente e será exclusiva
de `--memoria=arc`; tracing continua o padrão. CI desta revisão pendente.
