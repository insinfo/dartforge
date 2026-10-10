# Cleanup de laço — AOT Windows, 2026-10-10

Prova local com HIR explícita, preparação ARC, driver nativo e runtime Rust.
Não há fixture C++. Base `66fd85db`; o exemplo e o script eram novos no
workspace. O manifesto registra as modificações, SHA-256 e blobs Git dessas
fontes, que acompanham esta evidência no commit de publicação.

| Caminho | ARC O0/O2 | tracing O0/O2 |
| --- | --- | --- |
| normal, dez iterações | saída `1` LF, código 0 | saída `1` LF, código 0 |
| erro, uma iteração | saída `1` LF, código 0 | saída `1` LF, código 0 |
| normal sem release na aresta | trap | trap |
| erro sem release na saída | trap | trap |

Os oito controles retornaram `3221225501` (`STATUS_ILLEGAL_INSTRUCTION`,
equivalente ao código assinado `-1073741795`), sem stdout. Os controles
removem uma chamada de release do LLVM já preparado e verificado. Falhas
de geração não contam como sucesso dos controles.

A cada iteração, a função aloca Mint de `i64::MAX` e o hook observa bloco
vivo com owner (`3`). Na continuação normal, o cleanup precede a coleta e
o segundo hook verifica ausência de owner do código. Depois de a função
sair, o harness coleta e confere morte física (`0`) do último objeto, sem
alocação entre morte e observação. Endereços guardados para diagnóstico
não são registrados como raízes Ref. No tracing, raízes observacionais
podem manter o bloco físico durante a função.

O erro usa exceção real pré-carregada (`42`, handle Smi `85`), não throw
originado no hook. O harness verifica identidade da exceção, captura e
liberação, clear, ausência de owner e morte final do Mint da primeira volta.

Executado com `DARTFORGE_ARC_CONFERIR=1`, `DARTFORGE_ARC_BERCARIO=0` e
`DARTFORGE_GC_STRESS=1`. Comandos, versões, códigos de saída e SHA-256 dos
80 arquivos brutos estão em `evidencia.json`. O gerador usa perfil debug,
com `llvm-embutido`; O0/O2 referem-se ao executável nativo gerado.
`.gitattributes` conserva bytes originais. Executáveis não foram incluídos.

Reprodução: `python scripts/provar-arc-aot.py arc_cleanup_laco --debug --features llvm-embutido`.

Limites: não prova morte individual de todas as alocações anteriores,
lowering de fonte Dart, junções gerais, unwind, Finalizable, suspensão ou
desempenho ARC≥A0. Tracing usa o mesmo corpo com tokens explícitos apenas
para comparação de runtime. ARC no produto só é selecionado com
`--memoria=arc`; tracing continua o padrão. A CI desta revisão está pendente.
