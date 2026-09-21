# Incremento 24 — macro `@DataClass()` e emissão textual de augmentations

Este incremento acrescenta a segunda macro incorporada do DartForge e uma API de
inspeção que mostra, em texto Dart, exatamente o que a expansão injetou. Todo o
código está em `crates/macros`; nenhum outro crate foi alterado. As únicas mudanças
fora dele são as entradas de `Cargo.lock` para as três novas dev-dependencies do
crate (`dartforge-semantic`, `dartforge-hir` e `dartforge-codegen`), que existem para
que os testes fechem o ciclo até a emissão JavaScript.

Como em `@JsonCodable()`, `@DataClass()` é uma macro escrita em Rust, incorporada
ao compilador e explicitamente experimental. Ela não carrega macros de pacotes,
não executa código Dart em tempo de compilação e não implementa o protocolo do
protótipo histórico. Consulte `docs/IMPLEMENTACAO-21.md` para a infraestrutura de
expansão e `docs/MACROS-ARQUITETURA.md` para as barreiras de fase.

## A macro

```dart
@DataClass()
class Usuario {
  final String nome;
  final int idade;
  final String? apelido;
  final bool ativo;
}
void main() {
  var usuario = Usuario('Dart', 15, null, true);
  var maior = usuario.copyWith(null, 16, 'apelido', null);
  print(maior.idade);
  print(usuario.igualA(maior));
  print(maior.descrever());
}
```

A expansão acrescenta um construtor posicional (quando ausente) e três membros:

| Membro | Assinatura gerada |
| --- | --- |
| construtor | `Usuario(this.nome, this.idade, this.apelido, this.ativo)` |
| cópia | `Usuario copyWith(String? nome, int? idade, String? apelido, bool? ativo)` |
| igualdade | `bool igualA(Usuario outro)` |
| descrição | `String descrever()` |

A saída do exemplo é:

```text
16
false
Usuario(nome: Dart, idade: <int>, apelido: apelido, ativo: <bool>)
```

## Decisões de subconjunto e seus limites

As três assinaturas acima não são as que uma macro Dart completa produziria. Cada
desvio existe porque o subconjunto aceito hoje não oferece o recurso necessário.
Nenhuma delas é uma preferência de estilo, e todas devem ser revistas quando o
recurso correspondente entrar no subconjunto.

### `copyWith` usa posicionais anuláveis, e `null` significa "mantém"

O subconjunto não tem parâmetros nomeados nem opcionais. Um `copyWith` idiomático
(`copyWith({String? nome})`) é impossível de declarar. A macro gera, portanto, um
parâmetro **posicional e obrigatório** por campo, na ordem de declaração, com o
tipo do campo tornado anulável, e o corpo `return Usuario(nome ?? this.nome, ...)`.

Limites diretos dessa escolha:

- **Todos os argumentos são obrigatórios.** `u.copyWith('outro')` não compila em
  uma classe de quatro campos; é preciso escrever `u.copyWith('outro', null, null, null)`.
- **Um campo anulável não pode ser zerado.** Como `null` é o sinal de "mantém o
  valor atual", `copyWith(null, null, null, null)` devolve uma cópia idêntica e não
  há como pedir que `apelido` volte a ser `null`. Para isso, use o construtor.
- **A ordem posicional é parte do contrato.** Reordenar campos na classe muda
  silenciosamente o significado das chamadas existentes.

Quando o subconjunto ganhar parâmetros nomeados opcionais, a assinatura deve migrar
para `copyWith({String? nome, ...})` e o segundo limite desaparece apenas se a macro
passar a distinguir "ausente" de "null", o que exige um sentinel ou wrappers.

### A igualdade é `igualA`, não `operator ==`

O subconjunto não permite declarar `operator ==`, e `hashCode` está na lista de
membros reservados que a análise semântica rejeita. A macro gera um método comum
`bool igualA(Nome outro)` que compara todos os campos com `==` e `&&`, associando
à esquerda; uma classe sem campos devolve `true`.

Limites diretos:

- **`a == b` continua sendo identidade.** A macro não muda o comportamento de `==`,
  de coleções que usam igualdade, nem de nada que dependa de `hashCode`.
