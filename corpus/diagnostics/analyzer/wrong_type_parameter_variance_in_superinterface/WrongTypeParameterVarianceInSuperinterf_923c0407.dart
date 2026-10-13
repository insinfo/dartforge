typedef F<X> = void Function(X);
class A<X> {}
mixin B<X> on A<F<X>> {}
//      ^
// [diag.wrongTypeParameterVarianceInSuperinterface] 'X' can't be used contravariantly or invariantly in 'A<F<X>>'.
