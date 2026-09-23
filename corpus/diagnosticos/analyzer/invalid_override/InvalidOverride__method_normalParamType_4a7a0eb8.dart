abstract class I<U> {
  void m(U u) => null;
//     ^
// [context 1] The member being overridden.
}
abstract class J<V> {
  void m(V v) => null;
//     ^
// [context 2] The member being overridden.
}
class B implements I<int>, J<String> {
  void m(double d) {}
//     ^
// [diag.invalidOverride][context 1] 'B.m' ('void Function(double)') isn't a valid override of 'I.m' ('void Function(int)').
// [diag.invalidOverride][context 2] 'B.m' ('void Function(double)') isn't a valid override of 'J.m' ('void Function(String)').
}
