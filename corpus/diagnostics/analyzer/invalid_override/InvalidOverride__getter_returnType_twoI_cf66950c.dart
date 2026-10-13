abstract class I<U> {
  U get g => throw 0;
//      ^
// [context 1] The member being overridden.
}
abstract class J<V> {
  V get g => throw 0;
//      ^
// [context 2] The member being overridden.
}
class B implements I<int>, J<String> {
  double get g => throw 0;
//           ^
// [diag.invalidOverride][context 1] 'B.g' ('double Function()') isn't a valid override of 'I.g' ('int Function()').
// [diag.invalidOverride][context 2] 'B.g' ('double Function()') isn't a valid override of 'J.g' ('String Function()').
}
