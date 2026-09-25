// Poda por tipo do receptor: `toca` chamado só em `Sopro` não mantém `toca` de `Flauta`.
class Sopro {
  void toca() => print('sopro');
}

class Gaita extends Sopro {
  void toca() => print('gaita');
}

class Flauta {
  void toca() => print('flauta');
  void assobia() => print('assobio');
  String toString() => 'flauta!';
}

void main() {
  Sopro s = Gaita();
  s.toca();
  print(Flauta());
}
