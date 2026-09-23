class A {
  String foo() => '';
//       ^^^
// [context 1] The member being overridden.
}

mixin M on A {
  int foo() => 0;
//    ^^^
// [diag.invalidOverride][context 1] 'M.foo' ('int Function()') isn't a valid override of 'A.foo' ('String Function()').
}
