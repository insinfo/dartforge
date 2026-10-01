// Extensão genérica sobre um tipo registro: os argumentos de tipo da
// extensão saem do tipo estático do receptor campo a campo (o
// `toSequenceParser` do petitparser; docs/NATIVO-PROJETOS-REAIS.md, C16).

class P<T> {}

class S2<A, B> extends P<(A, B)> {
  final P<A> a;
  final P<B> b;
  S2(this.a, this.b);
}

extension E2<A, B> on (P<A>, P<B>) {
  P<(A, B)> seq() => S2<A, B>(this.$1, this.$2);
  List<A> lista() => <A>[];
}

extension EN<X> on ({P<X> p, int n}) {
  List<X> repetir() => <X>[];
}

void main() {
  final p = (P<String>(), P<int>()).seq();
  print(p.runtimeType);
  print(p is P<(String, int)>);
  print((P<String>(), P<int>()).lista().runtimeType);
  print((p: P<double>(), n: 2).repetir().runtimeType);
  final f = (P<bool>(), P<int>()).seq;
  print(f().runtimeType);
}
