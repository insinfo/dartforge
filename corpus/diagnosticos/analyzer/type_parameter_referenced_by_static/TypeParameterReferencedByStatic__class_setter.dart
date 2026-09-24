class A<T> {
  static set foo(T _) {}
//               ^
// [diag.typeParameterReferencedByStatic] Static members can't reference type parameters of the class.
}
