class A {
  int get a => 0;
  void set b(_) {}
//         ^
// [context 1] The setter being overridden.
  int c = 0;
}
class B extends A {
  @override
  final int a = 1;
  @override
  int b = 0;
//    ^
// [diag.invalidOverrideSetter][context 1] The setter 'B.b' ('void Function(int)') isn't a valid override of 'A.b' ('void Function(dynamic)').
  @override
  int c = 0;
}