class A {
  void _m1() {}
//     ^^^
// [diag.unusedElement] The declaration '_m1' isn't referenced.
}
class B {
  void public() => _m1();
  void _m1() {}
}
