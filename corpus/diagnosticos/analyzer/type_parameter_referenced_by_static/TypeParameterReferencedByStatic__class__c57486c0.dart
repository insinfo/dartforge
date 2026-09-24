class A<T> {
  static foo() {
    // ignore:unused_local_variable
    T v;
//  ^
// [diag.typeParameterReferencedByStatic] Static members can't reference type parameters of the class.
  }
}
