# Incremento 08 — Null safety escalar nativa e medições por fase

Alvo Dart 3.6.2, implementação Rust MIT. O trabalho parte do backend AOT existente,
sem reutilizar o runtime pré-null-safety do Dartino. As referências do SDK fixado e
os contrastes com o experimento arquivado estão em [AOT-NULL-SAFETY](AOT-NULL-SAFETY.md).

## Semântica e representação

O LLVM agora aceita `int?`, `bool?`, `null`, `??`, `!`, igualdade nullable,
parâmetros/retornos anuláveis e retorno implícito null. Os valores usam agregados
`{i1, i64}` ou `{i1, i1}` com tag de presença e payload inicializado. Não há heap,
alocação de objeto Dart ou GC para essa representação escalar.

A análise de fluxo compartilhada valida as promoções antes da emissão. Checks do
backend não tornam operações inválidas em Dart aceitáveis. O operador `??` preserva
avaliação preguiçosa e efeitos; `!` avalia seu operando uma única vez. Em falha,
o runtime imprime diagnóstico e encerra com código 101. Ainda não implementa
exceções Dart capturáveis nem reproduz a stack trace da VM.

A HIR conserva o tipo de armazenamento nullable mesmo após promoção semântica.
O backend pode inserir checks redundantes, que LLVM O2 pode eliminar. Um RHS de
tipo diferente em `??` é permitido quando o frontend provou lhs não-null; o ramo
incompatível recebe falha explícita se uma HIR inválida violar esse contrato.

Strings, classes, herança, extensions e objetos continuam fora do subconjunto nativo.
`var value = null` implica dynamic em Dart e segue rejeitado pelo frontend; use
anotação `int?`/`bool?`. O JavaScript mantém suas regras e representação anteriores.

## Perfil de compilação

`dartforge aot entrada.dart saida.exe --optimize --timings` produz somente JSON no
stdout, com schema_version 1. Erros continuam no stderr e retornam status não zero.

| Campo | O que inclui |
| --- | --- |
| frontend_ns | Carga das fontes, lexer/parser, análise, ligação de bibliotecas e emissão LLVM IR |
| prepare_ns | Gravação da IR e do harness Rust no staging |
| clang_ns | Processo Clang completo, incluindo otimização e emissão de objeto |
| rustc_link_ns | Compilação do runtime Rust e ligação do executável |
| publish_ns | Cópia exclusiva, permissões e sincronização da saída |
| driver_total_ns | Driver inteiro, incluindo preparação e limpeza |
| total_ns | Pipeline CLI, excluindo startup do processo, parsing das flags e serialização JSON |
| executable_bytes | Tamanho efetivamente publicado |

Os tempos são de parede, não de CPU. As fases são disjuntas, mas não esgotam os
overheads totais. O runner diferencial conserva também `compileMs`, medido fora do
processo, e confere o schema, a soma das fases e o tamanho do arquivo. Não há cache
de runtime/objetos nesse driver; essas medições devem orientar a próxima otimização.

## Regressões

Corpus positivo com funções nullable, igualdade entre tipos, coalescência aninhada,
retornos implícitos, promoções, reatribuições em laços e RHS incompatível mas
inalcançável. Oráculo: Dart VM/AOT 3.6.2; LLVM O0 e O2. A falha de `!` é testada
em subprocesso, exigindo código 101 e ausência de efeitos posteriores. Cinco casos
inválidos verificam que coerções do backend não contornam o checker semântico.

## Validação local e medições

- 192 testes aprovados: unidade, integração com Node/LLVM e doctests; nenhuma falha
  ou teste ignorado na execução completa. Fmt, Clippy, rustdoc e release aprovados.
- Oito casos × O0/O2 × três builds: 48 executáveis DartForge com saída equivalente
  à VM e aos 24 executáveis Dart AOT oficiais. Os relatórios de fase também foram
  validados quanto a schema, soma de intervalos e tamanho do executável.
- [Relatório bruto](conformance-native-increment-08.json) inclui toolchains, revisão
  base/estado Git, saídas, amostras externas e fases internas.
- Exemplo local `dist/native/null-safety.exe`, com LLVM O2, imprime `null`, `7`,
  `43`, `42`. IR e JSON de tempos ficam ao lado; fontes em `examples/native-null/`.

Para `null_safety.dart`, medianas de três amostras nesta máquina (ms):

| Modo | Frontend | Clang | Runtime/link Rust | Total CLI |
| --- | ---: | ---: | ---: | ---: |
| O0 | 0.761 | 51.139 | 269.980 | 348.759 |
| O2 | 0.678 | 56.624 | 280.077 | 346.267 |

As medianas não são aditivas. O corpus é pequeno e três amostras não demonstram
superioridade geral. A medição separa o custo do frontend do custo repetido de
compilar/linkar o runtime, preparando uma avaliação posterior de cache por ABI.
