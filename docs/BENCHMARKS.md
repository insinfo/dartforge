# Medição e comparação

Ainda não existem benchmarks que demonstrem vantagem do DartForge.
O tempo do exemplo print mede somente um subconjunto minúsculo e não serve para comparar
um compilador completo com este bootstrap.

## Matriz futura

| Cenário | Baseline | Medidas |
|---|---|---|
| Build de desenvolvimento frio | DDC via toolchain webdev fixada | wall time, CPU, pico RSS |
| Rebuild de corpo/assinatura/template | DDC + invalidação equivalente | mediana, p95, arquivos revisitados |
| Build de produção | dart compile js com flags registradas | tempo, memória, tamanho |
| Execução do JavaScript | mesmos browsers/Node e entradas | startup, throughput, latência |
| Editor | analysis_server com mesmo corpus | tempo de diagnóstico, completion, RSS |

## Protocolo

1. Selecionar um SDK Dart 3.6.2 e confirmar dart --version antes das medições. O executável atualmente identificado no PATH informa 3.6.2 e corresponde ao alvo. O clone dos fontes NÃO fornece dart.exe.
2. Manter Dart 3.6.2 como baseline inicial e fixar versão ngdart, commits, lockfiles, hardware e configuração de energia.
3. Exigir equivalência dos testes antes de medir velocidade.
4. Compilar DartForge em release e excluir compilação do Rust do tempo de uso.
5. Separar cache do processo, cache do compilador e cache de disco. Documentar o que é “frio”.
6. Fazer pelo menos 5 warmups e 30 amostras para medições curtas; ajustar para builds longos.
7. Registrar amostras brutas, mediana, p95, dispersão, CPU e pico de memória.
8. Medir subprocessos e servidor residente quando existirem. Working set não equivale em todos
   os sistemas a RSS; registrar método de coleta.
9. Usar as mesmas garantias de runtime, modo de compilação e cobertura da linguagem.
10. Publicar regressões e limitações, sem extrapolar microbenchmarks para aplicações completas.

Instrumentar fases: I/O, lex, parse, resolve, typecheck, lower, optimize, emit, bundle.
Para comparação DDC, reproduzir o comando efetivamente usado pela versão fixada de webdev;
não inventar uma interface estável de linha de comando do compilador interno.
