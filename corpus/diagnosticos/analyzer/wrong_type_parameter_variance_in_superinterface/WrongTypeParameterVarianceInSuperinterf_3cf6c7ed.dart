typedef F1<X> = void Function(X);
typedef F2<X> = F1<X> Function();
class A<X> {}
class B<X> extends A<F2<X>> {}
//      ^
// [diag.wrongTypeParameterVarianceInSuperinterface] 'X' can't be used contravariantly or invariantly in 'A<F2<X>>'.
