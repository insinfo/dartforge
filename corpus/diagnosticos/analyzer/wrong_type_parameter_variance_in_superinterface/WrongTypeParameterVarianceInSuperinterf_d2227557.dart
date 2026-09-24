class A<T> {}
extension type B<T>(A<void Function(Object?)> it)
//               ^
// [diag.wrongTypeParameterVarianceInSuperinterface] 'T' can't be used contravariantly or invariantly in 'A<void Function(T)>'.
  implements A<void Function(T)> {}
