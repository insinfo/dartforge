class A {
  set foo(String _) {}
//    ^^^
// [context 1] The setter being overridden.
}

mixin M on A {
  set foo(int _) {}
//    ^^^
// [diag.invalidOverrideSetter][context 1] The setter 'M.foo' ('void Function(int)') isn't a valid override of 'A.foo' ('void Function(String)').
}
