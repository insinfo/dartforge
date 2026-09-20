# Benchmarks reproduzíveis

Os objetivos são latência de desenvolvimento menor que DDC e compilação de produção
menor que dart2js, preservando semântica e qualidade de saída. Eles não foram demonstrados.
Uma comparação de um subconjunto sem runtime completo com compiladores completos não
permite anunciar um fator geral de aceleração.

## Fases do compilador

    cargo bench --locked -p dartforge-compiler --bench pipeline

O executável escreve JSON com mediana, p95 e todas as amostras de lexer, parser,
análise semântica, emissão e pipelines completos sem/com avaliação de constantes. Usa black_box, aquecimento explícito e
lotes para reduzir o custo do relógio. Inclui a destruição das estruturas alocadas.
Mediana e p95 são calculados sobre médias de lotes, não sobre cada chamada individual.
O corpus é sintético e criado antes da medição; não inclui disco, startup, SDK ou Node.
No corpus de fases somente f0 é alcançável; o corpus por processos chama todas as funções.
Metadados indisponíveis aparecem como null; git_status_porcelain não vazio identifica
uma árvore diferente do commit registrado. Os tempos isolados não devem ser somados como se fossem um pipeline completo.

Variáveis opcionais: DARTFORGE_BENCH_ITERATIONS (100), DARTFORGE_BENCH_SAMPLES (21),
DARTFORGE_BENCH_FUNCTIONS (100). Todas devem ser inteiros positivos.
Mantenha toolchain, hardware, corpus, configurações e revisão fixos ao comparar mudanças.
Execute benchmarks sem agentes/Cargo concorrentes antes de publicar conclusões.

## Processos novos: DartForge, DDC e dart2js

    ./scripts/benchmark-process.ps1 -Samples 7 -Functions 100

Exige SDK Dart 3.6.2 e Node. Compila o CLI em release antes da medição e gera um corpus
com todas as funções alcançáveis. Mede processos novos com arquivos aquecidos, alterna
ordem dos compiladores e salva amostras, comandos implícitos no script, versões,
hardware e hash da entrada em target/bench-process/<id>/results.json.

DDC: snapshot oficial dartdevc, módulo common, null safety padrão. dart2js: compile js -O2.
DartForge: pipeline padrão e pipeline com --optimize, registrados separadamente. A execução do JavaScript de DartForge e dart2js é comparada
fora da janela medida. A saída DDC ainda não é executada: o relatório registra essa
limitação, não presume equivalência observável e não calcula fatores de aceleração.

O tempo inclui inicialização, compilação, disco e invocação PowerShell. Não mede uma
sessão incremental persistente do DDC. Tamanho dos artefatos não é comparado, porque
runtime compartilhado, source maps e otimizações são diferentes.

## Próximos controles

- Corpus real ngdart e imports resolvidos, com saída funcionalmente equivalente.
- Sessões DDC persistentes e recompilações após mudanças em corpo, assinatura e imports.
- RSS máximo, alocações, CPU, tamanho com runtime incluído e execução do JavaScript.
- Repetições independentes com mediana/p95 e orçamento de regressão após baseline estável.
