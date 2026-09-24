typedef F<X> = void Function(X);
mixin A<X> {}
enum E<X> with A<F<X>> {
//     ^
// [diag.wrongTypeParameterVarianceInSuperinterface] 'X' can't be used contravariantly or invariantly in 'A<F<X>>'.
  v
}
