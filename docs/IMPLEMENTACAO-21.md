# Incremento 21 — metaprogramação experimental e mapas

Dart 3.6.2 passa a ser registrado como **base mínima**, não um teto. Recursos
posteriores e macros estão autorizados no roteiro. A implementação deve identificar
quais contratos pertencem a cada versão e quais são extensões do DartForge.

## Primeira macro executável

`@JsonCodable()` é uma macro incorporada em Rust, explicitamente experimental.
A nova crate `dartforge-macros` expande a AST antes da análise semântica e antes
das otimizações. Ela gera declarações comuns, verificadas pelo mesmo sistema de
tipos da aplicação; não usa inspeção da anotação em tempo de execução.

```dart
@JsonCodable()
class Usuario {
  final String nome;
  final int idade;
  final String? apelido;
}
void main() {
  var usuario = Usuario.fromJson({'nome': 'Dart', 'idade': 15});
  print(usuario.nome);
  print(usuario.toJson()['idade'] as int);
}
```

Nesta etapa a anotação é incorporada ao compilador e dispensa import. Ela não
resolve nem finge implementar o antigo `package:json/json.dart`. O carregamento
de macros fornecidas por pacotes e a execução de código Dart arbitrário durante
a compilação continuam pendentes. Também não há suporte geral a `macro class`,
augmentations, consultas assíncronas ou ao protocolo de execução do protótipo.

## Contrato de expansão

- Classes concretas simples, sem superclasse explícita, mixins ou interfaces.
- Campos int, bool, String e formas nullable, sem inicializadores explícitos.
- Construtor posicional é gerado quando ausente. Um construtor existente precisa
  inicializar todos os campos na ordem declarada, com this.campo e corpo vazio.
- A fábrica `fromJson(Map<String, Object?> json)` lê cada campo e aplica um cast
  para seu tipo. Campos obrigatórios ausentes ou valores de tipo incorreto falham;
  campos nullable ausentes recebem null. Chaves extras são ignoradas.
- `toJson()` devolve Map<String, Object?> com as chaves originais, inclusive
  campos privados; valores null são incluídos. Não devolve texto JSON.
- Declarações fromJson/toJson existentes e aplicações duplicadas são erros.
- A expansão é atômica: falhas deixam a AST original intacta. O caminho sem macros
  não clona a AST. Cada nó gerado recebe span exclusivo e proveniência na anotação.
- O linker reserva espaço para os spans gerados de cada unidade. Diagnósticos
  voltam ao arquivo e à anotação que solicitaram a geração.

## Infraestrutura compartilhada

Mapas com chaves String usam JavaScript Map, preservando ordem e chaves como
`__proto__`. Leituras ausentes retornam null; escritas covariantes verificam os
tipos reais antes de alterar o mapa. Tipos são reificados. Literais preservam a
ordem de avaliação das chaves e valores. Const maps e a API completa de Map
permanecem pendentes.

Fábricas nomeadas com corpo de expressão passam a ser suportadas no JavaScript,
sem receptor this e sem acesso implícito a membros de instância. Essa infraestrutura
também funciona em declarações escritas manualmente. LLVM diagnostica mapas e
fábricas nomeadas; não há lowering nativo ou Wasm desses recursos nesta etapa.

## Referência e próximos passos

Referência local: `references/dart-macros`, commit
`89aecb1e3c10373ea795f003268d7cd48c9ed431`, especialmente
`pkgs/macro/lib/src/macro.dart`, `pkgs/macro/lib/src/builders.dart` e
`pkgs/_test_macros/lib/json_codable.dart`.
[Repositório do protótipo](https://dart.googlesource.com/macros/).

O experimento abandonado continua útil como referência, sem impedir a evolução
do DartForge. A macro desta etapa possui contrato próprio e limitado; não é
compatibilidade integral com toda a API experimental original.

Prioridades seguintes: hospedagem de macros de usuário, resolução por pacote,
cache de expansões por dependências, classes genéricas e serialização composta;
variáveis wildcard 3.7, elementos null-aware 3.8, dot shorthands 3.10, parâmetros
nomeados privados 3.12 e primary constructors 3.13; extension types e biblioteca
padrão. Cada incremento precisa de testes semânticos e diferenciais adequados.
