abstract class I {
  int get getter => 0;
//        ^^^^^^
// [context 1] The member being overridden.
}
abstract class J {
  num get getter => 0;
}
abstract class A implements I, J {}
class B extends A {
  String get getter => '';
//           ^^^^^^
// [diag.invalidOverride][context 1] 'B.getter' ('String Function()') isn't a valid override of 'I.getter' ('int Function()').
}
