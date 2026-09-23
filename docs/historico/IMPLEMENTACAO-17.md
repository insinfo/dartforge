# Incremento 17 — this, escopos e construtores

O frontend admite construtores generativos sem nome com parâmetros posicionais
obrigatórios, incluindo `Usuario(this.username, this.email)`. Campos sem
inicializador são representados explicitamente e validados antes da emissão.

O linker preserva a precedência de locais e parâmetros sobre membros, inclusive
campos privados importados. `this.campo` acessa a instância explicitamente. No
corpo do construtor, um formal `this.campo` não introduz uma variável local.

JS e LLVM avaliam os argumentos na ordem escrita, executam inicializadores dos
campos derivados antes dos campos da base e depois os corpos da base para a
derivada. Campos mutáveis com inicializador e formal executam o inicializador
antes da sobrescrita. O teste diferencial inclui despacho virtual durante o
construtor da base para verificar que o campo derivado já foi inicializado.

No JavaScript, classes cuja hierarquia contém construtor explícito usam factories
internas com o prototype correto; isso permite inicializar a instância antes dos
corpos sem violar a restrição JavaScript de usar this antes de super(). Classes
sem esses construtores mantêm o caminho anterior. No LLVM, argumentos e instância
são protegidos pelas raízes do GC; corpos usam helpers void e o factory retorna
a instância somente após concluir a construção.

Os passes percorrem argumentos e corpos de construtores. A fusão de funções
considera seus argumentos e evita capturar nomes de parâmetros de construtores.

Contrato, exemplos, referências e limites: [THIS-CONSTRUCTORS.md](THIS-CONSTRUCTORS.md).
Fixtures integradas: `tests/conformance/modules/constructors17` e
`tests/native/modules/constructors17`. A implementação FFI do incremento 16
permanece no conjunto de testes locais; o CI não é aguardado nesta etapa.

## Resultado local integrado

- **335 testes Rust/doctests** passaram: suíte workspace e reexecução do alvo
  LLVM após corrigir um identificador não suportado no fixture original.
- Rustfmt, Clippy all-targets e Rustdoc privado com `-D warnings` passaram.
- **40 casos JavaScript** passaram contra dart2js O2, com constantes e fusão habilitadas.
- **40 execuções nativas O0/O2** passaram contra Dart VM/AOT, com GC forçado e fusão.
- **2 execuções FFI O0/O2** passaram contra o oracle real @Native do VM/AOT.

Relatórios: [JavaScript](dados/conformance-js-increment-17.json),
[nativo](dados/conformance-native-increment-17.json) e
[FFI](dados/conformance-ffi-increment-16-17.json). São verificações do subconjunto
exercitado; não demonstram superioridade de desempenho sobre DDC ou dart2js.
Conforme solicitado, a validação desta entrega é local e o commit omite CI.
