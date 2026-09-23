mixin M {
  int add(int a, int b) => a + b;
//    ^^^
// [context 1] The member being overridden.
}
class A with M {
//    ^
// [diag.invalidImplementationOverride] 'M.add' ('int Function(int, int)') isn't a valid concrete implementation of 'A.add' ('int Function()').
  int add();
//    ^^^
// [diag.invalidOverride][context 1] 'A.add' ('int Function()') isn't a valid override of 'M.add' ('int Function(int, int)').
}
