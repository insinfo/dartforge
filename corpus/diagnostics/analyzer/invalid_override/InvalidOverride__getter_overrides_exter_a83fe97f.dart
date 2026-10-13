class A {
  external final int x;
//                   ^
// [context 1] The member being overridden.
}
abstract class B implements A {
  num get x;
//        ^
// [diag.invalidOverride][context 1] 'B.x' ('num Function()') isn't a valid override of 'A.x' ('int Function()').
  void set x(num value);
}
