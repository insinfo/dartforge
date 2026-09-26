// Extensões genéricas: T da extensão reificado (como expressão, em is, em closures), inferido pelo
// tipo estático do receptor; membros da extensão chamados sem receptor; setters e operadores.
extension E<T> on List<T> {
  Type get t => T;
  bool eh(Object o) => o is T;
  List<R> mapear<R>(R Function(T) f) => [for (final x in this) f(x)];
  String descr() => '$T ${t} ${eh(1)} ${mapear<String>((x) => '$x').runtimeType}';
  Type viaClosure() => (() => T)();
}
extension on int { String get dobro => '${this * 2}'; }
Type f<T>() => T;
class C<T> { Type get t => T; Type viaFn() => (() => T)(); }
void main() {
  print(f<int>());
  print(C<String>().t);
  print(C<double>().viaFn());
  print(<int>[1].t);
  print(<String>['a'].eh(1));
  print(<String>['a'].eh('b'));
  print(<int>[1, 2].mapear((x) => x * 1.5));
  print(<int>[1, 2].mapear((x) => x * 1.5).runtimeType);
  print(<num>[1].descr());
  print(<bool>[true].viaClosure());
  print(21.dobro);
  List<Object> l = <int>[3];
  print(l.t);
  acessos();
}

class Caixa { int v = 1; }
extension Acesso on Caixa {
  int get dobro => v * 2;
  set dobro(int x) => v = x ~/ 2;
  int operator [](int i) => v + i;
  operator []=(int i, int x) => v = x - i;
  Caixa operator +(int n) => Caixa()..v = v + n;
}
void acessos() {
  final c = Caixa();
  c.dobro = 10;
  print(c.v);
  c.dobro += 4;
  print(c.v);
  print(c[3]);
  c[2] = 9;
  print(c.v);
  c[1] += 5;
  print(c.v);
  print((c + 4).v);
}
