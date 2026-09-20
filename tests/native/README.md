# Corpus nativo Dart 3.6.2

Fontes originais do projeto, sob MIT. Os arquivos `.stdout` foram obtidos executando
os casos com `Dart SDK 3.6.2 stable` (VM) e são verificados pelos testes Rust que
compilam com LLVM O0/O2 e executam os artefatos em Windows e Linux.

`scripts/conformance-native.ps1` recompila as mesmas fontes com `dart compile exe`,
executa também a VM e exige equivalência dos quatro caminhos: VM, AOT oficial,
DartForge LLVM O0 e O2. Em caso de falha, retorna erro em vez de publicar relatório
positivo. `-Samples N` repete builds em processos distintos e registra tempos brutos.
Não mede DDC, dart2js nem prova conformidade da linguagem completa.

Casos: i64/overflow, chamadas/recursão, ordem de argumentos, controle de fluxo,
curto-circuito, escopos, package_config, privacidade e exports.
