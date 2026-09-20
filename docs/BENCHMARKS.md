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

## Escalabilidade de classes e bibliotecas

    cargo bench --locked -p dartforge-codegen --bench classes
    cargo bench --locked -p dartforge-compiler --bench libraries

O benchmark de classes usa ASTs com hierarquias reversas de 100/1.000 classes para
isolar emissão e ordenação topológica. Não inclui lexer, parser, análise ou execução.
O de bibliotecas cria arquivos temporários antes de medir: separa carregamento do grafo,
compilação do grafo pré-carregado e pipeline com leitura, nos dois modos. Todas as funções
públicas são alcançáveis; os símbolos privados homônimos exercitam os namespaces.
Também mede acerto exato e miss após limpar o cache da sessão. O acerto ainda carrega
todas as fontes, mas evita parsing/análise/emissão e cópia do JavaScript. Não há
recompilação incremental por unidade. A criação do corpus fica fora do tempo.

DARTFORGE_BENCH_LIBRARIES controla o número de bibliotecas (100) e
DARTFORGE_BENCH_SAMPLES controla amostras (21) no benchmark de bibliotecas. O relatório
registra arquivos, bytes, amostras, toolchain e estado Git. O filesystem está aquecido.

## AOT nativo inicial

    ./scripts/conformance-native.ps1 -Samples 3

Executa os casos de `tests/native/` na VM Dart 3.6.2, no AOT oficial e no DartForge
LLVM O0/O2. Registra tempos de build completos, incluindo subprocessos Clang/rustc,
runtime e link; não mede somente emissão de IR. O LLVM instalado deve ser informado
com `-ClangExe`, `DARTFORGE_CLANG` ou PATH. `-DartExe` seleciona o SDK alvo.
Os [resultados iniciais](IMPLEMENTACAO-07.md) usam corpus pequeno e três amostras;
não comprovam superioridade geral nem comparam AOT com DDC/dart2js.

A partir do incremento 08, cada amostra nativa registra também `phaseTimings`,
obtido de `aot --timings`. `frontend_ns` inclui carga, lexer/parser, análise,
link de bibliotecas e emissão da IR; `clang_ns` inclui o subprocesso LLVM e
`rustc_link_ns` inclui compilação do harness e link. O total do CLI exclui seu
próprio startup e serialização JSON; `compileMs` do runner inclui o processo.
As fases não incluem cache; não devem ser interpretadas como recompilação incremental.
