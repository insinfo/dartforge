class A {
  int x;
  A(dynamic this.x) {}
//  ^^^^^^^^^^^^^^
// [diag.fieldInitializingFormalNotAssignable] The parameter type 'dynamic' is incompatible with the field type 'int'.
}
