typedef F1<X> = X Function();
typedef F2<X> = void Function(F1<X>);
class A<X> {}
class B<X> extends A<F2<X>> {}
//      ^
// [diag.wrongTypeParameterVarianceInSuperinterface] 'X' can't be used contravariantly or invariantly in 'A<F2<X>>'.
