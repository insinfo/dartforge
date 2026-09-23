void f(void x) {
  x[0];
// ^^^
// [diag.useOfVoidResult] This expression has a type of 'void' so its value can't be used.
}
