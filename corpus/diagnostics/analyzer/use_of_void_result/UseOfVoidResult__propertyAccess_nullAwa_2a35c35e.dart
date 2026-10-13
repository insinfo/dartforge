void f(void x) {
  x?.foo;
//   ^^^
// [diag.useOfVoidResult] This expression has a type of 'void' so its value can't be used.
}
