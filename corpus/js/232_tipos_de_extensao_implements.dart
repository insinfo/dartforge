// Tipo de extensão que implementa outro (membro herdado resolvido no
// declarante) e o tipo da representação; `is`/`as` apagados.
extension type Base(int v) {
  int get dobro => v * 2;
  String descreve() => 'base $v';
}

extension type Filho(int v) implements Base {
  int get triplo => v * 3;
  int soma() => dobro + triplo;
  Base comoBase() => this;
}

extension type Lista<T>(List<T> l) implements Iterable<T> {
  T get segundo => l[1];
}

String mostra(Base b) => b.descreve();

void main() {
  var f = Filho(2);
  print(f.dobro);
  print(f.soma());
  print(mostra(f));
  print(f.comoBase().dobro);
  var l = Lista([1, 2, 3]);
  print(l.segundo);
  print(l.length);
  print(l.map((x) => x * 2).toList());
  for (var x in l) {
    print(x);
  }
  Object? o = f;
  print(o is int);
  print((o as Filho).triplo);
  var fs = <Filho>[Filho(1), Filho(5)];
  print(fs.map((x) => x.triplo).toList());
  print(fs.runtimeType);
}
