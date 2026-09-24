extension E<T> on int {
  static T? foo;
//       ^
// [diag.typeParameterReferencedByStatic] Static members can't reference type parameters of the class.
}
