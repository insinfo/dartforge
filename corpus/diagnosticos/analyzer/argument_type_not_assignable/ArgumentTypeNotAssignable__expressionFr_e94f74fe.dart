class C<T> {
  C(T a);
}

var g = C<int>.new;
var x = g('Hello');
//        ^^^^^^^
// [diag.argumentTypeNotAssignable] The argument type 'String' can't be assigned to the parameter type 'int'.
