void f() {}
class A {
  n() {
    var a;
//      ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
    a = f();
//      ^
// [diag.useOfVoidResult] This expression has a type of 'void' so its value can't be used.
  }
}
