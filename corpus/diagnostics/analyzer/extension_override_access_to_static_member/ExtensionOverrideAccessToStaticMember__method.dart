extension E on String {
  static String empty() => '';
}
void f() {
  E('a').empty();
//       ^^^^^
// [diag.extensionOverrideAccessToStaticMember] An extension override can't be used to access a static member from an extension.
}
