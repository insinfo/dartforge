class A {
  void set b(_) {}
//         ^
// [context 1] The setter being overridden.
}
class B implements A {
  @override
  int b = 0;
//    ^
// [diag.invalidOverrideSetter][context 1] The setter 'B.b' ('void Function(int)') isn't a valid override of 'A.b' ('void Function(dynamic)').
}
