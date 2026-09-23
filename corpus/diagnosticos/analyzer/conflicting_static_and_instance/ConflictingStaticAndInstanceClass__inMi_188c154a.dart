mixin M {
  void foo() {}
}
class B extends Object with M {
  static void foo() {}
//            ^^^
// [diag.conflictingStaticAndInstance] Class 'B' can't define static member 'foo' and have instance member 'M.foo' with the same name.
}
