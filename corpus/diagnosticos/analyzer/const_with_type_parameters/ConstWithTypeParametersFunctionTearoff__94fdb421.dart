void f<T>(T a) {}
class A<U> {
  void m([void Function(U) fn = f<U>]) {}
//                                ^
// [diag.constWithTypeParametersFunctionTearoff] A constant function tearoff can't use a type parameter as a type argument.
}
