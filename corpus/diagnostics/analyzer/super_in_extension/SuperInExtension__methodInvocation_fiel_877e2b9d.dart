extension E on int {
  late final v = super.foo();
//           ^
// [diag.extensionDeclaresInstanceField] Extensions can't declare instance fields.
//               ^^^^^
// [diag.superInExtension] The 'super' keyword can't be used in an extension because an extension doesn't have a superclass.
}
