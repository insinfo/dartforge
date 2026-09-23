class A {
  final int x;
  const A([this.x = 'foo']);
//                  ^^^^^
// [diag.invalidAssignment] A value of type 'String' can't be assigned to a variable of type 'int'.
}
var v = const A();
//      ^^^^^^^^^
// [diag.constConstructorParamTypeMismatch] A value of type 'String' can't be assigned to a parameter of type 'int' in a const constructor.
