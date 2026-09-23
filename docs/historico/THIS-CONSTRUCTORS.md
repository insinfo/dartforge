# This implícito e construtores

O incremento 17 acrescenta construtores generativos sem nome, com parâmetros
posicionais obrigatórios, parâmetros inicializadores `this.campo` e corpo opcional.
A referência de linguagem continua sendo Dart **3.6.2**.

```dart
class Counter {
  int value;
  Counter(this.value) {
    value += 1;
  }
  int add(int amount) {
    value += amount;
    return value;
  }
}
void main() {
  print(Counter(2).add(3)); // 6
}
```

O mesmo mecanismo de resolução de membros implícitos atende classes comuns,
classes herdadas, enums e mixins já suportados. A análise registra quando uma
leitura, escrita ou chamada usa o receptor `this`; os backends consomem essa
resolução, sem decidir pelo nome isolado durante a emissão.

## Resolução de nomes

Dentro de um método ou corpo de construtor, variáveis locais e parâmetros comuns
têm precedência sobre membros da instância. Membros têm precedência sobre nomes
globais. `this.campo` acessa explicitamente o membro mesmo quando um parâmetro ou
local tem o mesmo nome.

```dart
class Counter {
  int value = 0;
  Counter(int value) {
    value += 1;        // parâmetro comum
    this.value = value; // campo recebe o parâmetro
  }
}
```

Uma declaração local oculta o nome externo em todo seu bloco. Por isso,
`print(value); int value = 2;` não usa o campo antes da declaração: produz
diagnóstico de uso antes da inicialização/declaração. `int value = value;` também
não recupera o campo ocultado. Um bloco interno pode declarar outro local e,
ao terminar, a resolução volta ao escopo externo.

O parâmetro inicializador `this.value` tem uma regra diferente: **não introduz
uma variável local chamada `value` no corpo**. Em `Counter(this.value) { value += 1; }`,
a atribuição modifica o campo. Uma variável local declarada nesse corpo ainda
pode ocultá-lo. Referências a `this` fora de contexto de instância são recusadas;
inicializadores de campos também não podem acessar `this`, explícito ou implícito.

## Inicialização definida

Um campo próprio pode ser inicializado na declaração ou por um parâmetro
inicializador do construtor. A análise verifica existência do campo, tipos,
duplicatas e quantidade de argumentos. `this.campo` não pode inicializar um campo
herdado.

- Campo não nullable sem inicializador precisa de um formal inicializador.
- Campo **mutável nullable** sem inicializador recebe `null`.
- Campo **final**, mesmo nullable, exige inicialização explícita na declaração
  ou no construtor.
- Um campo final inicializado na declaração não pode ser inicializado novamente
  por `this.campo`.
- Atribuir no corpo do construtor não substitui a inicialização exigida de um
  campo não nullable que não seja `late`. `late` não faz parte deste recorte.

Campos mutáveis podem ter inicializador e também formal inicializador:

```dart
int initial() {
  print(99);
  return 1;
}
class Counter {
  int value = initial();
  Counter(this.value);
}
void main() {
  print(Counter(2).value);
}
```

O oracle Dart 3.6.2 imprimiu `99` e depois `2`. O inicializador executa e seus
efeitos permanecem; em seguida o valor fornecido ao formal substitui o campo.
Remover essa avaliação alteraria o programa.

Parâmetros comuns do construtor não ficam em escopo dentro dos inicializadores
da declaração dos campos. Os argumentos da chamada são avaliados uma vez, na
ordem escrita. Na construção herdada, a implementação deve preservar a ordem
de inicialização Dart: inicialização da classe derivada antes da construção da
base, e corpo da base antes do corpo da derivada. A inicialização não deve ser
traduzida mecanicamente para uma ordem imposta pelo construtor JavaScript.

## Corpos e herança

O corpo pode usar os comandos já suportados, chamar métodos e criar closures.
`return;` encerra o construtor; retornar uma expressão é recusado. Closures
aninhadas possuem contexto próprio de retorno e controle de fluxo.

Sem chamada explícita à base, há uma chamada implícita a `super()` sem argumentos.
Como listas de inicialização e parâmetros `super` ainda não são suportados, uma
base cujo construtor exige argumentos produz diagnóstico explícito. O compilador
não inventa valores nem encaminha parâmetros automaticamente.

## Limites atuais

São suportados um construtor generativo sem nome por classe, parâmetros comuns
tipados e parâmetros inicializadores `this.campo` cujo tipo é obtido do campo próprio. Permanecem fora
do recorte: construtores nomeados, `factory`, redirecionamentos, construtores
`const` de classes comuns, parâmetros opcionais/nomeados, listas de inicialização,
`super(...)`, superparâmetros, a forma explicitamente tipada `int this.campo` e
construtores em declarações de mixins. O suporte específico a construtores const
de enums, introduzido anteriormente, não significa suporte geral a const classes.

A análise exige que aplicações `with` sejam normalizadas antes da validação.
Classes genéricas não são sintetizadas por esse trabalho. Recursos não
implementados recebem diagnóstico; não são ignorados para produzir código.

## Referências e verificações

Foram consultados no clone do SDK os arquivos da tag `3.6.2`, commit
`b0cc5495e0f5e8ae150825a5352e708cb49e65ff`, usando `git show`:

- `tests/language/initializing_formal/scope_test.dart`: formal inicializador não
  permanece como variável local no corpo; atribuição pelo nome modifica o campo.
- `tests/language/initializing_formal/access_test.dart`: acesso aos valores em
  inicialização e corpo, como referência para distinguir esses escopos.
- `tests/language/initializing_formal/type_test.dart`: tipos associados aos
  campos e aos parâmetros inicializadores, consultado pelo agente de frontend.

Experimentos originais foram analisados/executados com
`C:/tools/dartsdk-3.6.2/bin/dart.exe`, em `target/semantic-review/`:

- `constructors.dart`: confirmou erros para final nullable sem inicialização,
  não nullable inicializado apenas no corpo e final inicializado duas vezes.
- `constructor_mutable.dart`: confirmou que inicializador mais formal é válido
  para campo mutável.
- `constructor_mutable_effect.dart`: confirmou a sequência de efeitos `99`, `2`.

As regressões Rust estão em `crates/semantic/tests/constructors.rs`, cobrindo
casos positivos e negativos de tipos, escopos, retornos e herança. O trabalho usa
testes do SDK como referência de comportamento; não copia sua implementação para
os fontes MIT do projeto.
