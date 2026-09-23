class A {}
class B extends A {}
class C extends D {}
class D {}

void f<X extends A, Y extends B>(X x) {
  if (x is Y) {
    A a = x;
//    ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
    B b = x;
//    ^
// [diag.unusedLocalVariable] The value of the local variable 'b' isn't used.
    X x2 = x;
//    ^^
// [diag.unusedLocalVariable] The value of the local variable 'x2' isn't used.
    Y y = x;
//    ^
// [diag.unusedLocalVariable] The value of the local variable 'y' isn't used.
  }
}
