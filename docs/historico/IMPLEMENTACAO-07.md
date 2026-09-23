# Incremento 07 — Primeiro backend AOT LLVM

DartForge passa a ter dois caminhos de emissão a partir do frontend Rust:

```text
Dart 3.6.2 → lexer/parser/análise → HIR validada
                                  ├─ JavaScript ESM
                                  └─ LLVM IR → objeto Clang → runtime/link Rust → executável
```

O linker de bibliotecas recebe um emissor selecionado e mantém os diagnósticos
associados ao arquivo de origem. A mesma resolução de `package:`, imports, filtros,
exports e privacidade atende aos dois backends, sem converter Dart para fonte Rust.
Apenas o pequeno harness/runtime é compilado como Rust; o programa Dart vira LLVM IR.

## Crates e comandos

- `dartforge-llvm`: geração textual de LLVM IR, sem bindings FFI no compilador.
- `dartforge-native`: execução direta de Clang/rustc, staging exclusivo, captura de
  erros e publicação sem sobrescrever arquivos existentes.
- `dartforge-runtime`: fonte Rust embarcada da entrada e impressão de i64/bool.

`dartforge emit-llvm entrada.dart saida.ll` preserva a IR para inspeção.
`dartforge aot entrada.dart saida.exe` usa O0; `--optimize` seleciona LLVM O2.
Clang e rustc devem produzir código para o mesmo host. Cross compilation não está
exposta. O runtime exige os serviços normais do sistema/CRT; não exige Dart SDK.

## Semântica implementada

Inteiros i64 com wrap no overflow, booleanos, void, funções/recursão, parâmetros,
locais e escopos, atribuições, comparações, if/else, while/do/for, break/continue,
return, print int/bool e curto-circuito. Variáveis usam alloca no entry; LLVM pode
promovê-las e otimizar o CFG. Chamadas e argumentos preservam a ordem Dart.

Aritmética não usa `nsw`/`nuw`: overflow nativo não pode virar poison LLVM.
`&&`/`||` usam branches/phi, não avaliação antecipada dos dois operandos.
Nomes do usuário viram identificadores numéricos internos da IR.

Classes, herança, null, strings, extensions e outros recursos sem representação
nativa são rejeitados antes de invocar ferramentas, mesmo em código morto. Isso
não altera o suporte existente no backend JavaScript. Literais continuam no domínio
i32 do parser compartilhado, embora operações/resultados nativos usem i64.
Ainda não há heap Dart, GC, exceções, bibliotecas padrão nativas ou ngdart nativo.

## Referências e instalação verificada

As [referências Dartino/LLVM](AOT-REFERENCIAS.md) distinguem o experimento de 2016
arquivado em dart-archive/sdk do alvo moderno Dart 3.6.2. Os clones ficam ignorados
em references/. Nenhum código dos forks foi incorporado à licença MIT do DartForge.

Na máquina original foi extraído LLVM **22.1.8** em `D:\LLVM\22.1.8`, a partir do
[asset oficial](https://github.com/llvm/llvm-project/releases/tag/llvmorg-22.1.8)
`LLVM-22.1.8-win64.exe`. SHA-256 verificado contra o digest da API GitHub:
`16e5709785fef73c854646241c4a92c5cd574318d1b33c63330dd7721903e55c`.
Clang e rustc 1.98.1 reportaram host `x86_64-pc-windows-msvc` e LLVM 22.1.8.
O driver usa recursos de IR disponíveis em LLVM 17+; a CI confere também o LLVM dos runners.

[Contrato do driver e fronteira FFI](AOT-DRIVER.md). O compilador/driver mantêm
`unsafe_code=forbid`; o harness compilado separadamente explicita a chamada FFI
à entrada e os símbolos exportados. Não há passagem de ponteiros nessa ABI inicial.

## Medições e próximos passos

O runner `scripts/conformance-native.ps1` compara stdout/exit com Dart VM e Dart AOT
3.6.2, em O0/O2, e registra tempos brutos quando `-Samples` é usado. O tempo inclui
inicialização de processos, compilação do runtime e link, ainda sem cache de objetos
ou runtime. Um corpus pequeno não demonstra vantagem em aplicações reais.

Próximos marcos: medir fases separadamente, cachear runtime com chave de ABI/toolchain,
ampliar IR tipada e domínio de literais, definir layouts/valores, strings, objetos,
GC e safepoints, depois exceções e bibliotecas. O fork LLVM antigo não é dependência
de build e seu coletor/exceções não foram transplantados.

## Resultados locais

- 184 testes Rust aprovados, incluindo execução nativa O0/O2, Node e doctests.
- Formatação, Clippy com warnings proibidos, rustdoc e build release aprovados.
- Cinco fixtures nativas × dois modos × três builds: 30 executáveis DartForge
  executados com resultados idênticos à VM e aos 15 executáveis AOT oficiais.
- [Relatório nativo bruto](dados/conformance-native-increment-07.json), com toolchains,
  saídas e tempos por amostra. Três amostras fornecem uma observação inicial,
  não uma comparação estatística robusta ou uma afirmação de vantagem geral.
- Exemplo persistente gerado localmente em `dist/native/fibonacci.exe`, com IR em
  `dist/native/fibonacci.ll`; execução imprime `6765` e `499500`.
  Esses artefatos de build ficam ignorados; a fonte está em `examples/native/main.dart`.

Medianas de build, incluindo processos, runtime e link (ms):

| Caso | LLVM | DartForge | Dart AOT oficial |
| --- | --- | ---: | ---: |
| arithmetic.dart | O0 | 362.71 | 1583.49 |
| arithmetic.dart | O2 | 367.01 | 1583.49 |
| calls.dart | O0 | 344.88 | 1538.58 |
| calls.dart | O2 | 357.92 | 1538.58 |
| control_flow.dart | O0 | 358.66 | 1544.78 |
| control_flow.dart | O2 | 346.25 | 1544.78 |
| edge_cfg.dart | O0 | 356.26 | 1644.31 |
| edge_cfg.dart | O2 | 355.71 | 1644.31 |
| modules/packages/main.dart | O0 | 354.73 | 1558.01 |
| modules/packages/main.dart | O2 | 369.10 | 1558.01 |

A comparação do AOT oficial inclui seu runtime e pipeline completo; o DartForge
ainda suporta um subconjunto pequeno. Os modos não representam pipelines com
o mesmo conjunto de otimizações. O próximo trabalho de desempenho deve separar
frontend, LLVM e compilação/link do runtime, mantendo equivalência de comportamento.

O backend JavaScript também passou em 50 comparações após a refatoração do linker:
[modo direto](dados/conformance-increment-07.json) e [constantes](dados/conformance-increment-07-constants.json).
