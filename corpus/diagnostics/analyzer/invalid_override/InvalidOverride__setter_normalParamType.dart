class A {
  void set s(int v) {}
//         ^
// [context 1] The setter being overridden.
}
class B extends A {
  void set s(String v) {}
//         ^
// [diag.invalidOverrideSetter][context 1] The setter 'B.s' ('void Function(String)') isn't a valid override of 'A.s' ('void Function(int)').
}
