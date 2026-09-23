class A<T> {
  static Object foo() {
    return (T a) {};
//          ^
// [diag.typeParameterReferencedByStatic] Static members can't reference type parameters of the class.
  }
}
