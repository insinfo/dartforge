class A<T> {
  const A();
  final x = A<T>.new;
//            ^
// [diag.constWithTypeParametersConstructorTearoff] A constant constructor tearoff can't use a type parameter as a type argument.
}
