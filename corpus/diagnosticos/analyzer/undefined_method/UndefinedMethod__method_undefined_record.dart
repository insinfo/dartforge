void f() {
  var x = 1;
  (x,).foo();
//     ^^^
// [diag.undefinedMethod] The method 'foo' isn't defined for the type '(int,)'.
}
