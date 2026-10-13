void f(void x) {
  for (x in [1, 2]) {}
//     ^
// [diag.useOfVoidResult] This expression has a type of 'void' so its value can't be used.
}
