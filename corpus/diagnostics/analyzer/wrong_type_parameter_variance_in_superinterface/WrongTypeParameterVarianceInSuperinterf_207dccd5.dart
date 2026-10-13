typedef F<X> = void Function(X);
mixin A<X> {}
class B<X> extends Object with A<F<X>> {}
//      ^
// [diag.wrongTypeParameterVarianceInSuperinterface] 'X' can't be used contravariantly or invariantly in 'A<F<X>>'.
