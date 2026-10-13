extension E on String {
  bool operator==(Object _) => false;
//             ^^
// [diag.extensionDeclaresMemberOfObject] Extensions can't declare members with the same name as a member declared by 'Object'.
  int get hashCode => 0;
//        ^^^^^^^^
// [diag.extensionDeclaresMemberOfObject] Extensions can't declare members with the same name as a member declared by 'Object'.
  String toString() => '';
//       ^^^^^^^^
// [diag.extensionDeclaresMemberOfObject] Extensions can't declare members with the same name as a member declared by 'Object'.
  dynamic get runtimeType => null;
//            ^^^^^^^^^^^
// [diag.extensionDeclaresMemberOfObject] Extensions can't declare members with the same name as a member declared by 'Object'.
  dynamic noSuchMethod(_) => null;
//        ^^^^^^^^^^^^
// [diag.extensionDeclaresMemberOfObject] Extensions can't declare members with the same name as a member declared by 'Object'.
}
