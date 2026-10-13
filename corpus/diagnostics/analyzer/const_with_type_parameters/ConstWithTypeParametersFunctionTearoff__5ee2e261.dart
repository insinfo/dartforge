void f<T>(T t) => t;

class C<T> {
  void foo([void Function(T) p = f]) {}
//                               ^
// [diag.constWithTypeParametersFunctionTearoff] A constant function tearoff can't use a type parameter as a type argument.
}
