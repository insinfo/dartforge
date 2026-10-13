extension type E(int it) {
  E.named() : it = 0, super();
//                    ^^^^^
// [diag.extensionTypeConstructorWithSuperInvocation] Extension type constructors can't include super initializers.
}
