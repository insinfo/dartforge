// requer-dart: 3.13
// Inferência usando bounds (3.7): a restrição do bound declarado entra na
// solução. `f(C())` infere `X = B`, porque `C <: B <: A<B>`; antes da 3.7
// era erro.
class A<X extends A<X>> {}

class B extends A<B> {}

class C extends B {}

String f<X extends A<X>>(X x) => 'X=$X';

class Caixa<T extends Comparable<T>> {
  final T v;
  Caixa(this.v);
  String get tipo => '$T';
}

String g<T extends num>(T? t) => 'T=$T';

List<X> lista<X extends Object>(X? x) => [if (x != null) x];

void main() {
  print(f(B()));
  print(f(C()));
  print(f<B>(C()));
  print(Caixa(3).tipo);
  print(Caixa('s').tipo);
  print(g(1));
  print(g(1.5));
  print(lista(1).runtimeType == List<int>);
}
