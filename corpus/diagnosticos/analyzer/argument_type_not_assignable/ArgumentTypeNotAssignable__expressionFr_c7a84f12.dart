void f<T>(T a) {}

var g = f<int>;
var x = g('Hello');
//        ^^^^^^^
// [diag.argumentTypeNotAssignable] The argument type 'String' can't be assigned to the parameter type 'int'.
