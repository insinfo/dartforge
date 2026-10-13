class A<T> {
  static T foo() {
//       ^
// [diag.typeParameterReferencedByStatic] Static members can't reference type parameters of the class.
    throw 0;
  }
}
