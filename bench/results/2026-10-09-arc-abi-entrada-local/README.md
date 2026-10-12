# Conferência compilada da entrada ARC — Windows, 2026-10-09

Fonte da CLI: `eaa457d835530dc15c0df3170edd5fe5682d48ae`. Compilação
release com JIT, encerrada com sucesso em 8 min 59 s. SHA-256 da CLI:
`f63880596b253db5b2d5d0cbd48cdda5c9b9b222f711dc31a3ffed0a2b22ffc0`.
Dart 3.6.2 e Clang 22.1.8, caminhos registrados em `cli.json`.

## Resultado

Dez processos raiz monitorados: nove encerrados com zero e um encerrado
com a falha fatal esperada. A guarda foi extraída do IR **real** emitido
para `gc_d04_finally.dart`, na função `df.preparar_isolado`. Dois módulos
LLVM pequenos usam exatamente esse trecho com stubs do runtime que retornam
0 ou 1 na conferência. Ambos compilam com `clang -O2`: 1 permite retorno
normal; 0 executa `llvm.trap` e termina com `3221225501` (`0xC000001D`,
instrução ilegal), stdout/stderr vazios. Esses stubs testam os ramos da
guarda; não simulam uma DLL antiga completa nem certificam seus contratos.

Dois executáveis AOT ARC reais, otimizados, com exceções por checagem e
tabelas, executaram `gc_d04_finally.dart` sob `GC_STRESS=1`. Ambos tiveram
código zero, stderr vazio e stdout igual ao Dart: `336 10 1566\n`.
ARC auditado, ciclos sempre, berçário desligado e heap de 256 MB. SDK da
fonte, sem `DARTFORGE_SDK_DLL` selecionada no ambiente; isso não afirma
ausência de uma biblioteca do SDK escolhida automaticamente pelo compilador.
A emissão de IR e o oráculo Dart não usam GC_STRESS.

Hashes da CLI e da entrada foram conferidos; a CLI permaneceu idêntica
antes/depois do runner. Os hashes das quatro imagens e do IR real foram
conferidos nos arquivos locais. O script arquivado é byte a byte o script
executado, conforme `script_sha256`. Imagens e IR completo permanecem em
`target/arc-abi-entrada-validacao`; este arquivo guarda seus hashes e os
trechos LLVM pequenos, sem incluir binários no histórico.
Os arquivos deste diretório usam `-text` para preservar os bytes arquivados
e seus checksums em qualquer configuração de finais de linha do Git.

Não é benchmark, corpus completo ou prova de ownership SSA integrado,
ArcKeepAlive, cleanup ARC ou contratos gerais de retornos owned. Não prova
o gate ARC >= A0. A compatibilidade verificada pela entrada é a versão de
tokens e o modo do heap.

## Reprodução

Na raiz do repositório, com a CLI compilada para a revisão examinada:

```powershell
. ./scripts/env.ps1
$env:TEMP = Join-Path (Get-Location) 'target/tmp-finalizavel-semantico'
$env:TMP = $env:TEMP
New-Item -ItemType Directory -Force $env:TEMP | Out-Null
python bench/resultados/2026-10-09-arc-abi-entrada-local/validar.py target/arc-abi-entrada-repro
```

O destino deve ser novo. O runner recusa sobrescrita, preserva resultados
parciais de falhas, monitora processos sequencialmente e verifica stdout e
integridade da CLI. O nome/hash da revisão registrada muda numa reprodução;
comparar também a fonte e as opções, não só o resultado final.
