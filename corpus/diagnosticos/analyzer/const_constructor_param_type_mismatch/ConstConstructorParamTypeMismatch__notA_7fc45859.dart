class A {
  const A();
}
class B extends A {
  const B();
}
class C {
  final B b;
  const C(this.b);
}
const A u = const A();
var v = const C(u);
//              ^
// [diag.argumentTypeNotAssignable] The argument type 'A' can't be assigned to the parameter type 'B'.
// [diag.constConstructorParamTypeMismatch] A value of type 'A' can't be assigned to a parameter of type 'B' in a const constructor.
