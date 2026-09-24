extension type A(int it) {
  void f() {
    super.foo();
//  ^^^^^
// [diag.superInExtensionType] The 'super' keyword can't be used in an extension type because an extension type doesn't have a superclass.
  }
}
