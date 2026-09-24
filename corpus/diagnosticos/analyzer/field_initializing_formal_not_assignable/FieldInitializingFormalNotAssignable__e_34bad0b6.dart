enum E {
  v(0);
  final int x;
  const E(dynamic this.x);
//        ^^^^^^^^^^^^^^
// [diag.fieldInitializingFormalNotAssignable] The parameter type 'dynamic' is incompatible with the field type 'int'.
}
