void f(void v) {
  <int, void>{'': ?v};
//            ^^
// [diag.mapKeyTypeNotAssignable] The element type 'String' can't be assigned to the map key type 'int'.
//                 ^
// [diag.useOfVoidResult] This expression has a type of 'void' so its value can't be used.
}