- **A comparação é rasa.** Só há campos escalares, então isso hoje é igualdade de
  valor; se o subconjunto aceitar campos de classe, `igualA` passará a comparar
  identidade e a regra precisará ser revista.
- **O nome é do DartForge**, não do Dart. É português para deixar claro que não é
  uma implementação de `operator ==`.

### `descrever()` só mostra valores `String`

Esta foi a decisão que mais reduziu escopo, e ela decorre de três ausências
simultâneas no subconjunto, verificadas na implementação atual do backend:

1. `+` exige **dois `int` ou dois `String`** (`crates/semantic/src/lib.rs`,
   `BinaryOp::Add`). `'x' + 1` é um erro semântico, não uma conversão implícita.
2. `toString`, `hashCode`, `runtimeType` e `noSuchMethod` são nomes reservados que
   a análise rejeita; não há `1.toString()` nem `int.parse` inverso.
3. O lexer rejeita interpolação de strings explicitamente
   (`crates/lexer/src/lib.rs`: "string interpolation is not supported").

Não existe, portanto, **nenhuma** forma de converter `int` ou `bool` em texto dentro
do subconjunto. Em vez de gerar código que não compila, ou de omitir os campos e
produzir um formato que muda conforme os tipos declarados, a macro mantém o formato
`Nome(campo: valor, ...)` estável e substitui o valor que não sabe imprimir por um
marcador do tipo declarado:

| Tipo do campo | Texto produzido |
| --- | --- |
| `String` | o valor do campo |
| `String?` | o valor do campo, ou `null` quando ausente (via `?? 'null'`) |
| `int` | `<int>` |
| `int?` | `<int?>` |
| `bool` | `<bool>` |
| `bool?` | `<bool?>` |

Limites diretos:

- **`descrever()` não é `toString()`** e não serve para serializar ou comparar
  estado; para dois objetos que só diferem em campos `int`, o texto é idêntico.
- **O marcador é visível de propósito.** `<int>` no meio da saída é o sinal de que
  o subconjunto ainda não sabe imprimir aquele campo.
- Literais adjacentes são fundidos antes da materialização, então uma classe sem
  campos `String` produz um único literal, sem nenhuma concatenação.

Quando houver interpolação ou `toString()`, o marcador deve dar lugar ao valor e o
formato permanece o mesmo.

## Coexistência com `@JsonCodable()`

As duas macros **podem ser aplicadas à mesma classe**, em qualquer ordem, e isso é
testado. A decisão se apoia em três fatos:

- Os conjuntos de nomes gerados são disjuntos: `fromJson`/`toJson` contra
  `copyWith`/`igualA`/`descrever`.
- As duas exigem exatamente a mesma forma de classe (concreta, sem herança, mixins,
  interfaces ou enum; campos escalares sem inicializador).
- As duas precisam do mesmo construtor posicional com initializing formals, que é
  gerado **uma única vez** e compartilhado.

Não há, portanto, combinação incompatível a diagnosticar: a única falha possível ao
combiná-las é uma colisão com um membro já escrito, e cada conjunto é conferido
contra a anotação que o reservou. Em
`@JsonCodable() @DataClass() class C { int copyWith() => 1; }` o erro aponta
`@DataClass()`; em `@DataClass() @JsonCodable() class C { int toJson() => 1; }`
aponta `@JsonCodable()`.

A ordem das anotações não altera nada do que é gerado: as duas ordens produzem ASTs
idênticas. A anotação usada nos erros compartilhados (elegibilidade, campos,
construtor) é `@JsonCodable()` quando presente, `@DataClass()` caso contrário.

## Contrato de expansão

- Classe concreta, sem superclasse explícita, mixins, interfaces ou valores de enum;
  `mixin` e `mixin class` não são alvos válidos.
- Campos `int`, `bool`, `String` e suas formas anuláveis, sem inicializadores.
- Construtor posicional gerado quando ausente. Um construtor existente precisa
  inicializar todos os campos na ordem declarada, com `this.campo` e corpo vazio.
