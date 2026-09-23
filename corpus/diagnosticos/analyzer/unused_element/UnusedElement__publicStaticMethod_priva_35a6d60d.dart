mixin _A {
  static void m() {}
//            ^
// [diag.unusedElement] The declaration 'm' isn't referenced.
}
void main() {
  _A;
}
