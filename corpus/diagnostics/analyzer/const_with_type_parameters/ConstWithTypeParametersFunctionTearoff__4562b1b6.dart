void f<T>(T a) {}
class A<U> {
  const A();
//^^^^^
// [diag.constConstructorWithFieldInitializedByNonConst] Can't define the 'const' constructor because the field 'x' is initialized with a non-constant value.
  final x = f<U>;
//            ^
// [diag.constWithTypeParametersFunctionTearoff] A constant function tearoff can't use a type parameter as a type argument.
}
