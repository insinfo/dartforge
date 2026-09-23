class A {
  int _f(int p) => 7;
//    ^^
// [diag.unusedElement] The declaration '_f' isn't referenced.
}
/// This is similar to [A._f].
int g() => 7;
