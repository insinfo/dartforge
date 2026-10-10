# Cadeia de retorno e preparação de raízes — Windows, 2026-10-10

Fontes: `5cb4e56d237a452b61aa3f8fa648fbad050a2df9`, sem alterações locais
nos fontes da matriz. SDK Dart 3.6.2; Clang 22.1.8; gerador Rust debug,
executáveis AOT O0/O2. Valida a extração da preparação compartilhada de
tipos/apontados/conversões de Phi após inclusão de contexto por raízes na
análise ARC. Não é medição de desempenho.

A fonte define `identidade` e `repassar`, que chama identidade duas vezes.
O lowering produz o contrato de retorno Owned; os planos de ownership
preparam os corpos e suas tabelas sem construir manualmente esses corpos HIR.
O harness observa identidade do Mint real, sobrevivência após soltar o
argumento e morte física após soltar o retorno e coletar.

Passaram 16 execuções em ARC/tracing × O0/O2: quatro positivas (`1\n`, código
zero) e 12 controles negativos por trap. Cada combinação retira separadamente
o retain, o drop do retorno ou o drop intermediário. Compilação falha, saída
inesperada e erro de carregamento não contam como controles aprovados.
Auditoria ARC, ARC puro e GC stress ficam ligados no ambiente do harness;
tracing não ativa ARC no heap. Na CLI, ARC permanece exclusivo de `--memoria=arc`.

Os 96 arquivos brutos conservam fonte Dart, LLVM, build log, stdout, stderr
e código de saída de cada caso. `evidencia.json` registra comandos, versões,
hashes e resultado; os hashes dos arquivos e dos três logs adicionais foram
conferidos antes do arquivamento. Não há executáveis arquivados. A evidência
anterior de 12 casos em `2026-10-10-retorno-da-fonte-windows` foi preservada.

Os logs adicionais registram a matriz AOT, os 306 testes unitários do emissor
(sete ignorados) e os 97 exemplos públicos mais uma rejeição na compilação.
O aviso de `atribuir_slots` não usado é preexistente. O produtor desses logs
é a mesma revisão acima; a execução AOT foi posterior à suíte e aos exemplos.

Para reproduzir, configure Clang/LLVM e `DARTFORGE_SDK_LIB` para o diretório
`lib` do SDK 3.6.2, e execute:

```text
python scripts/provar-arc-aot.py arc_retorno_da_fonte --debug
```

A prova é local à cadeia de retorno e aos tokens explícitos do harness.
Não certifica programa/SDK completo, observadores, dispatch, callbacks,
instrumentação ou políticas dos §§27–34. O gate ARC≥A0 continua reprovado.
Ao arquivar, a CI da revisão publicada `b0ee6645` ainda executava no Windows
e no macOS; Linux e mensagens já haviam passado.
