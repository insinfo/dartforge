typedef F<X> = void Function(X);
class A<X> {}
mixin M {}
class B<X> = Object with M implements A<F<X>>;
//      ^
// [diag.wrongTypeParameterVarianceInSuperinterface] 'X' can't be used contravariantly or invariantly in 'A<F<X>>'.
