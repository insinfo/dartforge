extension type E(int it) {
  static int hashCode() => 0;
//           ^^^^^^^^
// [diag.extensionTypeDeclaresMemberOfObject] Extension types can't declare members with the same name as a member declared by 'Object'.
  static dynamic noSuchMethod(Invocation i) => null;
//               ^^^^^^^^^^^^
// [diag.extensionTypeDeclaresMemberOfObject] Extension types can't declare members with the same name as a member declared by 'Object'.
  static Type runtimeType() => int;
//            ^^^^^^^^^^^
// [diag.extensionTypeDeclaresMemberOfObject] Extension types can't declare members with the same name as a member declared by 'Object'.
  static String toString() => '';
//              ^^^^^^^^
// [diag.extensionTypeDeclaresMemberOfObject] Extension types can't declare members with the same name as a member declared by 'Object'.
}
