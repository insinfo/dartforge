typedef F<X> = void Function(X);
class A<X> {}
enum E<X> implements A<F<X>> {
//     ^
// [diag.wrongTypeParameterVarianceInSuperinterface] 'X' can't be used contravariantly or invariantly in 'A<F<X>>'.
  v
}
