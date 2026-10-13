void f(void v) {
  <void, int>{?v: ''};
//             ^
// [diag.useOfVoidResult] This expression has a type of 'void' so its value can't be used.
//                ^^
// [diag.mapValueTypeNotAssignable] The element type 'String' can't be assigned to the map value type 'int'.
}
