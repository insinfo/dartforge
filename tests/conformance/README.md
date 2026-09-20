# Corpus diferencial

Casos originais DartForge, sob MIT. O corpus cobre aritmética, escopos, strings Unicode,
booleanos, limites numéricos, funções, recursão, retorno, if/else, curto-circuito e ordem
de avaliação dos argumentos. Não é a suíte completa do SDK Dart.

    ./scripts/conformance.ps1

O runner exige Dart 3.6.2, compila cada caso com DartForge e dart2js -O2, executa ambos
no Node e compara stdout. SDK, flags e resultados ficam em target/conformance/<id>/results.json.
O SDK 3.6.2 instalado corresponde ao alvo; não é necessário AllowVersionMismatch.
Para selecionar outro executável da mesma versão, use -DartExe caminho/para/dart.

Para investigar outra versão explicitamente: -AllowVersionMismatch. Essa opção marca
os resultados como exploratórios e não permite alegar conformidade com o alvo.

Matriz opcional para diagnóstico:

    ./scripts/conformance.ps1 -OptimizationLevels O0,O1,O2,O3

O caso numeric_edges tem divergência conhecida com -O0 no SDK 3.6.2. Divergências
permanecem registradas e retornam falha; não são tratadas como aprovações.
Consulte ../../docs/DART2JS-REFERENCIA.md.

Testes Rust: cargo test --locked --workspace. Testes adicionais com Node:

    cargo test --locked --workspace -- --include-ignored

Não confundir inteiros da VM com o backend web. Próxima etapa: fixtures selecionadas da
revisão 3.6.2 do SDK com origem, licença, expectativa e skips explícitos.
