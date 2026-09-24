class A {}
class B extends A {
  set b(x) {}
}
f(a) {
  if (a is A) {
    a.b = 0;
//    ^
// [diag.undefinedSetter] The setter 'b' isn't defined for the type 'A'.
  }
}
