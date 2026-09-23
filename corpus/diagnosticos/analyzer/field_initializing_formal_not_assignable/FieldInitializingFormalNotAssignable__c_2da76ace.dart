class A {
  int x;
  A(String this.x) {}
//  ^^^^^^^^^^^^^
// [diag.fieldInitializingFormalNotAssignable] The parameter type 'String' is incompatible with the field type 'int'.
}
