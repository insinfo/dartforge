void f() {}
class A {
  n() {
    Object a = f();
//         ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
//             ^
// [diag.useOfVoidResult] This expression has a type of 'void' so its value can't be used.
  }
}
