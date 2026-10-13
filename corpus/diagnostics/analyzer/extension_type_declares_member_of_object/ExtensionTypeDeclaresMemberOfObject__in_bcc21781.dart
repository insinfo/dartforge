extension type E(int it) {
  bool operator==(Object _) => false;
//             ^^
// [diag.extensionTypeDeclaresMemberOfObject] Extension types can't declare members with the same name as a member declared by 'Object'.
  int get hashCode => 0;
//        ^^^^^^^^
// [diag.extensionTypeDeclaresMemberOfObject] Extension types can't declare members with the same name as a member declared by 'Object'.
  String toString() => '';
//       ^^^^^^^^
// [diag.extensionTypeDeclaresMemberOfObject] Extension types can't declare members with the same name as a member declared by 'Object'.
  dynamic get runtimeType => null;
//            ^^^^^^^^^^^
// [diag.extensionTypeDeclaresMemberOfObject] Extension types can't declare members with the same name as a member declared by 'Object'.
  dynamic noSuchMethod(_) => null;
//        ^^^^^^^^^^^^
// [diag.extensionTypeDeclaresMemberOfObject] Extension types can't declare members with the same name as a member declared by 'Object'.
}
