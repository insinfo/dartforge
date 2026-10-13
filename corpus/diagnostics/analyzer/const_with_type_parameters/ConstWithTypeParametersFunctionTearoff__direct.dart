void f<T>(T a) {}
class A<U> {
  void m() {
    const c = f<U>;
//        ^
// [diag.unusedLocalVariable] The value of the local variable 'c' isn't used.
//              ^
// [diag.constWithTypeParametersFunctionTearoff] A constant function tearoff can't use a type parameter as a type argument.
  }
}
