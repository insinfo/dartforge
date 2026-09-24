extension type E(int it) {
  static int get hashCode => 0;
//               ^^^^^^^^
// [diag.extensionTypeDeclaresMemberOfObject] Extension types can't declare members with the same name as a member declared by 'Object'.
  static int get noSuchMethod => 0;
//               ^^^^^^^^^^^^
// [diag.extensionTypeDeclaresMemberOfObject] Extension types can't declare members with the same name as a member declared by 'Object'.
  static int get runtimeType => 0;
//               ^^^^^^^^^^^
// [diag.extensionTypeDeclaresMemberOfObject] Extension types can't declare members with the same name as a member declared by 'Object'.
  static int get toString => 0;
//               ^^^^^^^^
// [diag.extensionTypeDeclaresMemberOfObject] Extension types can't declare members with the same name as a member declared by 'Object'.
}
