abstract class I {
  set setter14(int _) => null;
}
abstract class J {
  set setter14(num _) => null;
//    ^^^^^^^^
// [context 1] The setter being overridden.
}
abstract class A extends I implements J {}
class B extends A {
  set setter14(String _) => null;
//    ^^^^^^^^
// [diag.invalidOverrideSetter][context 1] The setter 'B.setter14' ('void Function(String)') isn't a valid override of 'J.setter14' ('void Function(num)').
}
