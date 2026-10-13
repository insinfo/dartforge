class A<T> {}
extension type B<T>(A<Never Function(Object?)> it)
//               ^
// [diag.wrongTypeParameterVarianceInSuperinterface] 'T' can't be used contravariantly or invariantly in 'A<T Function(T)>'.
  implements A<T Function(T)> {}
