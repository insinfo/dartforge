void f() {
  num? x;
  x as int;
//^
// [diag.castFromNullableAlwaysFails] This cast will always throw an exception because the nullable local variable 'x' is not assigned.
}
