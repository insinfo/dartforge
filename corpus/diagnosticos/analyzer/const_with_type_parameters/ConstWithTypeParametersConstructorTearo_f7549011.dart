class A<T> {
  void m() {
    const c = A<List<T>>.new;
//        ^
// [diag.unusedLocalVariable] The value of the local variable 'c' isn't used.
//                   ^
// [diag.constWithTypeParametersConstructorTearoff] A constant constructor tearoff can't use a type parameter as a type argument.
  }
}
