void f() {}
class A {
  n() {
    void a = f();
//       ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
  }
}
