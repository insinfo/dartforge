// switch expression como valor: atribuição, argumento, retorno, interpolação, guardas, records, _, aninhados.
enum Naipe { copas, ouros, espadas, paus }

String cor(Naipe n) => switch (n) {
      Naipe.copas || Naipe.ouros => 'vermelho',
      Naipe.espadas || Naipe.paus => 'preto',
    };

int pontos(String carta) => switch (carta) {
      'A' => 11,
      'K' || 'Q' || 'J' => 10,
      var s when int.tryParse(s) != null => int.parse(s),
      _ => 0,
    };

String fizzbuzz(int i) => switch ((i % 3, i % 5)) {
      (0, 0) => 'FizzBuzz',
      (0, _) => 'Fizz',
      (_, 0) => 'Buzz',
      _ => '$i',
    };

String comparaTres(int a, int b, int c) => switch ((a.compareTo(b), b.compareTo(c))) {
      (-1, -1) => 'crescente',
      (1, 1) => 'decrescente',
      (0, 0) => 'iguais',
      _ => 'misto',
    };

Object? valorMisto(int i) => switch (i) {
      0 => 'zero',
      1 => 1,
      2 => [2],
      _ => null,
    };

void main() {
  final n = 7;
  final tamanho = switch (n) {
    < 5 => 'pequeno',
    < 10 => 'medio',
    _ => 'grande',
  };
  print(tamanho);
  print(switch (n) { 7 => 'sete', _ => 'nao sete' });
  print('n eh ${switch (n) { > 5 => 'mais que cinco', _ => 'ate cinco' }}');
  final lista = [1, 2, 3].map((x) => switch (x) { 1 => 'um', 2 => 'dois', _ => 'muitos' }).toList();
  print(lista);
  print(Naipe.values.map(cor).toList());
  print(['A', 'K', '7', 'x', '10'].map(pontos).toList());
  print(List.generate(15, (i) => fizzbuzz(i + 1)).join(' '));
  print(comparaTres(1, 2, 3));
  print(comparaTres(3, 2, 1));
  print(comparaTres(2, 2, 2));
  print(comparaTres(1, 3, 2));

  for (var i = 0; i < 4; i++) {
    print(valorMisto(i));
  }

  final Object o = (nome: 'ana', idade: 30);
  final saudacao = switch (o) {
    (nome: String nome, idade: int idade) when idade >= 18 => 'ola $nome, adulta',
    (nome: String nome, idade: _) => 'ola $nome',
    _ => 'quem?',
  };
  print(saudacao);

  final aninhado = switch (n) {
    > 5 => switch (n.isOdd) {
        true => 'grande impar',
        false => 'grande par',
      },
    _ => 'pequeno',
  };
  print(aninhado);

  final valor = switch (n) { 7 => 1.5, _ => 2.5 } * 3;
  print(valor);
  final num numero = switch (n) { 7 => 3, _ => 0.5 };
  print(numero);
  final double d = switch (n) { 7 => 0.25, _ => 0.75 };
  print(d);
  final String? talvez = switch (n) { 7 => null, _ => 'x' };
  print(talvez);
  final int inteiro = switch (n) { 7 => n * 2, _ => 0 };
  print(inteiro);

  bool ehVogal(String c) => switch (c.toLowerCase()) {
        'a' || 'e' || 'i' || 'o' || 'u' => true,
        _ => false,
      };
  print('Dart'.split('').map(ehVogal).toList());
  print('programa'.split('').where(ehVogal).length);

  final resultados = <String>[];
  for (final v in [null, 1, 'dois', 3.5, [4], (5, 6), true]) {
    resultados.add(switch (v) {
      null => 'nulo',
      int i => 'int $i',
      String s => 'str $s',
      double x => 'double $x',
      [var unico] => 'lista de um $unico',
      (int a, int b) => 'record ${a + b}',
      bool b => 'bool $b',
      _ => 'outro',
    });
  }
  print(resultados.join(', '));

  final f = (int x) => switch (x) { 0 => 'z', _ => 'nz' };
  print(f(0) + f(1));
  print(switch (true) { true => 'sempre', false => 'nunca' });
  print(switch ('abc') { String(length: 3) => 'tres', _ => 'outro' });
}
