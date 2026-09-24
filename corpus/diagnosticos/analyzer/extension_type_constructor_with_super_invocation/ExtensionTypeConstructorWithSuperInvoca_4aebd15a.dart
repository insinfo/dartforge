extension type E(int it) {
  E.named() : it = 0, super.named();
//                    ^^^^^
// [diag.extensionTypeConstructorWithSuperInvocation] Extension type constructors can't include super initializers.
}
