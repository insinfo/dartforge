extension type const E._(int it) {
  const E(int it) : super._(it), assert(it >= 0);
//      ^
// [diag.finalNotInitializedConstructor1] All final variables must be initialized, but 'it' isn't.
//                  ^^^^^
// [diag.extensionTypeConstructorWithSuperInvocation] Extension type constructors can't include super initializers.
}
