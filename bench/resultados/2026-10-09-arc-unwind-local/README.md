# Saída excepcional local em ARC — validação dirigida

Fonte da CLI: `0c03571cec76e38bfb811579b82ebe2762794584`, build release
com feature `jit`, Rust 1.98.1, LLVM 22.1.8, Dart SDK 3.6.2 no Windows.
Hash da CLI em `cli.json`; hashes das oito imagens em `executaveis.json`
e das duas entradas em `entradas.json`.

Os 18 processos terminaram com código zero: duas execuções Dart, oito
compilações AOT e oito execuções AOT. Todas as saídas AOT coincidiram com
o oráculo Dart; stderr das oito execuções AOT vazio. Cada entrada foi
compilada com `--optimize`, tracing/ARC e checagem/tabelas, com SDK da
fonte e sem DLL selecionada por ambiente.

`gc_d03_excecao_profunda.dart` executou sem DARTFORGE_GC_STRESS no ambiente;
`gc_d04_finally.dart` executou com essa variável presente e igual a 1.
Ambos usaram heap de 256 MB; auditoria ARC ligada, ciclos sempre e
berçário desligado. Essas opções ARC não selecionam ARC no executável
tracing. Os tempos individuais são diagnósticos, sem repetição ou gate
de desempenho. Esta comparação não é o corpus completo, não insere owners
e não comprova proteção léxica de Finalizable.

Reproduzir após carregar `scripts/env.ps1` e construir a CLI dessa fonte:

```powershell
python bench/resultados/2026-10-09-arc-unwind-local/validar.py target/arc-unwind-repeticao
```

O diretório de saída precisa ser novo. O script preserva resultados parciais
em falha e não sobrescreve uma validação anterior. `resultados.json` contém
comandos, saídas, códigos e tempos medidos; `cli.json` vincula fonte e script.
