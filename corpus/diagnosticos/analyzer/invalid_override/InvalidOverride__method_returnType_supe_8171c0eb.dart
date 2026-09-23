class A {
  int foo() => 0;
//    ^^^
// [context 1] The member being overridden.
}

class B {
  String foo() => '';
//       ^^^
// [diag.invalidOverride][context 1] 'B.foo' ('String Function()') isn't a valid override of 'A.foo' ('int Function()').
}

augment class B extends A {}
