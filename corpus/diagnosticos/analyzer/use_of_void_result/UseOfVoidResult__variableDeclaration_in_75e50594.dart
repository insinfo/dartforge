f() {}
class A {
  n() {
    var a = f();
//      ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
  }
}
