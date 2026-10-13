class A {
  void foo() {}
}
abstract class B implements A {
  static set foo(_) {}
//           ^^^
// [diag.conflictingStaticAndInstance] Class 'B' can't define static member 'foo' and have instance member 'A.foo' with the same name.
}
