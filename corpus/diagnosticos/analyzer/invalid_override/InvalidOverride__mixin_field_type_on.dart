class A {
  String foo = '';
//       ^^^
// [context 1] The member being overridden.
// [context 2] The setter being overridden.
}

mixin M on A {
  int foo = 0;
//    ^^^
// [diag.invalidOverride][context 1] 'M.foo' ('int Function()') isn't a valid override of 'A.foo' ('String Function()').
// [diag.invalidOverrideSetter][context 2] The setter 'M.foo' ('void Function(int)') isn't a valid override of 'A.foo' ('void Function(String)').
}
