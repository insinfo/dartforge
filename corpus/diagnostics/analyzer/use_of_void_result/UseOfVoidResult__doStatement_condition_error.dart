void f(void x) {
  do {} while (x);
//             ^
// [diag.useOfVoidResult] This expression has a type of 'void' so its value can't be used.
}
