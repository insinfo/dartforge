class A<T> {
  static foo(T a) {}
//           ^
// [diag.typeParameterReferencedByStatic] Static members can't reference type parameters of the class.
}
