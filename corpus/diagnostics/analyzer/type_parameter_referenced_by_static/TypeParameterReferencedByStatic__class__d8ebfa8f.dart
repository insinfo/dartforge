class A<T> {
  static foo() {
    new T();
//      ^
// [diag.newWithNonType] The name 'T' isn't a class.
// [diag.typeParameterReferencedByStatic] Static members can't reference type parameters of the class.
  }
}
