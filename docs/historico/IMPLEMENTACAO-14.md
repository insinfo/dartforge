# Incremento 14 — modificadores, mixins e sealed

Dart 3.6.2 permanece o alvo. O frontend aceita base, final, sealed, mixin e mixin
class, com as combinações abstract/base compatíveis. A semântica verifica
restrições por biblioteca e a propagação transitiva de base/final.

Mixins são expandidos antes da análise em uma cadeia de classes abstratas
sintéticas. Os membros preservam o contexto lexical de origem e a identidade
privada da biblioteca. JS e LLVM reutilizam inicialização e despacho de classes.
No LLVM, o dono físico da aplicação determina os offsets dos campos; isso evita
usar o layout original do mixin quando a superclass tem campos adicionais.
Getters do subconjunto e chamadas implícitas também passam pelo despacho nativo.

Padrões de objeto vazios C() e exaustividade sealed foram acrescentados ao JS.
O compilador não cria instâncias ao testar padrões. Subtipos abertos não são
tratados como conjuntos fechados de classes atualmente conhecidas.

## Uso e limites

    cargo run -p dartforge-cli -- compile examples/modifiers/main.dart dist/modifiers.mjs
    node dist/modifiers.mjs

O exemplo combina EstadoLogin, Voador, Veiculo/Carro e Pagamento. Regras precisas
e recursos pendentes estão em [CLASS-MODIFIERS.md](CLASS-MODIFIERS.md).
Ainda faltam part/part of, restrições on, super, with em enums, aliases de aplicação,
padrões com desestruturação e lowering LLVM de switch. Anotações como @override
continuam fora do parser; os exemplos próprios omitem a anotação.

## Validação

Os testes próprios verificam:

- Restrições entre arquivos importados e mensagens localizadas nos dois backends.
- Propagação base/final através de hierarquias intermediárias e enums.
- Ordem de aplicação, precedência de métodos, inicializadores com efeitos e estado independente.
- Duas aplicações de um mixin sobre bases com quantidades diferentes de campos.
- Identidade privada de campos de bibliotecas diferentes com o mesmo nome original.
- Mixin class usado como classe e como mixin; getters e despacho por contrato de mixin.
- Cobertura sealed com subtipos abertos, nulabilidade, guardas, implements e imports.

As referências consultadas são os testes de class_modifiers e base_transitivity
da tag 3.6.2 do SDK. As saídas dos fixtures foram conferidas no Dart VM 3.6.2.

## Resultado integrado

- **296 testes** passaram com `cargo test --locked --workspace --no-fail-fast -- --include-ignored`.
- Rustfmt, Clippy all-targets com `-D warnings`, Rustdoc privado com `-D warnings` e build release passaram.
- **37 casos JavaScript** passaram contra dart2js O2 do SDK 3.6.2, com constantes e fusão solicitadas.
- **34 execuções nativas O0/O2** passaram contra Dart VM/AOT, com GC forçado, estatísticas e fusão, incluindo mixins com campos/getters.
- O exemplo de modificadores e o fixture com imports produziram saída idêntica no Dart VM e Node.

Relatórios completos: [JavaScript](dados/conformance-js-increment-14.json) e
[nativo](dados/conformance-native-increment-14.json). Essas verificações demonstram a
conformidade dos casos exercitados, não suporte completo à linguagem ou vantagem
de desempenho sobre DDC/dart2js.
