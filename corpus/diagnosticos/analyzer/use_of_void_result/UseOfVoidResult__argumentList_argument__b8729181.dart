void f(void x) {
  g(x);
//  ^
// [diag.useOfVoidResult] This expression has a type of 'void' so its value can't be used.
}
void g(dynamic x) { }
