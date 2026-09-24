class A<T> {
  const A(T x);
}
var v = const A<int>('foo');
//                   ^^^^^
// [diag.argumentTypeNotAssignable] The argument type 'String' can't be assigned to the parameter type 'int'.
// [diag.constConstructorParamTypeMismatch] A value of type 'String' can't be assigned to a parameter of type 'int' in a const constructor.
