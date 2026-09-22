// Construtor const: canonicalização, identical, const em coleções, const implícito.
class Par {
  final int a;
  final int b;
  const Par(this.a, this.b);
  @override
  String toString() => 'Par($a, $b)';
}

class Caixa {
  final Par par;
  final String rotulo;
  const Caixa(this.par, this.rotulo);
  @override
  String toString() => 'Caixa($par, $rotulo)';
}

class Nivel {
  final int valor;
  final String nome;
  const Nivel._(this.valor, this.nome);

  static const baixo = Nivel._(1, 'baixo');
  static const medio = Nivel._(2, 'medio');
  static const alto = Nivel._(3, 'alto');
  static const todos = [baixo, medio, alto];

  @override
  String toString() => 'Nivel.$nome';
}

class Unidade {
  const Unidade();
}

const parGlobal = Par(1, 2);

void main() {
  print(identical(const Par(1, 2), const Par(1, 2)));
  print(identical(const Par(1, 2), const Par(2, 1)));
  print(identical(Par(1, 2), Par(1, 2)));
  print(identical(const Par(1, 2), Par(1, 2)));
  print(identical(parGlobal, const Par(1, 2)));

  const lista = [Par(1, 2), Par(3, 4)];
  print(lista);
  print(identical(lista[0], const Par(1, 2)));
  print(identical(lista, const [Par(1, 2), Par(3, 4)]));

  const mapa = {'um': Par(1, 1), 'dois': Par(2, 2)};
  print(mapa);
  print(identical(mapa['um'], const Par(1, 1)));

  const caixa = Caixa(Par(5, 6), 'c');
  print(caixa);
  print(identical(caixa.par, const Par(5, 6)));
  print(identical(caixa, const Caixa(Par(5, 6), 'c')));

  const rec = (Par(7, 8), nome: 'r');
  print(rec);
  print(identical(rec.$1, const Par(7, 8)));

  final Par implicito = const Par(9, 9);
  print(identical(implicito, const Par(9, 9)));

  print(Nivel.todos);
  print(Nivel.medio.valor);
  print(identical(Nivel.todos[2], Nivel.alto));
  print(Nivel.todos.map((n) => n.nome).join(','));

  print(identical(const Unidade(), const Unidade()));
  print(identical(Unidade(), Unidade()));
  const conjunto = {Par(1, 1), Par(2, 2)};
  print(conjunto.length);
  print(identical(const [1, 2], const [1, 2]));
  print(identical(const <int>[], const <int>[]));
}
