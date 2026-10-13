class A {
  final int x;
  const A(this.x);
}
var v = const A('foo');
//              ^^^^^
// [diag.argumentTypeNotAssignable] The argument type 'String' can't be assigned to the parameter type 'int'.
// [diag.constConstructorParamTypeMismatch] A value of type 'String' can't be assigned to a parameter of type 'int' in a const constructor.
