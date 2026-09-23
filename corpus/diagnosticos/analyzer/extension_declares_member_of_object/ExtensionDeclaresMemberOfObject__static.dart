extension E on String {
  static void hashCode() {}
//            ^^^^^^^^
// [diag.extensionDeclaresMemberOfObject] Extensions can't declare members with the same name as a member declared by 'Object'.
}
