class A<T> {}
extension type B<T>(A<Never Function()> it)
  implements A<T Function()> {}
