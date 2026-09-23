extension E<T> on int {
  static T foo() => throw 0;
//       ^
// [diag.typeParameterReferencedByStatic] Static members can't reference type parameters of the class.
}
