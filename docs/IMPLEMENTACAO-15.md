# Incremento 15 — imports e exports condicionais

O resolvedor seleciona a primeira condição verdadeira, conforme Dart 3.6.2:

```dart
export 'client_stub.dart'
    if (dart.library.io) 'client_native.dart'
    if (dart.library.js) 'client_js.dart'
    if (dart.library.js_interop) 'client_wasm.dart';
```

Somente o destino selecionado entra no grafo. Alternativas inativas não precisam
existir e não carregam pacotes ou bibliotecas SDK. A sintaxe de toda a diretiva
continua sendo validada. Filtros show/hide, ciclos e URIs package: mantêm suas
regras após a seleção. Condições aceitam nomes pontuados e igualdade com strings,
inclusive strings raw e escapes.

## Perfis e comandos

`compile` usa o perfil JavaScript; `emit-llvm` e `aot` usam Native AOT. A inspeção
do grafo permite escolher o perfil explicitamente:

```powershell
cargo run -p dartforge-cli -- graph tests/conformance/modules/conditional15/main.dart --target js
cargo run -p dartforge-cli -- graph tests/conformance/modules/conditional15/main.dart --target native
cargo run -p dartforge-cli -- graph tests/conformance/modules/conditional15/main.dart --target wasm
```

A saída JSON versão 2 inclui alvo, versão do SDK de referência e inventário de
flags. Wasm permite inspecionar a seleção; emissão e execução Dart em Wasm ainda
não estão implementadas. APIs de compilação rejeitam ambientes incompatíveis com
o backend. Flags descrevem o SDK de referência, sem prometer implementação de
dart:io, dart:ffi ou interop no DartForge. Imports ativos dessas bibliotecas ainda
produzem diagnóstico; apenas o subconjunto existente de dart:core está disponível.

No SDK **3.6.2**, js_util também é anunciado em Wasm. Para distinguir os alvos,
use js antes de js_interop, como acima. A matriz foi conferida no código fixado
e por execução de programas VM, AOT, JS e Wasm do SDK. Veja os comandos e a
matriz completa em [ENVIRONMENT-REFERENCIAS.md](ENVIRONMENT-REFERENCIAS.md).

Uma biblioteca ausente não equivale à string 'false' nem à string vazia.
Nomes não reservados avaliam como string vazia nas condições de URI do SDK
3.6.2, mesmo com -D. Este incremento não adiciona uma opção -D ao DartForge.

## Integração e cache

CompilationEnvironment acompanha o SourceGraph e participa da identidade do
cache. As APIs antigas mantêm seus padrões; as novas variantes with_environment
recebem o ambiente explicitamente. Alterar uma fonte inativa preserva um cache
válido; alterar a fonte ativa ou a seleção invalida o resultado. Falhas limpam
o resultado anterior. A sessão ainda relê o grafo ativo para verificar igualdade;
isso não constitui compilação incremental por função.

Os testes cobrem precedência, ausência, filtros, pacotes, ciclos, diagnósticos com
spans, invalidação e isolamento de alternativas. O fixture conditional15 participa
das suítes diferenciais JavaScript e LLVM O0/O2.

## Resultado integrado

- 311 testes passaram, somando a suíte normal e os testes com dependências externas.
- Rustfmt, Clippy all-targets e Rustdoc privado com avisos tratados como erros passaram.
- 38 casos JavaScript passaram contra dart2js O2, com otimização de constantes e fusão.
- 36 execuções nativas O0/O2 passaram contra Dart VM/AOT, com fusão, GC forçado e estatísticas.
- A CLI release selecionou arquivos distintos nos três perfis do mesmo fixture.

Relatórios: [JavaScript](conformance-js-increment-15.json) e
[nativo](conformance-native-increment-15.json). Esses resultados verificam o
subconjunto exercitado; não comprovam vantagem de desempenho sobre DDC/dart2js.
