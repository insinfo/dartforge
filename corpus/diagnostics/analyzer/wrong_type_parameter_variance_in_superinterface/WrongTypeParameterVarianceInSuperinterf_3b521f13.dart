typedef F<X> = void Function(X);
mixin M<X> {}
class B<X> = Object with M<F<X>>;
//      ^
// [diag.wrongTypeParameterVarianceInSuperinterface] 'X' can't be used contravariantly or invariantly in 'M<F<X>>'.
