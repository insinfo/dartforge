void f<U>() {}
void g1<U>(U a, [num b = 0, List<U> c = const []]) {}
void g2<U>(U a, [int b = 0, Map<U, U> c = const {}]) {}
class M<U> {
  final void Function(U, [int, Never]) x7;
  const M(bool b) : x7 = b ? g1 : g2;
}
const m = const M<String>(true);
