void f(List list, void x) {
  list[x] = null;
//     ^
// [diag.useOfVoidResult] This expression has a type of 'void' so its value can't be used.
}
