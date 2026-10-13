class A	{
  int add(int a, int b) => a + b;
//    ^^^
// [context 1] The member being overridden.
}
class B	extends A {
//    ^
// [diag.invalidImplementationOverride] 'A.add' ('int Function(int, int)') isn't a valid concrete implementation of 'B.add' ('int Function()').
  int add();
//    ^^^
// [diag.invalidOverride][context 1] 'B.add' ('int Function()') isn't a valid override of 'A.add' ('int Function(int, int)').
}
