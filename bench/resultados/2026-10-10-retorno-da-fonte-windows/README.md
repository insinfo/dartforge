# Retorno Owned do corpo Dart da fonte — Windows, 2026-10-10

Fontes: `e1233a129e213028e59731f30d4d7f64ea8140d3`, sem mudanças locais na matriz final.
SDK Dart 3.6.2; backend Clang, gerador Rust debug, executáveis AOT O0/O2.

O exemplo baixa `Object? identidade(Object? valor) => valor;` da fonte,
seleciona o corpo no grupo fechado e prepara as exceções e ownership com
planos inicialmente padrão. O fato nominal de retorno vem do lowering.
O harness recebe um Mint real, compara identidade, solta o argumento,
confere sobrevivência do retorno e depois sua morte física após release/coleta.

Doze execuções: quatro positivas e oito controles por trap, em ARC/tracing
× O0/O2. Retirar o retain produzido ou o release final reprova a prova.
Tracing compara os tokens explícitos do harness sem ativar ARC no heap.
A regra da CLI continua: ARC somente por `--memoria=arc`.

Há 72 arquivos brutos (inclusive as fontes Dart) e três logs locais.
`evidencia.json` conserva comandos, hashes, saídas, versão do SDK e o commit
usado na matriz; registra a precedência dos logs de suite/doctests.
Nenhum executável é arquivado. Os atributos preservam os bytes brutos.

Para reproduzir, defina `DARTFORGE_SDK_LIB` para `lib` do SDK 3.6.2,
configure Clang/LLVM conforme o repositório e execute:

```text
python scripts/provar-arc-aot.py arc_retorno_da_fonte --debug
```

Essa prova não certifica todo o programa/SDK, entradas uniformes, callbacks
FFI, dispatch, construção, Finalizable ou exceções gerais. A integração
completa e o gate ARC/A0 permanecem pendentes. A CI da fonte anterior
137e428c ainda estava em execução ao congelar esta evidência.
