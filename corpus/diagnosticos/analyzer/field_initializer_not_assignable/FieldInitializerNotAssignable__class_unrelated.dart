class A {
  int x;
  A() : x = '';
//          ^^
// [diag.fieldInitializerNotAssignable] The initializer type 'String' can't be assigned to the field type 'int'.
}