- Aplicação duplicada da mesma macro é erro.
- Qualquer membro já declarado com um nome reservado pela macro é erro, inclusive
  campos, métodos, métodos abstratos e fábricas.
- A expansão é atômica: uma falha em qualquer classe deixa a AST original intacta,
  com as anotações preservadas. O caminho sem macros não clona a AST.
- Cada nó gerado recebe span exclusivo, fora do intervalo do arquivo, e proveniência
  na anotação. `ExpansionReport::remap` devolve o diagnóstico à anotação e prefixa a
  mensagem com o nome da macro responsável (`DataClass: ...`).
- As três barreiras globais são as mesmas: `Types` não gera nada, `Declarations`
  reserva todas as assinaturas de todas as classes e `Definitions` preenche os
  corpos. Nenhuma expansão é intercalada com a de outra classe.
- `applications` conta pares (classe, macro): uma classe com as duas anotações conta
  duas aplicações e um único construtor gerado.

### Diagnósticos

| Situação | Mensagem |
| --- | --- |
| duas `@DataClass()` | `Duplicate DataClass application` |
| herança, mixin, interface, enum, abstrata, `mixin`, `mixin class` | `DataClass currently requires a concrete class without inheritance, mixins or interfaces` |
| campo não escalar ou com inicializador | `DataClass requires scalar fields without initializers (int/bool/String, optionally nullable)` |
| membro já declarado | `DataClass conflicts with existing copyWith/igualA/descrever member` |
| construtor incompatível | `DataClass requires an empty constructor initializing every field in declaration order` |

Todos apontam a anotação, não o membro ofensor, porque é a anotação que autoriza a
geração e é lá que a correção acontece.

## Cache de planos

O plano continua sendo um esquema owned, sem AST, spans, nomes de classe ou IDs. As
macros aplicadas passam a fazer parte da chave: `MacroPlan` ganhou `json_codable` e
`data_class`, e a mesma lista de campos sob `@JsonCodable()`, sob `@DataClass()` e
sob as duas produz **três entradas distintas** no LRU. `PLAN_VERSION` foi para `2`;
planos gravados pela versão 1 descrevem apenas `JsonCodable` e não são reutilizáveis.

Um acerto de plano continua não evitando validação nem materialização: a AST, os
spans e a proveniência são novos a cada expansão, e o relatório de duas expansões da
mesma fonte é idêntico, com ou sem cache. Uma sessão com limites zerados erra sempre
e produz exatamente a mesma AST de uma sessão com cache.

## Emissão textual de augmentations

`dartforge_macros::augmentation_library(&program)` devolve o texto Dart equivalente
ao que a expansão injetaria, e `augmentations(&program)` devolve a mesma informação
estruturada por classe e por membro.

```text
augment class Usuario {
  augment Usuario(this.nome, this.idade);
  augment factory Usuario.fromJson(Map<String, Object?> json) {
    return Usuario(json['nome'] as String, json['idade'] as int);
  }
  augment Map<String, Object?> toJson() {
    return <String, Object?>{'nome': this.nome, 'idade': this.idade};
  }
}
```

**Este texto não volta ao compilador.** É material de inspeção, depuração e prova de
proveniência. A expansão real acontece em AST e nunca passa por reparsing; a sintaxe
`augment` sequer é aceita pelo parser do subconjunto, então gravar o resultado em
disco e compilá-lo não é um caminho suportado. A API existe para permitir ler,
revisar e versionar em texto os membros que a macro acrescentou.

Propriedades garantidas:

- **Determinismo byte a byte.** Mesma entrada, mesmo texto. Classes saem ordenadas
  por nome e, em empate, por identidade da declaração; os membros de cada classe
  saem em ordem canônica: construtor, fábricas por nome, métodos por nome. Nenhuma
  iteração depende de tabela hash, de spans ou da ordem do arquivo.
- **Mesma validação.** Os diagnósticos coincidem byte a byte com os da expansão.
- **Sem divergência de formato.** O texto de `descrever()` e a AST de `descrever()`
  são derivados da mesma sequência de pedaços, de modo que não podem discordar.
- A função recebe o programa **antes** de `expand`, porque a expansão consome as
  anotações; chamá-la depois devolve apenas o cabeçalho de comentário.

