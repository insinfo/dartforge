class A<T> {
  A<T> Function() fn;
  A([this.fn = A<T>.new]);
//               ^
// [diag.constWithTypeParametersConstructorTearoff] A constant constructor tearoff can't use a type parameter as a type argument.
}
