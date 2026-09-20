> Histórico do primeiro incremento. O alvo foi posteriormente corrigido pelo proprietário para Dart 3.6.2; consulte README.md e IMPLEMENTACAO-02.md para o estado atual.

# DartForge — primeiro incremento

Implementação realizada com três subagentes (frontend, análise semântica, backend) e
integração/validação no projeto D:\Projects\dartforge.

## O que funciona

- Lexer e parser para variáveis locais, blocos, atribuições e expressões com precedência.
- var, final e anotações int/String/bool; literais inteiros i32 e strings Unicode simples.
- Operadores +, -, *, ==, !=, <, <=, >, >=, &&, ||, negação numérica e booleana.
- Resolução lexical, sombreamento, tipos, duplicatas, uso antes da declaração e proteção de final.
- Emissão JavaScript ESM com identificadores protegidos e preservação de escopos.
- Comentários aninhados e limites explícitos de complexidade para evitar árvores excessivas.
- Runner diferencial que compara programas com o backend JavaScript oficial.

## Validação

- Formatação, Clippy com -D warnings, testes e build release: passaram.
- 25 testes Rust padrão passaram; 1 teste adicional com Node, ignorado por padrão, também passou.
  Total de testes distintos executados com sucesso: 26.
- Cinco casos diferenciais passaram: arithmetic, boolean, numeric_edges, scopes, strings.
- Proteção contra usar versão de SDK diferente sem opção explícita: verificada.

Alvo continua Dart 3.6.0. O executável efetivamente encontrado foi
C:\tools\dartsdk-3.6.2\bin\dart.exe, SDK 3.6.2. Portanto a comparação foi exploratória,
com -AllowVersionMismatch, e NÃO certifica conformidade com 3.6.0.
Node usado: v24.14.1. Baseline: dart compile js -O1.

Resultados detalhados: docs/conformance-increment-01.json.
Logs de checks: target/check-increment-01.log.

## Executar

    cd D:\Projects\dartforge
    . .\scripts\env.ps1
    cargo run -p dartforge-cli -- compile tests/conformance/cases/arithmetic.dart dist/arithmetic.mjs
    node dist/arithmetic.mjs

O CLI recusa sobrescrever saídas existentes. Escolha outro nome ao repetir.

    .\scripts\check.ps1
    cargo test -p dartforge-codegen -- --include-ignored
    .\scripts\conformance.ps1 -AllowVersionMismatch

Quando Dart 3.6.0 estiver selecionado, executar conformance.ps1 sem AllowVersionMismatch.

## Limites

Consulte docs/SUBCONJUNTO.md. Não há funções gerais, classes, imports, bibliotecas Dart,
ngdart, LSP ou servidor web completos. A HIR ainda é estrutural. Ainda não há otimizações
globais nem benchmarks que sustentem vantagem sobre DDC/dart2js.

A investigação de zero negativo encontrou especialização de print dependente do programa
em dart2js 3.6.2. O compilador normaliza zero nas operações inteiras suportadas; isso não
substitui a futura bateria de conformidade numérica de Dart 3.6.0.
## Complemento: níveis de otimização e referências

Baseline principal do runner atualizado para -O2. A matriz adicional O0/O1/O2/O3 teve 19/20 concordâncias; numeric_edges divergiu somente em O0. Consulte DART2JS-REFERENCIA.md e conformance-optimization-matrix.json. A execução original O1 acima é preservada como histórico.

Referências adicionadas: typescript-go, typescript (continuação atual), angular e oxc-angular-compiler. URLs, commits e licenças estão em references/manifest.json.
