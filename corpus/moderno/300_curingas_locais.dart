// requer-dart: 3.13
// Curingas (3.7): `_` local, parâmetro, catch, for e parâmetro de tipo não
// ligam nome; o inicializador roda; ler `_` resolve para o `_` de topo.
var _ = 'o _ de topo liga nome';

int calc(String quem) {
  print('calc $quem');
  return 7;
}

String tipo<_>(Object o) => 'tipo<_> recebeu $o';

class Par<_, _> {
  final String rotulo;
  Par(this.rotulo);
}

typedef Dois = void Function(int _, int _);

void main() {
  var _ = calc('a');
  var _ = calc('b');
  int _ = 3;
  final _ = calc('c');
  print(_);

  void g(int _, int _) => print('g ignora os dois');
  g(1, 2);
  Dois d = (_, _) => print('tipo de função com dois curingas');
  d(3, 4);

  [10, 20].forEach((_) => print('item'));
  var soma = 0;
  [1, 2, 3].asMap().forEach((_, v) => soma += v);
  print('soma $soma');

  try {
    throw StateError('x');
  } catch (_, _) {
    print('catch (_, _)');
  }
  try {
    throw 'y';
  } on String catch (_) {
    print('on String catch (_)');
  }

  for (var _ in [1, 2]) {
    print('for-in _');
  }
  for (var _ = 0, i = 0; i < 2; i++) {
    print('for clássico $i');
  }
  var lista = [for (var _ in [1, 2, 3]) 'x'];
  print(lista);

  print(tipo<int>(5));
  print(Par<int, String>('par').rotulo);

  // Padrões sempre foram curinga; continuam.
  var (a, _, c) = (1, 2, 3);
  print('$a $c');

  // Função local chamada `_` não liga nome: `_` continua o de topo.
  void _() => print('nunca chamada');
  print(_);
}
