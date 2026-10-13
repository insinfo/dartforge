class A<T> {
  static T? get foo => null;
//       ^
// [diag.typeParameterReferencedByStatic] Static members can't reference type parameters of the class.
}
