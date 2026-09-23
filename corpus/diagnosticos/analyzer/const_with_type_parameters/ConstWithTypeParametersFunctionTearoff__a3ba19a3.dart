void f<T>(T t) => t;

void bar<T>([void Function(T Function()) p = f]) {}
//                                           ^
// [diag.constWithTypeParametersFunctionTearoff] A constant function tearoff can't use a type parameter as a type argument.
