class C {
  C(int x, int y);
}
void f() {
  new C(;
//      ^
// [diag.missingIdentifier] Expected an identifier.
// [diag.expectedToken] Expected to find ')'.
}
