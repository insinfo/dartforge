var A = 0;
void f() {
  new A();
//    ^
// [diag.newWithNonType] The name 'A' isn't a class.
}
