class A {
  int p1;
  A(p1);
  augment A(this.p1);
//          ^^^^^^^
// [diag.fieldInitializingFormalNotAssignable] The parameter type 'dynamic' is incompatible with the field type 'int'.

}
