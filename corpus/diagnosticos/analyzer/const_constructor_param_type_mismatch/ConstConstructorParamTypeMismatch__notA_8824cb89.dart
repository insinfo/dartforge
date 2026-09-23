typedef String Int2String(int x);
class A {
  final Int2String f;
  const A(this.f);
}
int foo(String x) => 1;
var v = const A(foo);
//              ^^^
// [diag.argumentTypeNotAssignable] The argument type 'int Function(String)' can't be assigned to the parameter type 'Int2String'.
// [diag.constConstructorParamTypeMismatch] A value of type 'int Function(String)' can't be assigned to a parameter of type 'String Function(int)' in a const constructor.
