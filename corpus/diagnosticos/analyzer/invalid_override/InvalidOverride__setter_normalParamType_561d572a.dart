abstract class I<U> {
  set s(U u) {}
//    ^
// [context 1] The setter being overridden.
}
abstract class J<V> {
  set s(V v) {}
//    ^
// [context 2] The setter being overridden.
}
class B implements I<int>, J<String> {
  set s(double d) {}
//    ^
// [diag.invalidOverrideSetter][context 1] The setter 'B.s' ('void Function(double)') isn't a valid override of 'I.s' ('void Function(int)').
// [diag.invalidOverrideSetter][context 2] The setter 'B.s' ('void Function(double)') isn't a valid override of 'J.s' ('void Function(String)').
}