O texto escreve `Map<String, Object?>`, não `Map<String, dynamic>`: `dynamic` não
existe no subconjunto e o tipo realmente gerado é o reificado, com cast explícito por
campo. Nomes de campo são escapados no literal de `descrever()`, inclusive o `$` que
Dart aceita em identificadores e que iniciaria uma interpolação.

## Medições

Contagens determinísticas por classe, medidas pela própria expansão. `nodes` é
`materialized_nodes`, o número de spans sintéticos reservados; cada nó consome dois
bytes do espaço de spans, então o avanço de `extent` é sempre `2 * nodes`.

| Campos | Anotações | Declarações | Nós | Aplicações |
| ---: | --- | ---: | ---: | ---: |
| 1 | `@JsonCodable()` | 3 | 16 | 1 |
| 1 | `@DataClass()` | 4 | 21 | 1 |
| 1 | ambas | 6 | 35 | 2 |
| 4 | `@JsonCodable()` | 3 | 40 | 1 |
| 4 | `@DataClass()` | 4 | 69 | 1 |
| 4 | ambas | 6 | 104 | 2 |
| 16 | `@JsonCodable()` | 3 | 136 | 1 |
| 16 | `@DataClass()` | 4 | 249 | 1 |
| 16 | ambas | 6 | 368 | 2 |

`DataClass` materializa mais nós por campo que `JsonCodable` (≈15,5 contra ≈8,4 com
16 campos) porque gera três corpos em vez de dois, e `igualA` sozinha custa quatro
nós por campo.

Tempo por expansão de uma classe de 16 campos, medindo **apenas** a chamada de
expansão — parsing fica fora da janela. Corpus sintético de 64 fontes sobre 8
esquemas distintos, aquecimento explícito, melhor de 9 lotes, `--release`:

| Anotações | Sem cache | Com cache | Redução |
| --- | ---: | ---: | ---: |
| `@JsonCodable()` | 5,8 µs | 4,9 µs | 16% |
| `@DataClass()` | 12,2 µs | 9,2 µs | 24% |
| ambas | 16,5 µs | 14,3 µs | 13% |

Com cache, 632 das 640 expansões acertaram o plano. O ganho é modesto e deve ser: o
cache evita apenas copiar nomes de campos para um plano owned; validação e
materialização continuam acontecendo em todas as expansões. Estes números são de uma
máquina só (Intel Core i3-1215U, Windows 11, rustc 1.98.1) e servem para comparar as
três colunas entre si, não para anunciar desempenho absoluto. Eles não são
comparáveis com DDC ou dart2js, que expandem macros de um ecossistema inteiro.

## Testes

`crates/macros/tests/data_class.rs` cobre a expansão bem-sucedida e a forma de cada
membro, a classe sem campos, o formato de `descrever()` pedaço a pedaço, o construtor
existente reaproveitado, a coexistência com `@JsonCodable()` nas duas ordens, os
dezessete diagnósticos de elegibilidade e colisão com verificação do span na anotação
correta, a atomicidade de uma falha tardia, a proveniência que nomeia a macro,
o determinismo da expansão e do texto de aumento, a correspondência entre o texto e
a AST expandida, e o acerto, a falta e a expulsão do cache de planos.

Cada caso bem-sucedido atravessa lex, parse, expand, `expand_mixins`, análise
semântica e emissão JavaScript, de modo que um membro gerado mal tipado falha no
teste em vez de chegar ao backend. Um teste marcado `#[ignore]` executa o JavaScript
emitido em Node e confere a saída observável dos três membros, inclusive o caso em
que `copyWith(null, ...)` preserva um campo anulável.

## Próximos passos

- Parâmetros nomeados opcionais, para que `copyWith` deixe de ser posicional.
- Interpolação de strings ou conversão numérica para texto, para que `descrever()`
  imprima todos os campos.
- `operator ==` e `hashCode`, para substituir `igualA` por igualdade real.
- Campos de classe e coleções, com igualdade e descrição compostas.
- Hospedagem de macros de usuário e resolução por pacote, ainda pendentes desde o
  incremento 21.
