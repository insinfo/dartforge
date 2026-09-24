class _A {
  static void m() {}
//            ^
// [diag.unusedElement] The declaration 'm' isn't referenced.
}
void f(_A a) {}
