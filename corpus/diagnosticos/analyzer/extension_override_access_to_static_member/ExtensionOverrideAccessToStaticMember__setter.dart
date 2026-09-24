extension E on String {
  static void set empty(String s) {}
}
void f() {
  E('a').empty = 'b';
//       ^^^^^
// [diag.extensionOverrideAccessToStaticMember] An extension override can't be used to access a static member from an extension.
}
