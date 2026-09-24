void f(void x) {
  x ? null : null;
//^
// [diag.useOfVoidResult] This expression has a type of 'void' so its value can't be used.
}
