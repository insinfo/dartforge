enum E {
  v('');
//  ^^
// [diag.constConstructorParamTypeMismatch] A value of type 'String' can't be assigned to a parameter of type 'int' in a const constructor.
  final int x;
  const E(String this.x);
//        ^^^^^^^^^^^^^
// [diag.fieldInitializingFormalNotAssignable] The parameter type 'String' is incompatible with the field type 'int'.
}
