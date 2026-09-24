class A {
  int add(int a, int b) => a + b;
//    ^^^
// [context 1] The member being overridden.
}
mixin M {
  int add();
}
class B	extends A with M {}
//    ^
// [diag.invalidImplementationOverride] 'A.add' ('int Function(int, int)') isn't a valid concrete implementation of 'M.add' ('int Function()').
//                     ^
// [diag.invalidOverride][context 1] 'M.add' ('int Function()') isn't a valid override of 'A.add' ('int Function(int, int)').
