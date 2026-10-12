// Tear-off de construtor por um alias que não só repassa os parâmetros
// (`typedef F<T> = C<int, T>`): `F.new` é a função genérica
// `C<int, T> Function<T>(…)` nos parâmetros do alias, e o contexto a
// instancia implicitamente (antes saía `C<int, dynamic>`).
class C<A, B> {
  final B? b;
  C([this.b]);
  C.nomeado(this.b);
  factory C.fab(B b) => C(b);
  @override
  String toString() => 'C<$A, $B>($b)';
}

typedef F<T> = C<int, T>;
typedef G<T> = C<T, String>;
typedef H<X, Y> = C<Y, X>;
typedef P<X, Y> = C<X, Y>;
typedef L<T extends num> = C<T, List<T>>;

void main() {
  C<int, String> Function(String) f1 = F.new;
  print(f1('a'));
  C<int, String> Function(String) f2 = F.nomeado;
  print(f2('b'));
  C<int, double> Function(double) f3 = F.fab;
  print(f3(1.5));
  C<bool, String> Function([String?]) g1 = G.new;
  print(g1('c'));
  C<int, String> Function([String?]) h1 = H.new;
  print(h1('d'));
  C<int, String> Function([String?]) p1 = P.new;
  print(p1('e'));
  C<int, List<int>> Function(List<int>) l1 = L.nomeado;
  print(l1([1, 2]));
  print(f1.runtimeType);
  print(h1.runtimeType);
  Object o = f1;
  print(o is C<int, String> Function(String));
  print(o is C<int, Object> Function(Never));
  print(o is C<String, String> Function(String));
  // Sem contexto: a função genérica; a chamada infere pelos argumentos.
  final gen = F.nomeado;
  print(gen('x'));
  print(gen<num>(3));
  print(gen.runtimeType);
  // O tear-off genérico da própria classe: a assinatura genérica.
  final dn = C.nomeado;
  print(dn.runtimeType);
  print(dn<int, String>('y'));
}
