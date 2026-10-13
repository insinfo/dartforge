typedef F<X> = void Function(X);
class A<X> {}
class B<X> implements A<F<X>> {}
//      ^
// [diag.wrongTypeParameterVarianceInSuperinterface] 'X' can't be used contravariantly or invariantly in 'A<F<X>>'.
