mixin M {
  set foo(int _) {}
}

enum E with M {
  v;
  static void foo() {}
//            ^^^
// [diag.conflictingStaticAndInstance] Class 'E' can't define static member 'foo' and have instance member 'E.foo' with the same name.
}
